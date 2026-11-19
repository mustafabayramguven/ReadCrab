//! Okuma istatistikleri: seri (streak), toplamlar ve yıllık ısı haritası.

use std::collections::HashMap;

use ratatui::{
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use readcrab_core::{StatsDay, StatsSnapshot};

use crate::{
    app::{Action, Ctx, Fx, Msg},
    theme, util, widgets,
};

pub struct StatsView {
    year: i32,
    snap: Option<StatsSnapshot>,
    loading: bool,
    error: Option<String>,
}

impl StatsView {
    pub const HELP: &'static [(&'static str, &'static str)] = &[
        ("← / →", "önceki / sonraki yıl"),
        ("r", "yenile"),
        ("q / Esc", "geri"),
    ];

    pub fn new(ctx: &Ctx) -> StatsView {
        let year = util::current_year();
        let mut v = StatsView {
            year,
            snap: ctx.core.stats_cached(year).ok().flatten(),
            loading: false,
            error: None,
        };
        v.load(ctx);
        v
    }

    fn load(&mut self, ctx: &Ctx) {
        self.loading = true;
        let year = self.year;
        ctx.spawn(move |core| {
            Some(Msg::Stats {
                year,
                result: core.stats_load(year, util::today()),
            })
        });
    }

    fn change_year(&mut self, ctx: &Ctx, delta: i32) {
        let years = self
            .snap
            .as_ref()
            .map(|s| s.years.clone())
            .unwrap_or_default();
        let next = self.year + delta;
        let allowed = years.contains(&next) || (delta > 0 && next <= util::current_year());
        if !allowed {
            return;
        }
        self.year = next;
        self.snap = ctx.core.stats_cached(next).ok().flatten();
        self.error = None;
        self.load(ctx);
    }

    pub fn key(&mut self, ctx: &Ctx, k: KeyEvent, fx: &mut Fx) {
        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => fx.act(Action::Back),
            KeyCode::Left | KeyCode::Char('h') => self.change_year(ctx, -1),
            KeyCode::Right | KeyCode::Char('l') => self.change_year(ctx, 1),
            KeyCode::Char('r') => self.load(ctx),
            _ => {}
        }
    }

    pub fn on_msg(&mut self, m: Msg) {
        if let Msg::Stats { year, result } = m {
            if year != self.year {
                return;
            }
            self.loading = false;
            match result {
                Ok(s) => {
                    self.snap = Some(s);
                    self.error = None;
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }

    pub fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        let [head, _, summary, _, heat, foot] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(9),
            Constraint::Length(1),
        ])
        .areas(area);
        let mut h = vec![Span::styled(
            format!(" Okuma istatistikleri · {}", self.year),
            theme::title(),
        )];
        if self.loading {
            h.push(Span::styled(
                "  yükleniyor…",
                Style::default().fg(theme::WARN),
            ));
        } else if let Some(e) = &self.error {
            h.push(Span::styled(
                format!("  {}", util::truncate(e, 60)),
                Style::default().fg(theme::ERR),
            ));
        } else if self.snap.as_ref().is_some_and(|s| s.from_cache) {
            h.push(Span::styled("  (çevrimdışı önbellek)", theme::dim()));
        }
        f.render_widget(Paragraph::new(Line::from(h)), head);

        let Some(s) = &self.snap else {
            widgets::message(
                f,
                summary,
                vec![Line::styled("Henüz istatistik yok.", theme::dim())],
            );
            return;
        };
        let streak_style = if s.streak_at_risk {
            Style::default().fg(theme::WARN)
        } else {
            Style::default().fg(theme::OK)
        };
        let mut streak = vec![
            Span::styled("  Seri       ", theme::dim()),
            Span::styled(format!("{} gün", s.streak_current), streak_style),
            Span::styled(
                format!("  (en uzun {} gün)", s.streak_longest),
                theme::dim(),
            ),
        ];
        if s.streak_at_risk {
            streak.push(Span::styled(
                "  · bugün okumazsan seri bozulur",
                Style::default().fg(theme::WARN),
            ));
        }
        if s.freeze_available {
            streak.push(Span::styled("  · dondurma hakkı var", theme::dim()));
        }
        let today = vec![
            Span::styled("  Bugün      ", theme::dim()),
            Span::raw(util::fmt_duration(s.today_seconds)),
            Span::styled(
                format!(" / hedef {}", util::fmt_duration(s.streak_min_seconds)),
                theme::dim(),
            ),
        ];
        let totals = vec![
            Span::styled("  Toplam     ", theme::dim()),
            Span::raw(format!(
                "{} · {} sayfa · {} kitap bitti",
                util::fmt_duration(s.total_seconds),
                s.total_pages,
                s.finished_count
            )),
        ];
        f.render_widget(
            Paragraph::new(vec![
                Line::from(streak),
                Line::from(today),
                Line::from(totals),
            ]),
            summary,
        );
        draw_heatmap(f, heat, self.year, &s.days);
        f.render_widget(
            Paragraph::new(widgets::hints(&[
                ("←/→", "yıl"),
                ("r", "yenile"),
                ("Esc", "geri"),
            ])),
            foot,
        );
    }
}

fn level_color(d: Option<&StatsDay>) -> (char, Color) {
    let Some(d) = d else {
        return ('·', theme::DIM);
    };
    if d.frozen {
        return ('❄', theme::READING);
    }
    if d.seconds <= 0 {
        return ('·', theme::DIM);
    }
    let c = match d.seconds / 60 {
        0..=9 => Color::Rgb(0x6B, 0x3A, 0x30),
        10..=29 => Color::Rgb(0x9C, 0x4A, 0x36),
        30..=59 => Color::Rgb(0xC8, 0x5A, 0x40),
        _ => theme::ACCENT,
    };
    ('■', c)
}

/// Sütunlar hafta (Pazartesi başlar), satırlar gün. Sığmazsa son haftalar gösterilir.
fn draw_heatmap(f: &mut Frame, area: Rect, year: i32, days: &[StatsDay]) {
    let map: HashMap<i64, &StatsDay> = days
        .iter()
        .filter_map(|d| util::parse_date(&d.date).map(|n| (n, d)))
        .collect();
    let start = util::days_from_civil(year, 1, 1);
    let end = util::days_from_civil(year, 12, 31);
    let today = util::parse_date(&util::today()).unwrap_or(end);
    let last = end.min(today.max(start));
    let first_monday = start - util::weekday(start) as i64;
    let weeks = ((last - first_monday) / 7 + 1) as usize;
    let fit = (area.width.saturating_sub(6) / 2) as usize;
    let skip = weeks.saturating_sub(fit);
    const NAMES: [&str; 7] = ["Pzt ", "    ", "Çar ", "    ", "Cum ", "    ", "Paz "];
    let mut lines = Vec::new();
    // ay başlıkları
    let mut months = String::from("     ");
    let mut last_month = 0;
    for w in skip..weeks {
        let day = first_monday + (w * 7) as i64;
        let (_, m, _) = util::civil_from_days(day.max(start));
        if m != last_month {
            const AY: [&str; 12] = [
                "Oca", "Şub", "Mar", "Nis", "May", "Haz", "Tem", "Ağu", "Eyl", "Eki", "Kas", "Ara",
            ];
            let label = AY[(m - 1) as usize];
            let pos = 5 + (w - skip) * 2;
            while months.chars().count() < pos {
                months.push(' ');
            }
            if months.chars().count() == pos {
                months.push_str(label);
            }
            last_month = m;
        }
    }
    lines.push(Line::styled(months, theme::dim()));
    for (wd, name) in NAMES.iter().enumerate() {
        let mut spans = vec![Span::styled(format!(" {name}"), theme::dim())];
        for w in skip..weeks {
            let day = first_monday + (w * 7 + wd) as i64;
            if day < start || day > last {
                spans.push(Span::raw("  "));
                continue;
            }
            let (c, color) = level_color(map.get(&day).copied());
            spans.push(Span::styled(format!("{c} "), Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::default());
    let mut legend = vec![Span::styled("     az ", theme::dim())];
    for c in [
        Color::Rgb(0x6B, 0x3A, 0x30),
        Color::Rgb(0x9C, 0x4A, 0x36),
        Color::Rgb(0xC8, 0x5A, 0x40),
        theme::ACCENT,
    ] {
        legend.push(Span::styled("■ ", Style::default().fg(c)));
    }
    legend.push(Span::styled("çok   ❄ dondurulmuş gün", theme::dim()));
    lines.push(Line::from(legend));
    f.render_widget(Paragraph::new(lines), area);
}
