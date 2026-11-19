//! CBZ/CBR okuyucu. Sayfa görseli terminalin desteklediği protokolle (sixel, kitty, iTerm2;
//! yoksa yarım blok karakterler) çizilir. Çözme, ölçekleme ve kodlama arka planda yapılır.
//!
//! Konum web ve Android ile aynı: sayfa dizini (0'dan), yüzde `(sayfa + 1) / sayfa sayısı`.

use std::collections::HashSet;

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};
use ratatui_image::{protocol::Protocol, FilterType, Image, Resize};
use readcrab_core::{BookDetail, PositionSuggestion};

use crate::{
    app::{Action, Ctx, Fx, Msg},
    reader::{self, MarkAct, Marks, Session},
    theme, util, widgets,
};

struct Shown {
    page: i32,
    area: Rect,
    proto: Protocol,
}

enum Popup {
    Marks(Marks),
    Suggest(PositionSuggestion),
    Goto(String),
}

pub struct ComicReader {
    detail: BookDetail,
    count: i32,
    page: i32,
    shown: Option<Shown>,
    loading: Option<(i32, Rect)>,
    error: Option<(i32, String)>,
    prefetched: HashSet<i32>,
    session: Session,
    popup: Option<Popup>,
    suggestion_checked: bool,
}

impl ComicReader {
    pub const HELP: &'static [(&'static str, &'static str)] = &[
        ("→ l j Boşluk", "sonraki sayfa"),
        ("← h k b", "önceki sayfa"),
        ("g / G", "ilk / son sayfa"),
        (":", "sayfaya git"),
        ("m", "bu sayfaya yer imi koy / kaldır"),
        ("M / '", "yer imleri"),
        ("N", "sonraki cilt"),
        ("r", "sayfayı yeniden yükle"),
        ("q / Esc", "kütüphaneye dön"),
    ];

    pub fn new(ctx: &Ctx, detail: BookDetail) -> ComicReader {
        let count = detail.book.page_count.max(1);
        let page = detail
            .locator
            .trim()
            .parse::<i32>()
            .ok()
            .filter(|p| (0..count).contains(p))
            .unwrap_or(0);
        let id = detail.book.id;
        let session = Session::start(ctx, id, detail.book.percent, &detail.locator);
        ctx.spawn(move |core| {
            let _ = core.pull_book(id);
            Some(Msg::Pulled { book_id: id })
        });
        ComicReader {
            detail,
            count,
            page,
            shown: None,
            loading: None,
            error: None,
            prefetched: HashSet::new(),
            session,
            popup: None,
            suggestion_checked: false,
        }
    }

    fn id(&self) -> i64 {
        self.detail.book.id
    }

    pub fn captures_input(&self) -> bool {
        matches!(self.popup, Some(Popup::Goto(_)))
    }

    fn percent(&self) -> f64 {
        ((self.page + 1) as f64 / self.count as f64).clamp(0.0, 1.0)
    }

    fn set_page(&mut self, p: i32) {
        let p = p.clamp(0, self.count - 1);
        if p == self.page {
            return;
        }
        if p > self.page {
            self.session.page_turned();
        }
        self.page = p;
        self.error = None;
        self.session.input();
        self.session.mark(self.page.to_string(), self.percent());
    }

    fn next(&mut self, fx: &mut Fx) {
        if self.page + 1 >= self.count {
            match &self.detail.next_book_title {
                Some(t) => fx.info(format!("Cilt bitti. N ile sonrakine geç: «{t}»")),
                None => fx.info("Son sayfa."),
            }
            return;
        }
        self.set_page(self.page + 1);
    }

    fn request(&mut self, ctx: &Ctx, page: i32, area: Rect) {
        if self.loading == Some((page, area)) {
            return;
        }
        self.loading = Some((page, area));
        let id = self.id();
        let picker = ctx.picker.clone();
        ctx.spawn(move |core| {
            let result = core
                .page_path(id, page)
                .map_err(|e| e.to_string())
                .and_then(|p| std::fs::read(p).map_err(|e| e.to_string()))
                .and_then(|bytes| {
                    image::load_from_memory(&bytes).map_err(|e| format!("Görsel çözülemedi: {e}"))
                })
                .and_then(|img| {
                    picker
                        .new_protocol(img, area, Resize::Scale(Some(FilterType::Triangle)))
                        .map_err(|e| format!("Görsel çizilemedi: {e}"))
                });
            Some(Msg::Page {
                book_id: id,
                page,
                area,
                result,
            })
        });
    }

    /// Sonraki sayfayı yalnız indirir (çözmez).
    fn prefetch(&mut self, ctx: &Ctx, page: i32) {
        if page >= self.count || !self.prefetched.insert(page) {
            return;
        }
        let id = self.id();
        ctx.spawn(move |core| {
            let _ = core.page_path(id, page);
            None
        });
    }

    pub fn key(&mut self, ctx: &Ctx, k: KeyEvent, fx: &mut Fx) {
        if let Some(p) = self.popup.take() {
            self.popup = self.popup_key(ctx, p, k, fx);
            if self.popup.is_none() {
                fx.clear = true;
            }
            return;
        }
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => fx.act(Action::Back),
            KeyCode::Right
            | KeyCode::Char('l')
            | KeyCode::Char('j')
            | KeyCode::Down
            | KeyCode::Char(' ')
            | KeyCode::PageDown
            | KeyCode::Char('n') => self.next(fx),
            KeyCode::Left
            | KeyCode::Char('h')
            | KeyCode::Char('k')
            | KeyCode::Up
            | KeyCode::Char('b')
            | KeyCode::PageUp
            | KeyCode::Backspace
            | KeyCode::Char('p') => self.set_page(self.page - 1),
            KeyCode::Home | KeyCode::Char('g') => self.set_page(0),
            KeyCode::End | KeyCode::Char('G') => self.set_page(self.count - 1),
            KeyCode::Char(':') => self.popup = Some(Popup::Goto(String::new())),
            KeyCode::Char('m') => {
                let loc = self.page.to_string();
                let marks = ctx.core.list_bookmarks(self.id()).unwrap_or_default();
                let r = match marks.iter().find(|b| b.locator == loc) {
                    Some(b) => ctx
                        .core
                        .delete_bookmark(b.local_id)
                        .map(|_| fx.info("Yer imi kaldırıldı.")),
                    None => ctx
                        .core
                        .add_bookmark(self.id(), loc, format!("Sayfa {}", self.page + 1))
                        .map(|_| fx.info("Yer imi eklendi.")),
                };
                if let Err(e) = r {
                    fx.error(e.to_string());
                }
            }
            KeyCode::Char('M') | KeyCode::Char('\'') => {
                self.popup = Some(Popup::Marks(Marks::load(ctx, self.id())));
            }
            KeyCode::Char('N') => match self.detail.next_book_id {
                Some(id) => fx.act(Action::ReplaceBook(id)),
                None => fx.info("Bu serinin son kitabı."),
            },
            KeyCode::Char('r') => {
                self.error = None;
                self.loading = None;
                self.shown = None;
                fx.clear = true;
            }
            _ => {}
        }
    }

    fn popup_key(&mut self, ctx: &Ctx, p: Popup, k: KeyEvent, fx: &mut Fx) -> Option<Popup> {
        match p {
            Popup::Marks(mut m) => match m.key(ctx, self.id(), k) {
                MarkAct::None => Some(Popup::Marks(m)),
                MarkAct::Close => None,
                MarkAct::Go(loc) => {
                    match loc.trim().parse::<i32>() {
                        Ok(n) if (0..self.count).contains(&n) => self.set_page(n),
                        _ => fx.error("Yer imi bu kitapta geçersiz."),
                    }
                    None
                }
            },
            Popup::Suggest(s) => {
                if matches!(k.code, KeyCode::Enter | KeyCode::Char('e')) {
                    let _ = ctx.core.accept_position(self.id());
                    match s.locator.trim().parse::<i32>() {
                        Ok(n) if (0..self.count).contains(&n) => {
                            self.page = n;
                            self.error = None;
                            self.session.reset_saved(&s.locator, s.percent);
                        }
                        _ => fx.error("Önerilen konum bu kitapta geçersiz."),
                    }
                } else {
                    let _ = ctx.core.decline_position(self.id());
                }
                None
            }
            Popup::Goto(mut s) => match k.code {
                KeyCode::Esc => None,
                KeyCode::Enter => {
                    match s.trim().parse::<i32>() {
                        Ok(n) if n >= 1 && n <= self.count => self.set_page(n - 1),
                        _ => fx.error(format!("1 ile {} arasında bir sayı girin.", self.count)),
                    }
                    None
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    if s.len() < 6 {
                        s.push(c);
                    }
                    Some(Popup::Goto(s))
                }
                KeyCode::Backspace => {
                    s.pop();
                    Some(Popup::Goto(s))
                }
                _ => Some(Popup::Goto(s)),
            },
        }
    }

    pub fn on_msg(&mut self, ctx: &Ctx, m: Msg, fx: &mut Fx) {
        match m {
            Msg::Page {
                book_id,
                page,
                area,
                result,
            } if book_id == self.id() => {
                if self.loading == Some((page, area)) {
                    self.loading = None;
                }
                if page != self.page {
                    return;
                }
                match result {
                    Ok(proto) => {
                        // yeni görsel eskisini tamamen örtmüyorsa kalıntı kalmasın diye ekran temizlenir
                        let (nw, nh) = (proto.area().width, proto.area().height);
                        if let Some(old) = &self.shown {
                            let o = old.proto.area();
                            if old.area != area || nw < o.width || nh < o.height {
                                fx.clear = true;
                            }
                        }
                        self.shown = Some(Shown { page, area, proto });
                        self.error = None;
                        self.prefetch(ctx, page + 1);
                    }
                    Err(e) => self.error = Some((page, e)),
                }
            }
            Msg::Pulled { book_id } if book_id == self.id() && !self.suggestion_checked => {
                self.suggestion_checked = true;
                if let Ok(Some(s)) = ctx.core.position_suggestion(book_id) {
                    if self.popup.is_none() {
                        self.popup = Some(Popup::Suggest(s));
                        fx.clear = true;
                    }
                } else if !self.session.touched() {
                    // yerelde konum yokken sunucununki doğrudan uygulanmış olabilir
                    if let Ok(p) = ctx.core.get_progress(book_id) {
                        let page = p.locator.trim().parse::<i32>().ok();
                        if let Some(n) =
                            page.filter(|n| (0..self.count).contains(n) && *n != self.page)
                        {
                            self.session.reset_saved(&p.locator, p.percent);
                            self.page = n;
                            self.error = None;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn tick(&mut self, ctx: &Ctx) {
        self.session.tick(ctx);
    }

    pub fn close(&mut self, ctx: &Ctx) {
        self.session.close(ctx);
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &Ctx) {
        let area = f.area();
        let [head, body, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(area);
        reader::header(
            f,
            head,
            &self.detail.series_name,
            &self.detail.book.title,
            &format!("{}/{}", self.page + 1, self.count),
        );

        let error = self
            .error
            .as_ref()
            .filter(|(p, _)| *p == self.page)
            .map(|(_, e)| e.clone());
        let current = self
            .shown
            .as_ref()
            .is_some_and(|s| s.page == self.page && s.area == body);
        if !current && error.is_none() && self.popup.is_none() {
            self.request(ctx, self.page, body);
        }
        if let Some(e) = error {
            widgets::message(
                f,
                body,
                vec![
                    Line::styled(e.clone(), Style::default().fg(theme::ERR)),
                    Line::default(),
                    widgets::hints(&[("r", "yeniden dene"), ("→/←", "sayfa"), ("Esc", "geri")]),
                ],
            );
        } else if let Some(s) = &self.shown {
            // eski sayfa yenisi hazır olana dek görünür kalır (titreme olmasın)
            let a = s.proto.area();
            if a.width <= body.width && a.height <= body.height {
                let r = Rect::new(
                    body.x + (body.width - a.width) / 2,
                    body.y + (body.height - a.height) / 2,
                    a.width,
                    a.height,
                );
                f.render_widget(Image::new(&s.proto), r);
            } else {
                widgets::message(f, body, vec![Line::styled("Yükleniyor…", theme::dim())]);
            }
        } else {
            widgets::message(f, body, vec![Line::styled("Yükleniyor…", theme::dim())]);
        }

        let pct = self.percent();
        let mut spans = vec![Span::styled(
            format!(" Sayfa {}/{} ", self.page + 1, self.count),
            theme::dim(),
        )];
        spans.extend(widgets::bar(pct, 16));
        spans.push(Span::styled(
            format!(" {} ", util::fmt_percent(pct)),
            Style::default().fg(theme::ACCENT),
        ));
        if self.loading.is_some_and(|(p, _)| p == self.page) {
            spans.push(Span::styled(
                " yükleniyor…",
                Style::default().fg(theme::WARN),
            ));
        }
        if self.page + 1 >= self.count {
            if let Some(t) = &self.detail.next_book_title {
                spans.push(Span::styled(
                    format!("  Cilt bitti · N: «{}»", util::truncate(t, 40)),
                    Style::default().fg(theme::OK),
                ));
            }
        } else {
            spans.push(Span::raw("  "));
            spans.extend(
                widgets::hints(&[
                    ("←/→", "sayfa"),
                    (":", "git"),
                    ("m", "yer imi"),
                    ("?", "yardım"),
                ])
                .spans,
            );
        }
        f.render_widget(Paragraph::new(Line::from(spans)), foot);

        match &mut self.popup {
            None => {}
            Some(Popup::Marks(m)) => m.draw(f),
            Some(Popup::Suggest(s)) => reader::draw_suggestion(f, s, pct),
            Some(Popup::Goto(s)) => {
                let r = widgets::centered(f.area(), 40, 3);
                f.render_widget(Clear, r);
                f.render_widget(
                    Paragraph::new(widgets::input_line("Sayfa", s, true, false)).block(
                        widgets::popup_block(&format!("Sayfaya git (1-{})", self.count)),
                    ),
                    r,
                );
            }
        }
    }
}
