//! Kütüphane (seriler, okumaya devam) ve seri ayrıntısı ekranları.

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{ListItem, ListState, Paragraph, Tabs},
    Frame,
};
use readcrab_core::{
    BookItem, ContinueItem, CoreError, DownloadState, ReadStatus, SeriesDetail, SeriesItem,
};

use crate::{
    app::{Action, Ctx, Fx, Msg, Status},
    theme, util, widgets,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Continue,
    Series,
    Downloaded,
}

const TABS: [Tab; 3] = [Tab::Continue, Tab::Series, Tab::Downloaded];

pub struct Library {
    tab: Tab,
    query: String,
    typing: bool,
    series: Vec<SeriesItem>,
    cont: Vec<ContinueItem>,
    state: ListState,
    user: String,
    server: String,
}

impl Library {
    pub const HELP: &'static [(&'static str, &'static str)] = &[
        ("Tab / 1 2 3", "sekme: devam / seriler / indirilenler"),
        ("↑ ↓ / j k", "seç"),
        ("g / G", "başa / sona"),
        ("Enter / l", "aç"),
        ("/", "seri veya kitap ara"),
        ("d", "seriyi (ya da kitabı) indir"),
        ("r", "şimdi eşitle"),
        ("s", "okuma istatistikleri"),
        ("L", "çıkış yap"),
        ("q", "kapat"),
    ];

    pub fn new(ctx: &Ctx) -> Library {
        let (user, server) = ctx
            .core
            .session()
            .map(|s| (s.username, s.server_url))
            .unwrap_or_default();
        let mut l = Library {
            tab: Tab::Series,
            query: String::new(),
            typing: false,
            series: Vec::new(),
            cont: Vec::new(),
            state: ListState::default(),
            user,
            server,
        };
        l.reload(ctx);
        if !l.cont.is_empty() {
            l.tab = Tab::Continue;
            widgets::clamp_sel(&mut l.state, l.cont.len());
        }
        l
    }

    pub fn captures_input(&self) -> bool {
        self.typing
    }

    /// Yalnız yerel veritabanı; ağ yok.
    pub fn reload(&mut self, ctx: &Ctx) {
        self.cont = ctx.core.continue_reading().unwrap_or_default();
        let q = self.query.to_lowercase();
        if !q.is_empty() {
            self.cont.retain(|c| {
                c.title.to_lowercase().contains(&q) || c.series_name.to_lowercase().contains(&q)
            });
        }
        if self.tab != Tab::Continue {
            self.series = ctx
                .core
                .list_series(self.query.clone(), self.tab == Tab::Downloaded)
                .unwrap_or_default();
        }
        let n = self.len();
        widgets::clamp_sel(&mut self.state, n);
    }

    fn len(&self) -> usize {
        match self.tab {
            Tab::Continue => self.cont.len(),
            _ => self.series.len(),
        }
    }

    fn set_tab(&mut self, ctx: &Ctx, t: Tab) {
        if self.tab != t {
            self.tab = t;
            self.state.select(Some(0));
            self.reload(ctx);
        }
    }

    pub fn key(&mut self, ctx: &Ctx, k: KeyEvent, fx: &mut Fx) {
        if self.typing {
            match k.code {
                KeyCode::Esc => {
                    self.typing = false;
                    self.query.clear();
                    self.reload(ctx);
                }
                KeyCode::Enter | KeyCode::Down | KeyCode::Up => {
                    self.typing = false;
                }
                _ => {
                    if widgets::edit(&mut self.query, k, 64) {
                        self.state.select(Some(0));
                        self.reload(ctx);
                    }
                }
            }
            return;
        }
        let n = self.len();
        match k.code {
            KeyCode::Char('q') => fx.act(Action::Quit),
            KeyCode::Esc if !self.query.is_empty() => {
                self.query.clear();
                self.reload(ctx);
            }
            KeyCode::Tab => {
                let i = TABS.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.set_tab(ctx, TABS[(i + 1) % TABS.len()]);
            }
            KeyCode::BackTab => {
                let i = TABS.iter().position(|t| *t == self.tab).unwrap_or(0);
                self.set_tab(ctx, TABS[(i + TABS.len() - 1) % TABS.len()]);
            }
            KeyCode::Char('1') => self.set_tab(ctx, Tab::Continue),
            KeyCode::Char('2') => self.set_tab(ctx, Tab::Series),
            KeyCode::Char('3') => self.set_tab(ctx, Tab::Downloaded),
            KeyCode::Down | KeyCode::Char('j') => widgets::move_sel(&mut self.state, n, 1),
            KeyCode::Up | KeyCode::Char('k') => widgets::move_sel(&mut self.state, n, -1),
            KeyCode::PageDown => widgets::move_sel(&mut self.state, n, 10),
            KeyCode::PageUp => widgets::move_sel(&mut self.state, n, -10),
            KeyCode::Home | KeyCode::Char('g') => {
                widgets::move_sel(&mut self.state, n, -(n as isize))
            }
            KeyCode::End | KeyCode::Char('G') => widgets::move_sel(&mut self.state, n, n as isize),
            KeyCode::Char('/') => self.typing = true,
            KeyCode::Char('r') => fx.act(Action::Sync),
            KeyCode::Char('s') => fx.act(Action::OpenStats),
            KeyCode::Char('L') => fx.act(Action::AskLogout),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                let Some(i) = self.state.selected() else {
                    return;
                };
                match self.tab {
                    Tab::Continue => {
                        if let Some(c) = self.cont.get(i) {
                            fx.act(Action::OpenBook(c.book_id));
                        }
                    }
                    _ => {
                        if let Some(s) = self.series.get(i) {
                            fx.act(Action::OpenSeries(s.id));
                        }
                    }
                }
            }
            KeyCode::Char('d') => {
                let Some(i) = self.state.selected() else {
                    return;
                };
                match self.tab {
                    Tab::Continue => {
                        if let Some(c) = self.cont.get(i) {
                            match ctx.core.download_enqueue(vec![c.book_id]) {
                                Ok(()) => {
                                    fx.info(format!("«{}» indirme kuyruğunda.", c.title));
                                    fx.act(Action::StartDownloads);
                                }
                                Err(e) => fx.error(e.to_string()),
                            }
                        }
                    }
                    _ => {
                        if let Some(s) = self.series.get(i) {
                            enqueue_series(ctx, s.id);
                            fx.info(format!("«{}» kuyruğa ekleniyor…", s.name));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn draw(&mut self, f: &mut Frame, _ctx: &Ctx, status: Option<&Status>) {
        let area = f.area();
        let show_query = self.typing || !self.query.is_empty();
        let [head, tabs, query, body, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(u16::from(show_query)),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(area);

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(" ReadCrab", theme::title()),
                Span::styled(format!("  {} · {}", self.user, self.server), theme::dim()),
            ])),
            head,
        );
        let titles = vec![" Okumaya devam ", " Seriler ", " İndirilenler "];
        let sel = TABS.iter().position(|t| *t == self.tab).unwrap_or(0);
        f.render_widget(
            Tabs::new(titles)
                .select(sel)
                .style(theme::dim())
                .highlight_style(theme::title().add_modifier(Modifier::UNDERLINED))
                .divider("│"),
            tabs,
        );
        if show_query {
            f.render_widget(
                Paragraph::new(widgets::input_line("Ara", &self.query, self.typing, false)),
                query,
            );
        }

        let w = body.width.saturating_sub(2) as usize;
        let items: Vec<ListItem<'static>> = match self.tab {
            Tab::Continue => self.cont.iter().map(|c| continue_row(c, w)).collect(),
            _ => self.series.iter().map(|s| series_row(s, w)).collect(),
        };
        if items.is_empty() {
            let msg = match (self.tab, self.query.is_empty()) {
                (_, false) => "Aramayla eşleşen bir şey yok.",
                (Tab::Continue, _) => "Yarım kalan kitap yok. Seriler sekmesinden bir kitap açın.",
                (Tab::Downloaded, _) => "İndirilmiş kitap yok. Seride d ile indirebilirsiniz.",
                (Tab::Series, _) => "Kütüphane boş. r ile eşitleyin.",
            };
            widgets::message(f, body, vec![Line::styled(msg, theme::dim())]);
        } else {
            widgets::list(f, body, items, &mut self.state);
        }

        let mut line = status.map(widgets::status_line).unwrap_or_default();
        line.spans.push(Span::raw("   "));
        line.spans.extend(
            widgets::hints(&[
                ("Enter", "aç"),
                ("/", "ara"),
                ("d", "indir"),
                ("?", "yardım"),
            ])
            .spans,
        );
        f.render_widget(Paragraph::new(line), foot);
    }
}

/// Serinin indirilmemiş kitaplarını kuyruğa ekler (seri ayrıntısı yerelde; ağ yok).
fn enqueue_series(ctx: &Ctx, series_id: i64) {
    ctx.spawn(move |core| {
        let r = (|| -> Result<usize, CoreError> {
            let d = core.get_series(series_id)?;
            let ids: Vec<i64> = d
                .books
                .iter()
                .filter(|b| b.error.is_none() && b.download.state != DownloadState::Done)
                .map(|b| b.id)
                .collect();
            let n = ids.len();
            if n > 0 {
                core.download_enqueue(ids)?;
            }
            Ok(n)
        })();
        Some(Msg::Enqueued(r))
    });
}

fn pad_to(s: &str, w: usize) -> String {
    use unicode_width::UnicodeWidthStr;
    let t = util::truncate(s, w);
    let tw = t.width();
    format!("{t}{}", " ".repeat(w.saturating_sub(tw)))
}

fn series_row(s: &SeriesItem, w: usize) -> ListItem<'static> {
    // ad | okunan/toplam | çubuk | yüzde | indirilen
    let right = 30;
    let name_w = w.saturating_sub(right + 2).max(10);
    let mut spans = vec![Span::raw(pad_to(&s.name, name_w))];
    spans.push(Span::styled(
        format!(" {:>4}/{:<4} ", s.read_count, s.book_count),
        theme::dim(),
    ));
    spans.extend(widgets::bar(s.percent, 10));
    spans.push(Span::raw(format!(" {:>4}", util::fmt_percent(s.percent))));
    if s.downloaded_count > 0 {
        spans.push(Span::styled(
            format!("  ⬇{}", s.downloaded_count),
            Style::default().fg(theme::READING),
        ));
    }
    ListItem::new(Line::from(spans))
}

fn continue_row(c: &ContinueItem, w: usize) -> ListItem<'static> {
    let right = 18;
    let avail = w.saturating_sub(right + 2).max(12);
    let title_w = (avail * 3 / 5).max(8);
    let series_w = avail.saturating_sub(title_w + 1);
    let mut spans = vec![
        Span::raw(pad_to(&c.title, title_w)),
        Span::raw(" "),
        Span::styled(pad_to(&c.series_name, series_w), theme::dim()),
        Span::raw(" "),
    ];
    spans.extend(widgets::bar(c.percent, 10));
    spans.push(Span::raw(format!(" {:>4}", util::fmt_percent(c.percent))));
    if c.is_comic {
        spans.push(Span::styled(" ▣", theme::dim()));
    }
    ListItem::new(Line::from(spans))
}

pub struct SeriesView {
    id: i64,
    detail: SeriesDetail,
    state: ListState,
}

impl SeriesView {
    pub const HELP: &'static [(&'static str, &'static str)] = &[
        ("↑ ↓ / j k", "seç"),
        ("Enter / l", "oku"),
        ("d", "kitabı indir / indirmeyi sil"),
        ("D", "serinin tamamını indir"),
        ("f", "bitti olarak işaretle / geri al"),
        ("x", "bırakıldı olarak işaretle / geri al"),
        ("r", "şimdi eşitle"),
        ("Esc / q / h", "geri"),
    ];

    pub fn new(ctx: &Ctx, id: i64) -> Result<SeriesView, CoreError> {
        let detail = ctx.core.get_series(id)?;
        let mut state = ListState::default();
        // ilk okunmamış/okunmakta olan kitap seçili gelir
        let first = detail
            .books
            .iter()
            .position(|b| b.status == ReadStatus::Reading)
            .or_else(|| {
                detail
                    .books
                    .iter()
                    .position(|b| b.status == ReadStatus::Unread)
            })
            .unwrap_or(0);
        state.select(if detail.books.is_empty() {
            None
        } else {
            Some(first)
        });
        Ok(SeriesView { id, detail, state })
    }

    pub fn reload(&mut self, ctx: &Ctx) {
        if let Ok(d) = ctx.core.get_series(self.id) {
            self.detail = d;
        }
        widgets::clamp_sel(&mut self.state, self.detail.books.len());
    }

    fn selected(&self) -> Option<&BookItem> {
        self.state.selected().and_then(|i| self.detail.books.get(i))
    }

    pub fn key(&mut self, ctx: &Ctx, k: KeyEvent, fx: &mut Fx) {
        let n = self.detail.books.len();
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('h') | KeyCode::Left => {
                fx.act(Action::Back)
            }
            KeyCode::Down | KeyCode::Char('j') => widgets::move_sel(&mut self.state, n, 1),
            KeyCode::Up | KeyCode::Char('k') => widgets::move_sel(&mut self.state, n, -1),
            KeyCode::PageDown => widgets::move_sel(&mut self.state, n, 10),
            KeyCode::PageUp => widgets::move_sel(&mut self.state, n, -10),
            KeyCode::Home | KeyCode::Char('g') => {
                widgets::move_sel(&mut self.state, n, -(n as isize))
            }
            KeyCode::End | KeyCode::Char('G') => widgets::move_sel(&mut self.state, n, n as isize),
            KeyCode::Char('r') => fx.act(Action::Sync),
            KeyCode::Enter | KeyCode::Char('l') | KeyCode::Right => {
                if let Some(b) = self.selected() {
                    fx.act(Action::OpenBook(b.id));
                }
            }
            KeyCode::Char('d') => {
                let Some(b) = self.selected().cloned() else {
                    return;
                };
                let r = match b.download.state {
                    DownloadState::None | DownloadState::Failed => {
                        ctx.core.download_enqueue(vec![b.id]).map(|_| {
                            fx.info(format!("«{}» indirme kuyruğunda.", b.title));
                            fx.act(Action::StartDownloads);
                        })
                    }
                    _ => ctx.core.download_remove(vec![b.id]).map(|_| {
                        fx.info(format!("«{}» bu bilgisayardan silindi.", b.title));
                    }),
                };
                if let Err(e) = r {
                    fx.error(e.to_string());
                }
                self.reload(ctx);
            }
            KeyCode::Char('D') => {
                enqueue_series(ctx, self.id);
                fx.info("Seri indirme kuyruğuna ekleniyor…");
            }
            KeyCode::Char('f') | KeyCode::Char('x') => {
                let Some(b) = self.selected().cloned() else {
                    return;
                };
                let target = if k.code == KeyCode::Char('f') {
                    ReadStatus::Finished
                } else {
                    ReadStatus::Dropped
                };
                let new = if b.status == target {
                    if b.percent > 0.0 && target == ReadStatus::Dropped {
                        ReadStatus::Reading
                    } else {
                        ReadStatus::Unread
                    }
                } else {
                    target
                };
                if let Err(e) = ctx.core.set_status(b.id, new, util::today()) {
                    fx.error(e.to_string());
                }
                self.reload(ctx);
            }
            _ => {}
        }
    }

    pub fn draw(&mut self, f: &mut Frame, status: Option<&Status>) {
        let area = f.area();
        let [head, body, foot] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(area);
        let done = self
            .detail
            .books
            .iter()
            .filter(|b| b.status == ReadStatus::Finished)
            .count();
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {}", self.detail.name), theme::title()),
                Span::styled(
                    format!("  {done}/{} okundu", self.detail.books.len()),
                    theme::dim(),
                ),
            ])),
            head,
        );
        let w = body.width.saturating_sub(2) as usize;
        let items: Vec<ListItem<'static>> =
            self.detail.books.iter().map(|b| book_row(b, w)).collect();
        if items.is_empty() {
            widgets::message(
                f,
                body,
                vec![Line::styled("Bu seride kitap yok.", theme::dim())],
            );
        } else {
            widgets::list(f, body, items, &mut self.state);
        }
        let mut line = status.map(widgets::status_line).unwrap_or_default();
        line.spans.push(Span::raw("   "));
        line.spans.extend(
            widgets::hints(&[
                ("Enter", "oku"),
                ("d", "indir"),
                ("f", "bitti"),
                ("Esc", "geri"),
                ("?", "yardım"),
            ])
            .spans,
        );
        f.render_widget(Paragraph::new(line), foot);
    }
}

fn status_icon(b: &BookItem) -> Span<'static> {
    match b.status {
        ReadStatus::Unread => Span::styled("○ ", theme::dim()),
        ReadStatus::Reading => Span::styled("◐ ", Style::default().fg(theme::READING)),
        ReadStatus::Finished => Span::styled("● ", Style::default().fg(theme::OK)),
        ReadStatus::Dropped => Span::styled("✗ ", Style::default().fg(theme::WARN)),
    }
}

fn download_badge(b: &BookItem) -> Span<'static> {
    let d = &b.download;
    match d.state {
        DownloadState::None => Span::raw(""),
        DownloadState::Queued => Span::styled(" ⋯ sırada", theme::dim()),
        DownloadState::Running => {
            let p = if d.parts_total > 0 {
                d.parts_done as f64 / d.parts_total as f64
            } else {
                0.0
            };
            Span::styled(
                format!(" ↓ {}", util::fmt_percent(p)),
                Style::default().fg(theme::READING),
            )
        }
        DownloadState::Failed => Span::styled(" ! indirilemedi", Style::default().fg(theme::ERR)),
        DownloadState::Done => Span::styled(" ⬇", Style::default().fg(theme::READING)),
    }
}

fn book_row(b: &BookItem, w: usize) -> ListItem<'static> {
    let right = 34;
    let title_w = w.saturating_sub(right + 2).max(10);
    let mut spans = vec![status_icon(b)];
    if b.error.is_some() {
        spans.push(Span::styled(
            pad_to(&b.title, title_w),
            Style::default()
                .fg(theme::ERR)
                .add_modifier(Modifier::CROSSED_OUT),
        ));
    } else {
        spans.push(Span::raw(pad_to(&b.title, title_w)));
    }
    spans.push(Span::styled(
        format!(
            " {:<4} {:>8} ",
            b.format.to_uppercase(),
            util::fmt_bytes(b.size)
        ),
        theme::dim(),
    ));
    spans.extend(widgets::bar(b.percent, 8));
    spans.push(Span::raw(format!(" {:>4}", util::fmt_percent(b.percent))));
    spans.push(download_badge(b));
    ListItem::new(Line::from(spans))
}
