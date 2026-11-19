//! Küçük yardımcılar: yerel tarih/saat, konum (locator) biçimi, biçimlendirme.

use std::time::{SystemTime, UNIX_EPOCH};

/// Yerel saat bileşenleri (yıl, ay, gün, saat, dakika).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTm {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub min: u32,
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `localtime_r` ile yerel saat dilimine çevirir.
pub fn local_tm(secs: i64) -> LocalTm {
    let t = secs as libc::time_t;
    // SAFETY: `tm` sıfırla başlatılmış düz bir C yapısıdır; `localtime_r` iş parçacığı güvenlidir
    // ve yalnız verilen işaretçilere yazar.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let ok = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };
    if !ok {
        let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
        let rem = secs.rem_euclid(86_400);
        return LocalTm {
            year: y,
            month: m,
            day: d,
            hour: (rem / 3600) as u32,
            min: ((rem % 3600) / 60) as u32,
        };
    }
    LocalTm {
        year: tm.tm_year + 1900,
        month: (tm.tm_mon + 1) as u32,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        min: tm.tm_min as u32,
    }
}

/// Bugünün yerel tarihi, `YYYY-AA-GG`.
pub fn today() -> String {
    let t = local_tm(unix_now());
    format!("{:04}-{:02}-{:02}", t.year, t.month, t.day)
}

pub fn current_year() -> i32 {
    local_tm(unix_now()).year
}

/// Son eşitleme gibi damgalar: bugünse `SS:DD`, değilse `GG.AA SS:DD`.
pub fn fmt_stamp(secs: i64) -> String {
    let t = local_tm(secs);
    let n = local_tm(unix_now());
    if (t.year, t.month, t.day) == (n.year, n.month, n.day) {
        format!("{:02}:{:02}", t.hour, t.min)
    } else {
        format!("{:02}.{:02} {:02}:{:02}", t.day, t.month, t.hour, t.min)
    }
}

/// Howard Hinnant'ın algoritması: 1970-01-01'den gün sayısı → (yıl, ay, gün).
pub fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    ((if m <= 2 { y + 1 } else { y }) as i32, m, d)
}

/// (yıl, ay, gün) → 1970-01-01'den gün sayısı.
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y as i64 - 1 } else { y as i64 };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `YYYY-AA-GG` → gün sayısı.
pub fn parse_date(s: &str) -> Option<i64> {
    let mut it = s.split('-');
    let y = it.next()?.parse().ok()?;
    let m = it.next()?.parse().ok()?;
    let d = it.next()?.parse().ok()?;
    if it.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    Some(days_from_civil(y, m, d))
}

/// Haftanın günü, Pazartesi = 0.
pub fn weekday(days: i64) -> u32 {
    // 1970-01-01 Perşembe (3)
    ((days + 3).rem_euclid(7)) as u32
}

/// Metin kitabı konumu: web `{"ch","p"}`, mobil `{"ch","p","b"}`.
#[derive(serde::Deserialize)]
struct TextLoc {
    ch: i64,
    p: f64,
}

/// Geçersizse `None`; `p` [0,1]'e kırpılır.
pub fn parse_text_loc(s: &str) -> Option<(i32, f64)> {
    let l: TextLoc = serde_json::from_str(s.trim()).ok()?;
    if l.ch < 0 || l.ch > i32::MAX as i64 || !l.p.is_finite() {
        return None;
    }
    Some((l.ch as i32, l.p.clamp(0.0, 1.0)))
}

/// Android ile aynı biçim.
pub fn build_text_loc(ch: i32, p: f64) -> String {
    format!("{{\"ch\":{},\"p\":{:.6}}}", ch.max(0), p.clamp(0.0, 1.0))
}

pub fn fmt_percent(p: f64) -> String {
    format!("%{}", (p.clamp(0.0, 1.0) * 100.0).round() as i64)
}

pub fn fmt_bytes(n: i64) -> String {
    let n = n.max(0) as f64;
    if n < 1024.0 {
        format!("{n:.0} B")
    } else if n < 1024.0 * 1024.0 {
        format!("{:.0} KB", n / 1024.0)
    } else if n < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} MB", n / 1024.0 / 1024.0)
    } else {
        format!("{:.2} GB", n / 1024.0 / 1024.0 / 1024.0)
    }
}

pub fn fmt_duration(secs: i64) -> String {
    let secs = secs.max(0);
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{h} sa {m} dk")
    } else if m > 0 {
        format!("{m} dk")
    } else {
        format!("{secs} sn")
    }
}

/// Unicode genişliğine göre kırpar; kırpıldıysa sonuna `…` ekler.
pub fn truncate(s: &str, max: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let total: usize = s.chars().map(|c| c.width().unwrap_or(0)).sum();
    if total <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > max {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

/// Metnin son `max` sütunu; kesilen ilk sözcük atılır ve başa `…` konur.
pub fn tail_words(s: &str, max: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    let s = s.replace('\n', " ");
    let total: usize = s.chars().map(|c| c.width().unwrap_or(0)).sum();
    if total <= max {
        return s;
    }
    let mut w = 0;
    let mut start = s.len();
    for (i, c) in s.char_indices().rev() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > max {
            break;
        }
        w += cw;
        start = i;
    }
    let tail = &s[start..];
    let tail = match tail.find(' ') {
        Some(sp) if sp + 1 < tail.len() => &tail[sp + 1..],
        _ => tail,
    };
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_locator_roundtrip() {
        assert_eq!(parse_text_loc("{\"ch\":3,\"p\":0.25}"), Some((3, 0.25)));
        assert_eq!(
            parse_text_loc("{\"ch\":1,\"p\":0.5,\"b\":7}"),
            Some((1, 0.5))
        );
        assert_eq!(parse_text_loc(" {\"ch\":0,\"p\":1.7} "), Some((0, 1.0)));
        assert_eq!(parse_text_loc("12"), None);
        assert_eq!(parse_text_loc(""), None);
        assert_eq!(parse_text_loc("{\"ch\":-1,\"p\":0}"), None);
        assert_eq!(build_text_loc(2, 0.5), "{\"ch\":2,\"p\":0.500000}");
        let s = build_text_loc(4, 0.123456789);
        assert_eq!(parse_text_loc(&s), Some((4, 0.123457)));
    }

    #[test]
    fn civil_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        let d = days_from_civil(2026, 11, 19);
        assert_eq!(civil_from_days(d), (2026, 11, 19));
        assert_eq!(parse_date("2026-11-19"), Some(d));
        assert_eq!(parse_date("2026-13-01"), None);
        // 2026-11-19 Perşembe
        assert_eq!(weekday(d), 3);
        assert_eq!(weekday(days_from_civil(2026, 11, 16)), 0);
    }

    #[test]
    fn formatting() {
        assert_eq!(fmt_percent(0.456), "%46");
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(3 * 1024 * 1024), "3.0 MB");
        assert_eq!(fmt_duration(3725), "1 sa 2 dk");
        assert_eq!(fmt_duration(42), "42 sn");
        assert_eq!(truncate("merhaba dünya", 8), "merhaba…");
        assert_eq!(truncate("kısa", 8), "kısa");
        assert_eq!(tail_words("bir iki üç dört", 9), "…üç dört");
        assert_eq!(tail_words("kısa", 9), "kısa");
    }
}
