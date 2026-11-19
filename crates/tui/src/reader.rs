//! Metin ve çizgi roman okuyucularının ortak parçaları: ilerleme kaydı, okuma süresi
//! (istatistik/seri için), yer imi listesi ve "başka cihaz konumu" önerisi.

use std::time::{Duration, Instant};

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Clear, ListItem, ListState, Paragraph},
    Frame,
};
use readcrab_core::{BookmarkItem, PositionSuggestion};

use crate::{app::Ctx, theme, util, widgets};

/// Son tuştan bu kadar sonra okuma süresi saymayı bırakır.
const IDLE: Duration = Duration::from_secs(120);
/// İlerleme, konum değiştikten bu kadar sonra yerel veritabanına yazılır.
const SAVE_AFTER: Duration = Duration::from_millis(800);
const PING_EVERY: Duration = Duration::from_secs(15);

pub struct Session {
    pub book_id: i64,
    ping: Option<String>,
    active: Duration,
    last_input: Instant,
    last_tick: Instant,
    last_ping: Instant,
    pages: i64,
    percent: f64,
    pending: Option<(String, f64, Instant)>,
    saved: String,
    touched: bool,
}

impl Session {
    pub fn start(ctx: &Ctx, book_id: i64, percent: f64, locator: &str) -> Session {
        let ping = ctx.core.ping_start(book_id, percent, util::today()).ok();
        let now = Instant::now();
        Session {
            book_id,
            ping,
            active: Duration::ZERO,
            last_input: now,
            last_tick: now,
            last_ping: now,
            pages: 0,
            percent,
            pending: None,
            saved: locator.to_string(),
            touched: false,
        }
    }

    pub fn input(&mut self) {
        self.last_input = Instant::now();
    }

    /// İleri sayfa/bölüm geçişi.
    pub fn page_turned(&mut self) {
        self.pages += 1;
    }

    /// Kullanıcı konumu değiştirdi.
    pub fn mark(&mut self, locator: String, percent: f64) {
        self.percent = percent;
        self.touched = true;
        if locator == self.saved && self.pending.is_none() {
            return;
        }
        self.pending = Some((locator, percent, Instant::now()));
    }

    /// Kullanıcı açılıştan beri konumu değiştirdi mi.
    pub fn touched(&self) -> bool {
        self.touched
    }

    /// Dışarıdan uygulanan konum (ör. başka cihaz önerisi kabul edildi): tekrar kaydedilmez.
    pub fn reset_saved(&mut self, locator: &str, percent: f64) {
        self.saved = locator.to_string();
        self.percent = percent;
        self.pending = None;
    }

    pub fn tick(&mut self, ctx: &Ctx) {
        let now = Instant::now();
        let dt = now - self.last_tick;
        self.last_tick = now;
        if now - self.last_input < IDLE {
            self.active += dt;
        }
        if let Some((_, _, at)) = &self.pending {
            if now - *at >= SAVE_AFTER {
                self.flush(ctx);
            }
        }
        if now - self.last_ping >= PING_EVERY {
            self.update_ping(ctx);
        }
    }

    fn update_ping(&mut self, ctx: &Ctx) {
        self.last_ping = Instant::now();
        if let Some(id) = &self.ping {
            let _ = ctx.core.ping_update(
                id.clone(),
                self.active.as_secs() as i64,
                self.pages,
                self.percent,
            );
        }
    }

    pub fn flush(&mut self, ctx: &Ctx) {
        if let Some((loc, pct, _)) = self.pending.take() {
            match ctx
                .core
                .save_progress(self.book_id, loc.clone(), pct, util::today())
            {
                Ok(()) => self.saved = loc,
                // geçersiz değer: bir sonraki harekette yeniden denenir
                Err(_) => self.saved.clear(),
            }
        }
    }

    pub fn close(&mut self, ctx: &Ctx) {
        self.flush(ctx);
        self.update_ping(ctx);
        if let Some(id) = self.ping.take() {
            let _ = ctx.core.ping_close(id);
        }
    }
}

/// Yer imi listesi açılır penceresi.
pub struct Marks {
    pub items: Vec<BookmarkItem>,
    pub state: ListState,
}

pub enum MarkAct {
    None,
    Close,
    Go(String),
}

impl Marks {
    pub fn load(ctx: &Ctx, book_id: i64) -> Marks {
        let items = ctx.core.list_bookmarks(book_id).unwrap_or_default();
        let mut state = ListState::default();
        widgets::clamp_sel(&mut state, items.len());
        Marks { items, state }
    }

    pub fn key(&mut self, ctx: &Ctx, book_id: i64, k: KeyEvent) -> MarkAct {
        let n = self.items.len();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('M') => MarkAct::Close,
            KeyCode::Down | KeyCode::Char('j') => {
                widgets::move_sel(&mut self.state, n, 1);
                MarkAct::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                widgets::move_sel(&mut self.state, n, -1);
                MarkAct::None
            }
            KeyCode::Enter => match self.state.selected().and_then(|i| self.items.get(i)) {
                Some(b) => MarkAct::Go(b.locator.clone()),
                None => MarkAct::None,
            },
            KeyCode::Char('d') | KeyCode::Delete => {
                if let Some(b) = self.state.selected().and_then(|i| self.items.get(i)) {
                    let _ = ctx.core.delete_bookmark(b.local_id);
                    *self = Marks::load(ctx, book_id);
                }
                MarkAct::None
            }
            _ => MarkAct::None,
        }
    }

    pub fn draw(&mut self, f: &mut Frame) {
        let area = widgets::centered(f.area(), 64, (self.items.len() as u16 + 4).clamp(6, 22));
        f.render_widget(Clear, area);
        let block = widgets::popup_block("Yer imleri");
        let inner = block.inner(area);
        f.render_widget(block, area);
        if self.items.is_empty() {
            widgets::message(
                f,
                inner,
                vec![Line::styled(
                    "Yer imi yok. Okurken m ile ekleyin.",
                    theme::dim(),
                )],
            );
            return;
        }
        let [list, foot] = ratatui::layout::Layout::vertical([
            ratatui::layout::Constraint::Min(1),
            ratatui::layout::Constraint::Length(1),
        ])
        .areas(inner);
        let w = list.width.saturating_sub(4) as usize;
        let items: Vec<ListItem<'static>> = self
            .items
            .iter()
            .map(|b| {
                let mut spans = vec![Span::raw(util::truncate(&b.label, w.saturating_sub(14)))];
                spans.push(Span::styled(
                    format!("  {}", util::fmt_stamp(b.created_at)),
                    theme::dim(),
                ));
                if b.pending {
                    spans.push(Span::styled(" ↑", Style::default().fg(theme::WARN)));
                }
                ListItem::new(Line::from(spans))
            })
            .collect();
        widgets::list(f, list, items, &mut self.state);
        f.render_widget(
            Paragraph::new(widgets::hints(&[
                ("Enter", "git"),
                ("d", "sil"),
                ("Esc", "kapat"),
            ])),
            foot,
        );
    }
}

pub fn draw_suggestion(f: &mut Frame, s: &PositionSuggestion, here: f64) {
    let lines = vec![
        Line::raw(format!(
            " Başka bir cihazda {} konumunda kalmışsınız ({}).",
            util::fmt_percent(s.percent),
            util::fmt_stamp(s.updated_at)
        )),
        Line::raw(format!(
            " Bu bilgisayardaki konum: {}.",
            util::fmt_percent(here)
        )),
        Line::default(),
        widgets::hints(&[("Enter", "oraya git"), ("Esc", "burada kal")]),
    ];
    let area = widgets::centered(f.area(), 62, 6);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(lines).block(widgets::popup_block("Okuma konumu")),
        area,
    );
}

/// Okuyucu üst çubuğu: seri › kitap · ek bilgi.
pub fn header(f: &mut Frame, area: Rect, series: &str, title: &str, extra: &str) {
    let w = area.width as usize;
    let extra_w = extra.chars().count() + 3;
    let left = util::truncate(&format!(" {series} › {title}"), w.saturating_sub(extra_w));
    let line = Line::from(vec![
        Span::styled(left, theme::dim()),
        Span::styled(format!("  {extra}"), Style::default().fg(theme::ACCENT)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}
