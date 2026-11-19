//! Uygulama durumu, ekran yığını ve olay döngüsü.
//!
//! Çekirdeğin ağ çağrıları bloklar; hepsi ayrı iş parçacığında çalışır ve sonucu `Msg` olarak
//! kanaldan döner. Yerel veritabanı okumaları (liste, ilerleme) arayüz iş parçacığında yapılır.

use std::{
    path::PathBuf,
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    time::{Duration, Instant},
};

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    DefaultTerminal, Frame,
};
use ratatui_image::{picker::Picker, protocol::Protocol};
use readcrab_core::{
    Core, CoreError, DownloadRun, DownloadState, SearchResults, SessionInfo, StatsSnapshot,
    SyncReport, TextInfo,
};

use crate::{
    comic_reader::ComicReader,
    html::Block,
    library::{Library, SeriesView},
    login::Login,
    stats::StatsView,
    text_reader::TextReader,
    theme, widgets,
};

/// Arka plan işlerinin sonuçları.
pub enum Msg {
    Login(Result<SessionInfo, CoreError>),
    Synced(Result<SyncReport, CoreError>),
    Pushed(Result<i32, CoreError>),
    Downloads(Result<DownloadRun, CoreError>),
    TextInfo {
        book_id: i64,
        result: Result<TextInfo, CoreError>,
    },
    Chapter {
        book_id: i64,
        ch: i32,
        result: Result<Arc<Vec<Block>>, CoreError>,
    },
    Pulled {
        book_id: i64,
    },
    Page {
        book_id: i64,
        page: i32,
        area: Rect,
        result: Result<Protocol, String>,
    },
    Search {
        book_id: i64,
        result: Result<SearchResults, CoreError>,
    },
    Stats {
        year: i32,
        result: Result<StatsSnapshot, CoreError>,
    },
    Enqueued(Result<usize, CoreError>),
}

/// Ekranların paylaştığı bağlam.
#[derive(Clone)]
pub struct Ctx {
    pub core: Arc<Core>,
    pub tx: Sender<Msg>,
    pub data_dir: PathBuf,
    pub picker: Picker,
}

impl Ctx {
    /// `f` ayrı iş parçacığında çalışır; döndürdüğü ileti olay döngüsüne gelir.
    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce(&Core) -> Option<Msg> + Send + 'static,
    {
        let core = self.core.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if let Some(m) = f(&core) {
                let _ = tx.send(m);
            }
        });
    }

    pub fn chapter_file(&self, book_id: i64, ch: i32) -> PathBuf {
        self.data_dir
            .join("books")
            .join(book_id.to_string())
            .join(format!("c{ch}.html"))
    }
}

/// Ekranların uygulamaya bıraktığı istekler.
pub enum Action {
    Back,
    Quit,
    OpenSeries(i64),
    OpenBook(i64),
    /// Okuyucudaki kitabı sonraki ciltle değiştirir (yığına eklemeden).
    ReplaceBook(i64),
    OpenStats,
    Sync,
    AskLogout,
    StartDownloads,
}

#[derive(Default)]
pub struct Fx {
    pub actions: Vec<Action>,
    pub toast: Option<(String, bool)>,
    /// Görsel protokol kalıntıları için tüm ekranı yeniden çiz.
    pub clear: bool,
}

impl Fx {
    pub fn act(&mut self, a: Action) {
        self.actions.push(a);
    }

    pub fn info(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), false));
    }

    pub fn error(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), true));
    }
}

pub enum Screen {
    Login(Login),
    Library(Library),
    Series(SeriesView),
    Text(Box<TextReader>),
    Comic(Box<ComicReader>),
    Stats(StatsView),
}

impl Screen {
    fn is_reader(&self) -> bool {
        matches!(self, Screen::Text(_) | Screen::Comic(_))
    }
}

/// Liste ekranlarının alt çubuğu için durum.
pub struct Status {
    pub syncing: bool,
    pub downloading: bool,
    pub pending: i32,
    pub last_ok_at: Option<i64>,
    pub last_error: Option<String>,
}

struct Toast {
    text: String,
    err: bool,
    at: Instant,
}

pub struct App {
    ctx: Ctx,
    rx: Receiver<Msg>,
    screen: Screen,
    stack: Vec<Screen>,
    toast: Option<Toast>,
    syncing: bool,
    pushing: bool,
    downloading: bool,
    help: bool,
    confirm_logout: bool,
    quit: bool,
    clear: bool,
    last_push_check: Instant,
    last_refresh: Instant,
}

const PUSH_EVERY: Duration = Duration::from_secs(45);

impl App {
    pub fn new(core: Arc<Core>, data_dir: PathBuf, picker: Picker) -> App {
        let (tx, rx) = mpsc::channel();
        let ctx = Ctx {
            core,
            tx,
            data_dir,
            picker,
        };
        let has_session = ctx.core.session().is_some();
        let screen = if has_session {
            Screen::Library(Library::new(&ctx))
        } else {
            Screen::Login(Login::new(&ctx))
        };
        let mut app = App {
            ctx,
            rx,
            screen,
            stack: Vec::new(),
            toast: None,
            syncing: false,
            pushing: false,
            downloading: false,
            help: false,
            confirm_logout: false,
            quit: false,
            clear: false,
            last_push_check: Instant::now(),
            last_refresh: Instant::now(),
        };
        if has_session {
            app.start_sync();
            app.resume_downloads();
        }
        app
    }

    pub fn ctx(&self) -> &Ctx {
        &self.ctx
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.quit {
            if self.clear {
                terminal.clear()?;
                self.clear = false;
            }
            terminal.draw(|f| self.draw(f))?;
            if event::poll(Duration::from_millis(150))? {
                match event::read()? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => self.on_key(k),
                    Event::Resize(..) => self.clear = true,
                    _ => {}
                }
            }
            while let Ok(m) = self.rx.try_recv() {
                self.on_msg(m);
            }
            self.tick();
        }
        self.shutdown();
        Ok(())
    }

    fn status(&self) -> Status {
        let s = self.ctx.core.sync_status();
        Status {
            syncing: self.syncing || self.pushing,
            downloading: self.downloading,
            pending: s.pending_ops,
            last_ok_at: s.last_ok_at,
            last_error: s.last_error,
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let status = match self.screen {
            Screen::Library(_) | Screen::Series(_) => Some(self.status()),
            _ => None,
        };
        let ctx = &self.ctx;
        match &mut self.screen {
            Screen::Login(s) => s.draw(f),
            Screen::Library(s) => s.draw(f, ctx, status.as_ref()),
            Screen::Series(s) => s.draw(f, status.as_ref()),
            Screen::Text(s) => s.draw(f, ctx),
            Screen::Comic(s) => s.draw(f, ctx),
            Screen::Stats(s) => s.draw(f),
        }
        if let Some(t) = &self.toast {
            let area = f.area();
            if area.height > 0 {
                let row = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
                let style = if t.err {
                    Style::default().fg(theme::ERR)
                } else {
                    Style::default().fg(theme::OK)
                };
                f.render_widget(Clear, row);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(format!(" {}", t.text), style))),
                    row,
                );
            }
        }
        if self.help {
            widgets::help_popup(f, self.help_lines());
        }
        if self.confirm_logout {
            widgets::confirm_popup(
                f,
                "Çıkış yapılsın mı?",
                &[
                    "Bu bilgisayardaki indirmeler ve önbellek silinir.",
                    "Eşitlenmemiş ilerleme varsa önce eşitlenmesi denenir.",
                ],
            );
        }
    }

    fn help_lines(&self) -> &'static [(&'static str, &'static str)] {
        match &self.screen {
            Screen::Login(_) => Login::HELP,
            Screen::Library(_) => Library::HELP,
            Screen::Series(_) => SeriesView::HELP,
            Screen::Text(_) => TextReader::HELP,
            Screen::Comic(_) => ComicReader::HELP,
            Screen::Stats(_) => StatsView::HELP,
        }
    }

    fn captures_input(&self) -> bool {
        match &self.screen {
            Screen::Login(_) => true,
            Screen::Library(s) => s.captures_input(),
            Screen::Text(s) => s.captures_input(),
            Screen::Comic(s) => s.captures_input(),
            _ => false,
        }
    }

    fn on_key(&mut self, k: KeyEvent) {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.help {
            self.help = false;
            self.clear = true;
            return;
        }
        if self.confirm_logout {
            self.confirm_logout = false;
            self.clear = true;
            if matches!(
                k.code,
                KeyCode::Char('e' | 'E' | 'y' | 'Y') | KeyCode::Enter
            ) {
                self.logout();
            }
            return;
        }
        if !self.captures_input() && k.code == KeyCode::Char('?') {
            self.help = true;
            return;
        }
        let mut fx = Fx::default();
        let ctx = &self.ctx;
        match &mut self.screen {
            Screen::Login(s) => s.key(ctx, k, &mut fx),
            Screen::Library(s) => s.key(ctx, k, &mut fx),
            Screen::Series(s) => s.key(ctx, k, &mut fx),
            Screen::Text(s) => s.key(ctx, k, &mut fx),
            Screen::Comic(s) => s.key(ctx, k, &mut fx),
            Screen::Stats(s) => s.key(ctx, k, &mut fx),
        }
        self.apply(fx);
    }

    fn apply(&mut self, fx: Fx) {
        if let Some((text, err)) = fx.toast {
            self.show(text, err);
        }
        if fx.clear {
            self.clear = true;
        }
        for a in fx.actions {
            match a {
                Action::Back => self.back(),
                Action::Quit => self.quit = true,
                Action::OpenSeries(id) => match SeriesView::new(&self.ctx, id) {
                    Ok(v) => self.push(Screen::Series(v)),
                    Err(e) => self.show(e.to_string(), true),
                },
                Action::OpenBook(id) => {
                    if let Some(s) = self.open_book(id) {
                        self.push(s);
                    }
                }
                Action::ReplaceBook(id) => {
                    if let Some(s) = self.open_book(id) {
                        let old = std::mem::replace(&mut self.screen, s);
                        self.close_screen(old);
                        self.clear = true;
                    }
                }
                Action::OpenStats => {
                    let v = StatsView::new(&self.ctx);
                    self.push(Screen::Stats(v));
                }
                Action::Sync => self.start_sync(),
                Action::AskLogout => self.confirm_logout = true,
                Action::StartDownloads => self.start_downloads(),
            }
        }
    }

    fn open_book(&mut self, id: i64) -> Option<Screen> {
        let detail = match self.ctx.core.get_book(id) {
            Ok(d) => d,
            Err(e) => {
                self.show(e.to_string(), true);
                return None;
            }
        };
        if let Some(err) = &detail.book.error {
            self.show(format!("Bu kitap açılamıyor: {err}"), true);
            return None;
        }
        Some(if detail.book.is_comic {
            Screen::Comic(Box::new(ComicReader::new(&self.ctx, detail)))
        } else {
            Screen::Text(Box::new(TextReader::new(&self.ctx, detail)))
        })
    }

    fn push(&mut self, s: Screen) {
        let old = std::mem::replace(&mut self.screen, s);
        self.stack.push(old);
        self.clear = true;
    }

    fn back(&mut self) {
        let Some(prev) = self.stack.pop() else {
            self.quit = true;
            return;
        };
        let old = std::mem::replace(&mut self.screen, prev);
        self.close_screen(old);
        self.refresh();
        self.clear = true;
    }

    /// Okuyucu kapanırken ilerleme ve okuma süresi kaydedilir, sonra sunucuya itilir.
    fn close_screen(&mut self, s: Screen) {
        let was_reader = s.is_reader();
        match s {
            Screen::Text(mut r) => r.close(&self.ctx),
            Screen::Comic(mut r) => r.close(&self.ctx),
            _ => {}
        }
        if was_reader {
            self.start_push();
        }
    }

    fn refresh(&mut self) {
        let ctx = &self.ctx;
        match &mut self.screen {
            Screen::Library(s) => s.reload(ctx),
            Screen::Series(s) => s.reload(ctx),
            _ => {}
        }
        for s in &mut self.stack {
            if let Screen::Library(l) = s {
                l.reload(ctx);
            }
        }
    }

    fn show(&mut self, text: String, err: bool) {
        self.toast = Some(Toast {
            text,
            err,
            at: Instant::now(),
        });
    }

    fn start_sync(&mut self) {
        if self.syncing || self.ctx.core.session().is_none() {
            return;
        }
        self.syncing = true;
        self.ctx.spawn(|core| Some(Msg::Synced(core.sync_now())));
    }

    fn start_push(&mut self) {
        if self.pushing || self.syncing || self.ctx.core.session().is_none() {
            return;
        }
        self.pushing = true;
        self.last_push_check = Instant::now();
        self.ctx.spawn(|core| Some(Msg::Pushed(core.sync_push())));
    }

    fn start_downloads(&mut self) {
        if self.downloading {
            return;
        }
        self.downloading = true;
        self.ctx
            .spawn(|core| Some(Msg::Downloads(core.run_downloads())));
    }

    /// Önceki oturumdan kuyrukta kalan indirmeler sürer.
    fn resume_downloads(&mut self) {
        let queued = self
            .ctx
            .core
            .download_infos()
            .map(|v| v.iter().any(|d| d.state == DownloadState::Queued))
            .unwrap_or(false);
        if queued {
            self.start_downloads();
        }
    }

    fn logout(&mut self) {
        // eşitlenmemiş işlemler kaybolmasın diye önce kısa bir itme denemesi
        if self.ctx.core.sync_status().pending_ops > 0 {
            let _ = push_with_timeout(&self.ctx.core, Duration::from_secs(4));
        }
        self.ctx.core.download_stop();
        match self.ctx.core.logout() {
            Ok(()) => {
                self.screen = Screen::Login(Login::new(&self.ctx));
                self.stack.clear();
                self.show("Çıkış yapıldı.".to_string(), false);
            }
            Err(e) => self.show(e.to_string(), true),
        }
    }

    /// 401: giriş ekranına dönülür. Aynı sunucu ve kullanıcıyla girilirse yerel veri korunur.
    fn session_expired(&mut self) {
        if matches!(self.screen, Screen::Login(_)) {
            return;
        }
        let mut login = Login::new(&self.ctx);
        login.set_error("Oturum sona erdi; yeniden giriş yapın. Yerel veriler korunur.");
        let old = std::mem::replace(&mut self.screen, Screen::Login(login));
        // itme denenmez: aynı 401 yeniden gelir
        match old {
            Screen::Text(mut r) => r.close(&self.ctx),
            Screen::Comic(mut r) => r.close(&self.ctx),
            _ => {}
        }
        self.stack.clear();
        self.clear = true;
    }

    fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::Login(r) => match r {
                Ok(info) => {
                    widgets::remember_server(&self.ctx.data_dir, &info.server_url, &info.username);
                    self.screen = Screen::Library(Library::new(&self.ctx));
                    self.stack.clear();
                    self.show(
                        format!("Hoş geldin, {}. Kütüphane eşitleniyor…", info.username),
                        false,
                    );
                    self.start_sync();
                    self.resume_downloads();
                }
                Err(e) => {
                    if let Screen::Login(l) = &mut self.screen {
                        l.failed(e.to_string());
                    }
                }
            },
            Msg::Synced(r) => {
                self.syncing = false;
                match r {
                    Ok(rep) => {
                        self.refresh();
                        if rep.pushed > 0 {
                            self.show(
                                format!("Eşitlendi ({} değişiklik gönderildi).", rep.pushed),
                                false,
                            );
                        }
                    }
                    Err(CoreError::Unauthorized) => self.session_expired(),
                    Err(e) => self.show(format!("Eşitleme başarısız: {e}"), true),
                }
            }
            Msg::Pushed(r) => {
                self.pushing = false;
                if let Err(CoreError::Unauthorized) = r {
                    self.session_expired();
                }
            }
            Msg::Downloads(r) => {
                self.downloading = false;
                self.refresh();
                match r {
                    Ok(run) if run.failed > 0 => self.show(
                        format!(
                            "İndirme bitti: {} tamam, {} hatalı.",
                            run.finished, run.failed
                        ),
                        true,
                    ),
                    Ok(run) if run.finished > 0 => {
                        self.show(format!("İndirme bitti: {} kitap.", run.finished), false)
                    }
                    Ok(_) => {}
                    Err(e) => self.show(format!("İndirme durdu: {e}"), true),
                }
                self.resume_downloads();
            }
            Msg::Enqueued(r) => match r {
                Ok(0) => self.show("Seride indirilecek kitap kalmadı.".to_string(), false),
                Ok(n) => {
                    self.show(format!("{n} kitap indirme kuyruğuna eklendi."), false);
                    self.start_downloads();
                    self.refresh();
                }
                Err(e) => self.show(e.to_string(), true),
            },
            other => {
                let mut fx = Fx::default();
                let ctx = &self.ctx;
                match &mut self.screen {
                    Screen::Text(s) => s.on_msg(ctx, other, &mut fx),
                    Screen::Comic(s) => s.on_msg(ctx, other, &mut fx),
                    Screen::Stats(s) => s.on_msg(other),
                    _ => {}
                }
                self.apply(fx);
            }
        }
    }

    fn tick(&mut self) {
        if let Some(t) = &self.toast {
            let ttl = if t.err { 7 } else { 4 };
            if t.at.elapsed() > Duration::from_secs(ttl) {
                self.toast = None;
            }
        }
        let ctx = &self.ctx;
        match &mut self.screen {
            Screen::Text(s) => s.tick(ctx),
            Screen::Comic(s) => s.tick(ctx),
            _ => {}
        }
        if self.downloading && self.last_refresh.elapsed() > Duration::from_secs(1) {
            self.last_refresh = Instant::now();
            self.refresh();
        }
        if self.last_push_check.elapsed() > PUSH_EVERY {
            self.last_push_check = Instant::now();
            if self.ctx.core.sync_status().pending_ops > 0 {
                self.start_push();
            }
        }
    }

    fn shutdown(&mut self) {
        let old = std::mem::replace(&mut self.screen, Screen::Login(Login::new(&self.ctx)));
        if let Screen::Text(mut r) = old {
            r.close(&self.ctx);
        } else if let Screen::Comic(mut r) = old {
            r.close(&self.ctx);
        }
        self.ctx.core.download_stop();
    }
}

/// Çıkışta bekleyen işlemleri sunucuya göndermeyi dener; ağ yoksa en çok `limit` bekler.
/// Veri yerelde güvende; bir sonraki açılışta yeniden denenir.
pub fn push_with_timeout(core: &Arc<Core>, limit: Duration) -> Option<Result<i32, CoreError>> {
    if core.session().is_none() || core.sync_status().pending_ops == 0 {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    let c = core.clone();
    std::thread::spawn(move || {
        let _ = tx.send(c.sync_push());
    });
    rx.recv_timeout(limit).ok()
}
