//! Ortak çizim yardımcıları ve küçük ayar dosyası.

use std::path::Path;

use ratatui::{
    layout::{Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::{app::Status, theme, util};

/// Alanın ortasında `w`×`h` (sınırlanmış) dikdörtgen.
pub fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let [v] = Layout::vertical([Constraint::Length(h.min(area.height))])
        .flex(Flex::Center)
        .areas(area);
    let [r] = Layout::horizontal([Constraint::Length(w.min(area.width))])
        .flex(Flex::Center)
        .areas(v);
    r
}

pub fn popup_block(title: &str) -> Block<'static> {
    Block::bordered()
        .border_style(Style::default().fg(theme::ACCENT))
        .title(Span::styled(format!(" {title} "), theme::title()))
}

/// `[tuş] açıklama` dizisi.
pub fn hints(items: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (k, d)) in items.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ", theme::dim()));
        }
        spans.push(Span::styled(k.to_string(), theme::key()));
        spans.push(Span::styled(format!(" {d}"), theme::dim()));
    }
    Line::from(spans)
}

/// `━━━━────` biçiminde ilerleme çubuğu.
pub fn bar(p: f64, width: usize) -> Vec<Span<'static>> {
    let p = p.clamp(0.0, 1.0);
    let filled = (p * width as f64).round() as usize;
    vec![
        Span::styled("━".repeat(filled), Style::default().fg(theme::ACCENT)),
        Span::styled("─".repeat(width - filled.min(width)), theme::dim()),
    ]
}

pub fn status_line(s: &Status) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    if s.syncing {
        spans.push(Span::styled(
            "↻ eşitleniyor",
            Style::default().fg(theme::WARN),
        ));
    } else if let Some(e) = &s.last_error {
        spans.push(Span::styled(
            format!("✗ {}", util::truncate(e, 48)),
            Style::default().fg(theme::ERR),
        ));
    } else if let Some(t) = s.last_ok_at {
        spans.push(Span::styled(
            format!("✓ eşitlendi {}", util::fmt_stamp(t)),
            Style::default().fg(theme::OK),
        ));
    } else {
        spans.push(Span::styled("henüz eşitlenmedi", theme::dim()));
    }
    if s.pending > 0 {
        spans.push(Span::styled(
            format!(" · {} bekleyen", s.pending),
            Style::default().fg(theme::WARN),
        ));
    }
    if s.downloading {
        spans.push(Span::styled(
            " · ⬇ indiriliyor",
            Style::default().fg(theme::READING),
        ));
    }
    Line::from(spans)
}

pub fn help_popup(f: &mut Frame, lines: &[(&str, &str)]) {
    let kw = lines
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(4);
    let dw = lines
        .iter()
        .map(|(_, d)| d.chars().count())
        .max()
        .unwrap_or(10);
    let w = (kw + dw + 7) as u16;
    let h = lines.len() as u16 + 4;
    let area = centered(f.area(), w, h);
    let mut text: Vec<Line> = lines
        .iter()
        .map(|(k, d)| {
            Line::from(vec![
                Span::styled(format!(" {k:<kw$}  "), theme::key()),
                Span::raw(d.to_string()),
            ])
        })
        .collect();
    text.push(Line::default());
    text.push(Line::styled(" Kapatmak için bir tuşa basın.", theme::dim()));
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(text).block(popup_block("Tuşlar")), area);
}

pub fn confirm_popup(f: &mut Frame, title: &str, body: &[&str]) {
    let w = body
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(20)
        .max(30) as u16
        + 4;
    let h = body.len() as u16 + 4;
    let area = centered(f.area(), w, h);
    let mut text: Vec<Line> = body.iter().map(|s| Line::raw(format!(" {s}"))).collect();
    text.push(Line::default());
    text.push(hints(&[("e/Enter", "evet"), ("diğer", "vazgeç")]));
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(text).block(popup_block(title)), area);
}

pub fn message(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    let h = lines.len() as u16;
    let [mid] = Layout::vertical([Constraint::Length(h)])
        .flex(Flex::Center)
        .areas(area);
    f.render_widget(
        Paragraph::new(lines)
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true }),
        mid,
    );
}

/// Kaydırılabilir seçim listesi.
pub fn list(f: &mut Frame, area: Rect, items: Vec<ListItem<'static>>, state: &mut ListState) {
    let list = List::new(items)
        .highlight_style(theme::selected())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, area, state);
}

pub fn move_sel(state: &mut ListState, len: usize, delta: isize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let cur = state.selected().unwrap_or(0) as isize;
    let next = (cur + delta).clamp(0, len as isize - 1) as usize;
    state.select(Some(next));
}

pub fn clamp_sel(state: &mut ListState, len: usize) {
    match state.selected() {
        _ if len == 0 => state.select(None),
        None => state.select(Some(0)),
        Some(i) if i >= len => state.select(Some(len - 1)),
        _ => {}
    }
}

/// Metin kutusu: değer + imleç.
pub fn input_line(label: &str, value: &str, focused: bool, secret: bool) -> Line<'static> {
    let shown = if secret {
        "•".repeat(value.chars().count())
    } else {
        value.to_string()
    };
    let label_style = if focused {
        theme::title()
    } else {
        theme::dim()
    };
    let mut spans = vec![
        Span::styled(format!("{label:>14}  "), label_style),
        Span::styled(
            shown,
            if focused {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        ),
    ];
    if focused {
        spans.push(Span::styled("▏", Style::default().fg(theme::ACCENT)));
    }
    Line::from(spans)
}

/// Satır düzenleme tuşları; değer değiştiyse true.
pub fn edit(value: &mut String, k: ratatui::crossterm::event::KeyEvent, max: usize) -> bool {
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    match k.code {
        KeyCode::Backspace => value.pop().is_some(),
        KeyCode::Char('u') if k.modifiers.contains(KeyModifiers::CONTROL) => {
            let had = !value.is_empty();
            value.clear();
            had
        }
        KeyCode::Char('w') if k.modifiers.contains(KeyModifiers::CONTROL) => {
            let t = value.trim_end().len();
            value.truncate(t);
            let cut = value.rfind(' ').map(|i| i + 1).unwrap_or(0);
            value.truncate(cut);
            true
        }
        KeyCode::Char(c)
            if !k.modifiers.contains(KeyModifiers::CONTROL) && value.chars().count() < max =>
        {
            value.push(c);
            true
        }
        _ => false,
    }
}

/// Son kullanılan sunucu ve kullanıcı adı (parola ya da oturum belirteci değil).
/// Veritabanından ayrı tutulur: çıkışta yerel veri silinse de giriş formu dolu gelir.
pub fn remember_server(dir: &Path, server: &str, user: &str) {
    let _ = std::fs::write(dir.join("last-login"), format!("{server}\n{user}\n"));
}

pub fn last_server(dir: &Path) -> (String, String) {
    let s = std::fs::read_to_string(dir.join("last-login")).unwrap_or_default();
    let mut it = s.lines();
    (
        it.next().unwrap_or("").trim().to_string(),
        it.next().unwrap_or("").trim().to_string(),
    )
}
