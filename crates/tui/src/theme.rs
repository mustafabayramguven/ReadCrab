//! Renkler. Gövde metni terminalin kendi ön/arka plan rengini kullanır.

use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Rgb(0xE8, 0x6A, 0x4A);
pub const DIM: Color = Color::DarkGray;
pub const CODE: Color = Color::Cyan;
pub const OK: Color = Color::Green;
pub const WARN: Color = Color::Yellow;
pub const ERR: Color = Color::Red;
pub const READING: Color = Color::Rgb(0x5B, 0x9B, 0xD5);

pub fn title() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(DIM)
}

/// Açık ve koyu terminal temalarında okunur kalsın diye arka plan rengi verilmez.
pub fn selected() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn key() -> Style {
    Style::default().fg(ACCENT)
}
