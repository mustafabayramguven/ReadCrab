//! EPUB/MOBI/TXT okuyucu: bölüm gövdesi terminal genişliğine dizilir, kaydırarak okunur.
//!
//! Konum web ve Android ile aynı biçimdedir: `{"ch":bölüm,"p":bölüm içi kaydırma oranı,
//! "c":satır başı}`, genel yüzde `(ch + p) / bölüm sayısı`. Kaydedilen yer, ekranın en üstündeki
//! satırın başıdır (`c`, bkz. `wrap::Laid::starts`); açılışta o satır en üste gelir ve solunda
//! "kaldığın yer" belirteci (`▌`) durur. Belirteç oturum boyunca yerinde kalır, okuyucu
//! kapanırken konum o anki satıra taşınır. Kitap başına tek konum olduğundan eşitleme değişmez.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{Clear, ListItem, ListState, Paragraph},
    Frame,
};
use readcrab_core::{BookDetail, CoreError, PositionSuggestion, SearchHitItem, TocEntry};

use crate::{
    app::{Action, Ctx, Fx, Msg},
    html::{self, Block},
    reader::{self, MarkAct, Marks, Session},
    theme, util, widgets, wrap,
};

enum Target {
    /// Satır başı (locator `c`): o birimi içeren satır en üste gelir.
    Line(u32),
    /// Kaydırma oranı (locator `p`); `c`'siz eski kayıtlar.
    Ratio(f64),
    /// Bölüm metninin kesri (arama sonucu); isabet ekranın üst üçte birine gelir.
    Fraction(f64),
}

struct Search {
    query: String,
    typing: bool,
    busy: bool,
    hits: Vec<SearchHitItem>,
    truncated: bool,
    error: Option<String>,
    state: ListState,
}

enum Popup {
    Toc(ListState),
    Marks(Marks),
    Search(Search),
    Suggest(PositionSuggestion),
}

pub struct TextReader {
    detail: BookDetail,
    count: i32,
    toc: Vec<TocEntry>,
    ch: i32,
    cache: HashMap<i32, Arc<Vec<Block>>>,
    requested: HashSet<i32>,
    lines: Vec<Line<'static>>,
    /// Satır başlarının bölümdeki sırası (`lines` ile aynı uzunlukta).
    starts: Vec<u32>,
    laid: Option<(i32, usize)>,
    top: usize,
    view_h: usize,
    fwd_lines: usize,
    target: Option<Target>,
    mark_after: bool,
    error: Option<String>,
    session: Session,
    popup: Option<Popup>,
    width_pref: u16,
    suggestion_checked: bool,
    /// "Kaldığın yer" belirteci: (bölüm, satır başı).
    pin: Option<(i32, u32)>,
    /// Konum `c`'siz (eski biçim): belirteç bu bölüm ilk dizildiğinde üstteki satıra konur.
    pin_pending: Option<i32>,
}

const MIN_W: u16 = 30;
const MAX_W: u16 = 200;

impl TextReader {
    pub const HELP: &'static [(&'static str, &'static str)] = &[
        ("j k / ↑ ↓", "satır kaydır"),
        (
            "Boşluk / PgDn",
            "sonraki sayfa (bölüm sonunda sonraki bölüm)",
        ),
        ("b / PgUp", "önceki sayfa"),
        ("n / → / l", "sonraki bölüm"),
        ("p / ← / h", "önceki bölüm"),
        ("g / G", "bölüm başı / sonu"),
        ("t", "içindekiler"),
        ("m", "yer imi ekle"),
        ("M / '", "yer imleri"),
        ("/", "kitapta ara (çevrimiçi)"),
        ("+ / -", "metin genişliği"),
        ("N", "sonraki cilt"),
        ("r", "bölümü yeniden yükle"),
        ("▌", "kaldığın satır; kapatınca o anki satıra taşınır"),
        ("q / Esc", "kütüphaneye dön"),
    ];

    pub fn new(ctx: &Ctx, detail: BookDetail) -> TextReader {
        let id = detail.book.id;
        let loc = util::parse_text_loc(&detail.locator);
        let ch = loc.map(|l| l.ch).unwrap_or(0);
        let target = loc.map(target_of).unwrap_or(Target::Ratio(0.0));
        let session = Session::start(ctx, id, detail.book.percent, &detail.locator);
        let count = detail.book.page_count.max(1);
        let mut r = TextReader {
            detail,
            count,
            toc: Vec::new(),
            ch,
            cache: HashMap::new(),
            requested: HashSet::new(),
            lines: Vec::new(),
            starts: Vec::new(),
            laid: None,
            top: 0,
            view_h: 20,
            fwd_lines: 0,
            target: Some(target),
            mark_after: false,
            error: None,
            session,
            popup: None,
            width_pref: 80,
            suggestion_checked: false,
            pin: None,
            pin_pending: None,
        };
        if let Some(l) = loc {
            r.pin_at(&l);
        }
        ctx.spawn(move |core| {
            Some(Msg::TextInfo {
                book_id: id,
                result: core.text_info(id),
            })
        });
        ctx.spawn(move |core| {
            let _ = core.pull_book(id);
            Some(Msg::Pulled { book_id: id })
        });
        r.request(ctx, ch);
        r
    }

    fn id(&self) -> i64 {
        self.detail.book.id
    }

    pub fn captures_input(&self) -> bool {
        matches!(&self.popup, Some(Popup::Search(s)) if s.typing)
    }

    fn request(&mut self, ctx: &Ctx, ch: i32) {
        if ch < 0 || self.cache.contains_key(&ch) || !self.requested.insert(ch) {
            return;
        }
        let id = self.id();
        let path = ctx.chapter_file(id, ch);
        ctx.spawn(move |core| {
            let result = core
                .ensure_chapter(id, ch)
                .and_then(|_| std::fs::read_to_string(&path).map_err(CoreError::from))
                .map(|doc| Arc::new(html::parse(&doc)));
            Some(Msg::Chapter {
                book_id: id,
                ch,
                result,
            })
        });
    }

    fn ready(&self) -> bool {
        self.laid.map(|l| l.0) == Some(self.ch)
    }

    fn max_top(&self) -> usize {
        self.lines.len().saturating_sub(self.view_h)
    }

    fn ratio(&self) -> f64 {
        let m = self.max_top();
        if m == 0 {
            1.0
        } else {
            (self.top as f64 / m as f64).clamp(0.0, 1.0)
        }
    }

    fn percent(&self) -> f64 {
        ((self.ch as f64 + self.ratio()) / self.count.max(1) as f64).clamp(0.0, 1.0)
    }

    fn at_book_end(&self) -> bool {
        self.ready() && self.ch >= self.count - 1 && self.ratio() >= 0.98
    }

    fn chapter_title(&self) -> String {
        self.toc
            .iter()
            .find(|t| t.index == self.ch)
            .map(|t| t.title.clone())
            .unwrap_or_else(|| format!("Bölüm {}", self.ch + 1))
    }

    /// Ekranın en üstündeki satırın başı.
    fn anchor(&self) -> u32 {
        self.starts.get(self.top).copied().unwrap_or(0)
    }

    fn locator(&self) -> String {
        util::build_text_loc(self.ch, self.ratio(), Some(self.anchor()))
    }

    fn pin_at(&mut self, loc: &util::TextLoc) {
        match loc.c {
            Some(c) => {
                self.pin = Some((loc.ch, c));
                self.pin_pending = None;
            }
            None => {
                self.pin = None;
                self.pin_pending = Some(loc.ch);
            }
        }
    }

    fn moved(&mut self) {
        self.session.input();
        if self.ready() {
            let loc = self.locator();
            self.session.mark(loc, self.percent());
        }
    }

    fn go(&mut self, ctx: &Ctx, ch: i32, target: Target, mark: bool) {
        if ch < 0 || ch >= self.count {
            return;
        }
        self.ch = ch;
        self.target = Some(target);
        self.laid = None;
        self.lines.clear();
        self.starts.clear();
        self.top = 0;
        self.error = None;
        self.mark_after = mark;
        self.session.input();
        self.request(ctx, ch);
    }

    fn scroll(&mut self, delta: isize) {
        if !self.ready() {
            return;
        }
        let new = (self.top as isize + delta).clamp(0, self.max_top() as isize) as usize;
        if new == self.top {
            return;
        }
        if new > self.top {
            self.fwd_lines += new - self.top;
            while self.view_h > 0 && self.fwd_lines >= self.view_h {
                self.fwd_lines -= self.view_h;
                self.session.page_turned();
            }
        }
        self.top = new;
        self.moved();
    }

    fn page(&self) -> isize {
        self.view_h.saturating_sub(2).max(1) as isize
    }

    fn page_down(&mut self, ctx: &Ctx, fx: &mut Fx) {
        if !self.ready() {
            return;
        }
        if self.top >= self.max_top() {
            if self.ch + 1 < self.count {
                self.session.page_turned();
                self.go(ctx, self.ch + 1, Target::Ratio(0.0), true);
            } else if let Some(t) = &self.detail.next_book_title {
                fx.info(format!("Kitap bitti. N ile sonraki cilde geç: «{t}»"));
            } else {
                fx.info("Kitabın sonu.");
            }
            return;
        }
        let p = self.page();
        self.scroll(p);
    }

    fn page_up(&mut self, ctx: &Ctx) {
        if !self.ready() {
            return;
        }
        if self.top == 0 {
            if self.ch > 0 {
                self.go(ctx, self.ch - 1, Target::Ratio(1.0), true);
            }
            return;
        }
        let p = self.page();
        self.scroll(-p);
    }

    /// `pin`: "kaldığın yer" belirteci de bu konuma taşınır (başka cihazdan gelen konum).
    fn go_locator(&mut self, ctx: &Ctx, loc: &str, mark: bool, pin: bool) -> bool {
        match util::parse_text_loc(loc) {
            Some(l) if l.ch < self.count => {
                self.go(ctx, l.ch, target_of(l), mark);
                if pin {
                    self.pin_at(&l);
                }
                true
            }
            _ => false,
        }
    }

    pub fn key(&mut self, ctx: &Ctx, k: KeyEvent, fx: &mut Fx) {
        if let Some(p) = self.popup.take() {
            self.popup = self.popup_key(ctx, p, k, fx);
            return;
        }
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => fx.act(Action::Back),
            KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
            KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
            KeyCode::Char('d') if ctrl => {
                let p = self.page() / 2;
                self.scroll(p)
            }
            KeyCode::Char('u') if ctrl => {
                let p = self.page() / 2;
                self.scroll(-p)
            }
            KeyCode::Char(' ') | KeyCode::PageDown | KeyCode::Char('f') => self.page_down(ctx, fx),
            KeyCode::Char('b') | KeyCode::PageUp | KeyCode::Backspace => self.page_up(ctx),
            KeyCode::Home | KeyCode::Char('g') => {
                if self.ready() && self.top != 0 {
                    self.top = 0;
                    self.moved();
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                if self.ready() && self.top != self.max_top() {
                    self.top = self.max_top();
                    self.moved();
                }
            }
            KeyCode::Char('n') | KeyCode::Right | KeyCode::Char('l') => {
                if self.ch + 1 < self.count {
                    self.session.page_turned();
                    self.go(ctx, self.ch + 1, Target::Ratio(0.0), true);
                }
            }
            KeyCode::Char('p') | KeyCode::Left | KeyCode::Char('h') => {
                if self.ch > 0 {
                    self.go(ctx, self.ch - 1, Target::Ratio(0.0), true);
                }
            }
            KeyCode::Char('t') => {
                let mut st = ListState::default();
                let i = self
                    .toc
                    .iter()
                    .rposition(|t| t.index <= self.ch)
                    .unwrap_or(0);
                st.select(if self.toc.is_empty() { None } else { Some(i) });
                self.popup = Some(Popup::Toc(st));
            }
            KeyCode::Char('m') => {
                if !self.ready() {
                    return;
                }
                let label = format!(
                    "{} · {}",
                    util::truncate(&self.chapter_title(), 90),
                    util::fmt_percent(self.percent())
                );
                let loc = self.locator();
                match ctx.core.add_bookmark(self.id(), loc, label) {
                    Ok(_) => fx.info("Yer imi eklendi."),
                    Err(e) => fx.error(e.to_string()),
                }
            }
            KeyCode::Char('M') | KeyCode::Char('\'') => {
                self.popup = Some(Popup::Marks(Marks::load(ctx, self.id())));
            }
            KeyCode::Char('/') => {
                self.popup = Some(Popup::Search(Search {
                    query: String::new(),
                    typing: true,
                    busy: false,
                    hits: Vec::new(),
                    truncated: false,
                    error: None,
                    state: ListState::default(),
                }));
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.width_pref = (self.width_pref + 8).min(MAX_W);
            }
            KeyCode::Char('-') => {
                self.width_pref = self.width_pref.saturating_sub(8).max(MIN_W);
            }
            KeyCode::Char('N') => match self.detail.next_book_id {
                Some(id) => fx.act(Action::ReplaceBook(id)),
                None => fx.info("Bu serinin son kitabı."),
            },
            KeyCode::Char('r') if self.error.is_some() => {
                self.error = None;
                self.requested.remove(&self.ch);
                self.request(ctx, self.ch);
            }
            _ => {}
        }
    }

    fn popup_key(&mut self, ctx: &Ctx, p: Popup, k: KeyEvent, fx: &mut Fx) -> Option<Popup> {
        match p {
            Popup::Toc(mut st) => {
                let n = self.toc.len();
                match k.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('t') => None,
                    KeyCode::Down | KeyCode::Char('j') => {
                        widgets::move_sel(&mut st, n, 1);
                        Some(Popup::Toc(st))
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        widgets::move_sel(&mut st, n, -1);
                        Some(Popup::Toc(st))
                    }
                    KeyCode::PageDown => {
                        widgets::move_sel(&mut st, n, 10);
                        Some(Popup::Toc(st))
                    }
                    KeyCode::PageUp => {
                        widgets::move_sel(&mut st, n, -10);
                        Some(Popup::Toc(st))
                    }
                    KeyCode::Enter => {
                        if let Some(t) = st.selected().and_then(|i| self.toc.get(i)) {
                            let ch = t.index;
                            self.go(ctx, ch, Target::Ratio(0.0), true);
                        }
                        None
                    }
                    _ => Some(Popup::Toc(st)),
                }
            }
            Popup::Marks(mut m) => match m.key(ctx, self.id(), k) {
                MarkAct::None => Some(Popup::Marks(m)),
                MarkAct::Close => None,
                MarkAct::Go(loc) => {
                    if !self.go_locator(ctx, &loc, true, false) {
                        fx.error("Yer imi bu kitapta geçersiz.");
                    }
                    None
                }
            },
            Popup::Suggest(s) => {
                match k.code {
                    KeyCode::Enter | KeyCode::Char('e') => {
                        let id = self.id();
                        let _ = ctx.core.accept_position(id);
                        let loc = ctx
                            .core
                            .get_progress(id)
                            .map(|p| (p.locator, p.percent))
                            .unwrap_or((s.locator.clone(), s.percent));
                        self.session.reset_saved(&loc.0, loc.1);
                        if !self.go_locator(ctx, &loc.0, false, true) {
                            fx.error("Önerilen konum bu kitapta geçersiz.");
                        }
                    }
                    _ => {
                        let _ = ctx.core.decline_position(self.id());
                    }
                }
                None
            }
            Popup::Search(mut s) => {
                if s.typing {
                    match k.code {
                        KeyCode::Esc => return None,
                        KeyCode::Enter => {
                            let q = s.query.trim().to_string();
                            if q.chars().count() < 2 {
                                s.error = Some("En az 2 karakter yazın.".to_string());
                            } else {
                                s.typing = false;
                                s.busy = true;
                                s.error = None;
                                let id = self.id();
                                ctx.spawn(move |core| {
                                    Some(Msg::Search {
                                        book_id: id,
                                        result: core.search_text(id, q),
                                    })
                                });
                            }
                        }
                        _ => {
                            widgets::edit(&mut s.query, k, 64);
                        }
                    }
                    return Some(Popup::Search(s));
                }
                let n = s.hits.len();
                match k.code {
                    KeyCode::Esc | KeyCode::Char('q') => None,
                    KeyCode::Char('/') => {
                        s.typing = true;
                        Some(Popup::Search(s))
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        widgets::move_sel(&mut s.state, n, 1);
                        Some(Popup::Search(s))
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        widgets::move_sel(&mut s.state, n, -1);
                        Some(Popup::Search(s))
                    }
                    KeyCode::Enter => {
                        if let Some(h) = s.state.selected().and_then(|i| s.hits.get(i)) {
                            let (ch, p) = (h.ch, h.p);
                            self.go(ctx, ch, Target::Fraction(p), true);
                        }
                        None
                    }
                    _ => Some(Popup::Search(s)),
                }
            }
        }
    }

    pub fn on_msg(&mut self, ctx: &Ctx, m: Msg, fx: &mut Fx) {
        match m {
            Msg::TextInfo { book_id, result } if book_id == self.id() => match result {
                Ok(info) => {
                    self.count = info.chapter_count.max(1);
                    self.toc = info.toc;
                    if self.ch >= self.count {
                        let last = self.count - 1;
                        self.go(ctx, last, Target::Ratio(0.0), false);
                    }
                }
                Err(CoreError::Unauthorized) => fx.error("Oturum sona erdi."),
                Err(_) => {}
            },
            Msg::Chapter {
                book_id,
                ch,
                result,
            } if book_id == self.id() => match result {
                Ok(blocks) => {
                    self.cache.insert(ch, blocks);
                    if ch == self.ch {
                        self.error = None;
                        self.laid = None;
                        // sonraki bölüm arka planda hazırlanır
                        if ch + 1 < self.count {
                            self.request(ctx, ch + 1);
                        }
                    }
                }
                Err(e) => {
                    self.requested.remove(&ch);
                    if ch == self.ch {
                        self.error = Some(e.to_string());
                    }
                }
            },
            Msg::Pulled { book_id } if book_id == self.id() && !self.suggestion_checked => {
                self.suggestion_checked = true;
                if let Ok(Some(s)) = ctx.core.position_suggestion(book_id) {
                    if self.popup.is_none() {
                        self.popup = Some(Popup::Suggest(s));
                    }
                } else if !self.session.touched() {
                    // yerelde konum yokken sunucununki doğrudan uygulanmış olabilir
                    if let Ok(p) = ctx.core.get_progress(book_id) {
                        if !p.locator.is_empty() && p.locator != self.detail.locator {
                            self.session.reset_saved(&p.locator, p.percent);
                            self.detail.locator = p.locator.clone();
                            self.go_locator(ctx, &p.locator, false, true);
                        }
                    }
                }
            }
            Msg::Search { book_id, result } if book_id == self.id() => {
                if let Some(Popup::Search(s)) = &mut self.popup {
                    s.busy = false;
                    match result {
                        Ok(r) => {
                            s.hits = r.hits;
                            s.truncated = r.truncated;
                            s.state = ListState::default();
                            widgets::clamp_sel(&mut s.state, s.hits.len());
                            if s.hits.is_empty() {
                                s.error = Some("Sonuç yok.".to_string());
                            }
                        }
                        Err(e) => {
                            s.error = Some(e.to_string());
                            s.typing = true;
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

    /// Bölüm hazırsa satırları diz ve bekleyen konumu uygula.
    fn layout(&mut self, width: usize) {
        let Some(blocks) = self.cache.get(&self.ch).cloned() else {
            return;
        };
        if self.laid != Some((self.ch, width)) {
            // aynı bölüm yeniden diziliyorsa (pencere boyutu, genişlik) üstteki satır korunur
            if self.target.is_none() && self.laid.map(|l| l.0) == Some(self.ch) {
                self.target = Some(Target::Line(self.anchor()));
            }
            let laid = wrap::layout(&blocks, width);
            self.lines = laid.lines;
            self.starts = laid.starts;
            self.laid = Some((self.ch, width));
            if self.target.is_none() {
                self.top = 0;
            }
        }
        if let Some(t) = self.target.take() {
            let m = self.max_top();
            self.top = match t {
                Target::Line(c) => wrap::line_of(&self.starts, c).min(m),
                Target::Ratio(p) => (p.clamp(0.0, 1.0) * m as f64).round() as usize,
                Target::Fraction(p) => {
                    let line = (p.clamp(0.0, 1.0) * self.lines.len() as f64) as usize;
                    line.saturating_sub(self.view_h / 3).min(m)
                }
            };
            if self.mark_after {
                self.mark_after = false;
                self.moved();
            }
        }
        self.top = self.top.min(self.max_top());
        if self.pin_pending == Some(self.ch) {
            self.pin_pending = None;
            self.pin = Some((self.ch, self.anchor()));
        }
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &Ctx) {
        let area = f.area();
        let [head, _, body, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let text_w = self
            .width_pref
            .min(body.width.saturating_sub(4))
            .max(body.width.min(MIN_W));
        let text_area = Rect::new(
            body.x + (body.width - text_w) / 2,
            body.y,
            text_w,
            body.height,
        );
        self.view_h = text_area.height as usize;

        reader::header(
            f,
            head,
            &self.detail.series_name,
            &self.detail.book.title,
            &util::truncate(&self.chapter_title(), (area.width / 3) as usize),
        );

        if self.cache.contains_key(&self.ch) {
            self.layout(text_w as usize);
            let end = (self.top + self.view_h).min(self.lines.len());
            let visible = self.lines[self.top.min(end)..end].to_vec();
            f.render_widget(Paragraph::new(Text::from(visible)), text_area);
            self.draw_pin(f, body, text_area);
        } else if let Some(e) = &self.error {
            widgets::message(
                f,
                body,
                vec![
                    Line::styled(e.clone(), Style::default().fg(theme::ERR)),
                    Line::default(),
                    widgets::hints(&[("r", "yeniden dene"), ("n/p", "bölüm"), ("Esc", "geri")]),
                ],
            );
        } else {
            if !self.requested.contains(&self.ch) {
                self.request(ctx, self.ch);
            }
            widgets::message(f, body, vec![Line::styled("Yükleniyor…", theme::dim())]);
        }

        self.draw_footer(f, foot);

        let percent = self.percent();
        match &mut self.popup {
            None => {}
            Some(Popup::Toc(st)) => draw_toc(f, &self.toc, self.ch, st),
            Some(Popup::Marks(m)) => m.draw(f),
            Some(Popup::Search(s)) => draw_search(f, s),
            Some(Popup::Suggest(s)) => reader::draw_suggestion(f, s, percent),
        }
    }

    /// "Kaldığın yer": satırın solundaki boşlukta `▌`; boşluk yoksa satırın ilk hücresi boyanır.
    fn draw_pin(&self, f: &mut Frame, body: Rect, text_area: Rect) {
        let Some((ch, c)) = self.pin else {
            return;
        };
        if ch != self.ch || !self.ready() {
            return;
        }
        let line = wrap::line_of(&self.starts, c);
        if line < self.top || line >= self.top + self.view_h || line >= self.lines.len() {
            return;
        }
        let y = text_area.y + (line - self.top) as u16;
        let gap = text_area.x - body.x;
        let style = Style::default().fg(theme::ACCENT);
        if gap >= 1 {
            let x = text_area.x - gap.min(2);
            f.render_widget(Span::styled("▌", style), Rect::new(x, y, 1, 1));
        } else {
            f.buffer_mut().set_style(
                Rect::new(text_area.x, y, 1, 1),
                Style::default().bg(theme::ACCENT),
            );
        }
    }

    fn draw_footer(&self, f: &mut Frame, area: Rect) {
        let pct = if self.ready() {
            self.percent()
        } else {
            self.detail.book.percent
        };
        let mut spans = vec![Span::styled(
            format!(" Bölüm {}/{} ", self.ch + 1, self.count),
            theme::dim(),
        )];
        spans.extend(widgets::bar(pct, 16));
        spans.push(Span::styled(
            format!(" {} ", util::fmt_percent(pct)),
            Style::default().fg(theme::ACCENT),
        ));
        if self.at_book_end() {
            match &self.detail.next_book_title {
                Some(t) => spans.push(Span::styled(
                    format!("  Kitap bitti · N: «{}»", util::truncate(t, 40)),
                    Style::default().fg(theme::OK),
                )),
                None => spans.push(Span::styled(
                    "  Kitap bitti",
                    Style::default().fg(theme::OK),
                )),
            }
        } else {
            spans.push(Span::raw("  "));
            spans.extend(
                widgets::hints(&[
                    ("␣", "sayfa"),
                    ("t", "içindekiler"),
                    ("m", "yer imi"),
                    ("?", "yardım"),
                ])
                .spans,
            );
        }
        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}

fn draw_toc(f: &mut Frame, toc: &[TocEntry], cur: i32, st: &mut ListState) {
    let h = (toc.len() as u16 + 2).clamp(5, f.area().height.saturating_sub(4));
    let area = widgets::centered(f.area(), 70, h);
    f.render_widget(Clear, area);
    let block = widgets::popup_block("İçindekiler");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let w = inner.width.saturating_sub(4) as usize;
    let items: Vec<ListItem<'static>> = toc
        .iter()
        .map(|t| {
            let style = if t.index == cur {
                Style::default().fg(theme::READING)
            } else {
                Style::default()
            };
            ListItem::new(Line::styled(util::truncate(&t.title, w), style))
        })
        .collect();
    widgets::list(f, inner, items, st);
}

fn draw_search(f: &mut Frame, s: &mut Search) {
    let h = (s.hits.len() as u16 * 2 + 6).clamp(8, f.area().height.saturating_sub(2));
    let area = widgets::centered(f.area(), 80, h);
    f.render_widget(Clear, area);
    let block = widgets::popup_block("Kitapta ara");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [q, msg, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(inner);
    f.render_widget(
        Paragraph::new(widgets::input_line("Ara", &s.query, s.typing, false)),
        q,
    );
    let status = if s.busy {
        Line::styled(" Aranıyor…", Style::default().fg(theme::WARN))
    } else if let Some(e) = &s.error {
        Line::styled(format!(" {e}"), Style::default().fg(theme::ERR))
    } else if !s.hits.is_empty() {
        let more = if s.truncated { "+" } else { "" };
        Line::styled(
            format!(" {}{more} sonuç · Enter: git", s.hits.len()),
            theme::dim(),
        )
    } else {
        Line::styled(" Enter: ara · Esc: kapat · çevrimiçi çalışır", theme::dim())
    };
    f.render_widget(Paragraph::new(status), msg);
    let w = list.width.saturating_sub(4) as usize;
    let items: Vec<ListItem<'static>> = s
        .hits
        .iter()
        .map(|h| {
            let ctx_w = w.saturating_sub(h.hit.chars().count() + 2) / 2;
            let before = util::tail_words(&h.before, ctx_w);
            let after = util::truncate(&h.after, ctx_w);
            ListItem::new(vec![
                Line::styled(
                    format!("Bölüm {} · {}", h.ch + 1, util::fmt_percent(h.p)),
                    theme::dim(),
                ),
                Line::from(vec![
                    Span::raw(before.replace('\n', " ")),
                    Span::styled(h.hit.clone(), theme::title()),
                    Span::raw(after.replace('\n', " ")),
                ]),
            ])
        })
        .collect();
    widgets::list(f, list, items, &mut s.state);
}

fn target_of(l: util::TextLoc) -> Target {
    match l.c {
        Some(c) => Target::Line(c),
        None => Target::Ratio(l.p),
    }
}
