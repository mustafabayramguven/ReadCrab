//! Giriş ekranı.

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::{
    app::{Action, Ctx, Fx, Msg},
    theme, widgets,
};

pub struct Login {
    fields: [String; 3],
    focus: usize,
    busy: bool,
    error: Option<String>,
}

const LABELS: [&str; 3] = ["Sunucu", "Kullanıcı adı", "Parola"];

impl Login {
    pub const HELP: &'static [(&'static str, &'static str)] = &[
        ("Tab / ↓", "sonraki alan"),
        ("Shift+Tab / ↑", "önceki alan"),
        ("Enter", "sonraki alan / giriş yap"),
        ("Ctrl+U", "alanı temizle"),
        ("Esc", "çık"),
        ("Ctrl+C", "çık"),
    ];

    pub fn new(ctx: &Ctx) -> Login {
        let (server, user) = widgets::last_server(&ctx.data_dir);
        let server = if server.is_empty() {
            std::env::var("READCRAB_SERVER").unwrap_or_default()
        } else {
            server
        };
        let focus = if server.is_empty() {
            0
        } else if user.is_empty() {
            1
        } else {
            2
        };
        Login {
            fields: [server, user, String::new()],
            focus,
            busy: false,
            error: None,
        }
    }

    pub fn set_error(&mut self, e: &str) {
        self.error = Some(e.to_string());
    }

    pub fn failed(&mut self, e: String) {
        self.busy = false;
        self.error = Some(e);
        self.fields[2].clear();
        self.focus = 2;
    }

    fn submit(&mut self, ctx: &Ctx, fx: &mut Fx) {
        let [server, user, pass] = &self.fields;
        if server.trim().is_empty() {
            self.focus = 0;
            fx.error("Sunucu adresini girin (ör. https://sunucu.example:8443).");
            return;
        }
        if user.trim().is_empty() {
            self.focus = 1;
            fx.error("Kullanıcı adını girin.");
            return;
        }
        if pass.is_empty() {
            self.focus = 2;
            fx.error("Parolayı girin.");
            return;
        }
        self.busy = true;
        self.error = None;
        let (s, u, p) = (
            server.trim().to_string(),
            user.trim().to_string(),
            pass.clone(),
        );
        ctx.spawn(move |core| Some(Msg::Login(core.login(s, u, p))));
    }

    pub fn key(&mut self, ctx: &Ctx, k: KeyEvent, fx: &mut Fx) {
        if self.busy {
            return;
        }
        match k.code {
            KeyCode::Esc => fx.act(Action::Quit),
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % 3,
            KeyCode::BackTab | KeyCode::Up => self.focus = (self.focus + 2) % 3,
            KeyCode::Enter => {
                if self.focus < 2 {
                    self.focus += 1;
                } else {
                    self.submit(ctx, fx);
                }
            }
            _ => {
                let max = if self.focus == 0 { 200 } else { 128 };
                widgets::edit(&mut self.fields[self.focus], k, max);
            }
        }
    }

    pub fn draw(&mut self, f: &mut Frame) {
        let area = widgets::centered(f.area(), 64, 14);
        let [title, _, a, b, c, _, msg, _, help] = Layout::vertical([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(area);
        f.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("ReadCrab", theme::title()),
                    Span::styled(" · terminal okuyucu", theme::dim()),
                ]),
                Line::styled("Sunucunuza giriş yapın.", theme::dim()),
            ]),
            title,
        );
        for (i, row) in [a, b, c].into_iter().enumerate() {
            f.render_widget(
                Paragraph::new(widgets::input_line(
                    LABELS[i],
                    &self.fields[i],
                    self.focus == i && !self.busy,
                    i == 2,
                )),
                row,
            );
        }
        let status = if self.busy {
            Line::styled("Giriş yapılıyor…", Style::default().fg(theme::WARN))
        } else if let Some(e) = &self.error {
            Line::styled(e.clone(), Style::default().fg(theme::ERR))
        } else {
            Line::styled(
                "http:// yalnız yerel ağ adreslerinde kabul edilir.",
                theme::dim(),
            )
        };
        f.render_widget(
            Paragraph::new(status).wrap(ratatui::widgets::Wrap { trim: true }),
            msg,
        );
        f.render_widget(
            Paragraph::new(widgets::hints(&[
                ("Tab", "alan"),
                ("Enter", "giriş"),
                ("Esc", "çık"),
            ])),
            help,
        );
    }
}
