//! Blokları verilen genişliğe sözcük sınırından kırarak satırlara dizer.
//!
//! Her satırın bölümdeki başlangıç sırası (`Laid::starts`) da tutulur: satır başından önceki
//! anlamlı birim sayısı. Web ve Android `web/anchor.js` ile aynı sayımı yapar, böylece
//! kaydedilen satır başı (`c`) her cihazda aynı metin konumunu gösterir.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    html::{Block, Kind, Sty},
    theme,
};

pub fn style_of(s: Sty) -> Style {
    let mut st = Style::default();
    if s.bold {
        st = st.add_modifier(Modifier::BOLD);
    }
    if s.italic {
        st = st.add_modifier(Modifier::ITALIC);
    }
    if s.underline {
        st = st.add_modifier(Modifier::UNDERLINED);
    }
    if s.code {
        st = st.fg(theme::CODE);
    }
    if s.image {
        st = st.fg(theme::DIM).add_modifier(Modifier::ITALIC);
    }
    st
}

/// Satır başı sayımında sayılan karakter mi. `web/anchor.js`'deki `significant` ile aynı küme:
/// boşluklar, denetim ve görünmez biçim karakterleri sayılmaz.
pub fn significant(c: char) -> bool {
    let u = c as u32;
    !(u <= 0x20
        || (0x7f..=0x9f).contains(&u)
        || u == 0xa0
        || u == 0xad
        || u == 0x1680
        || (0x2000..=0x200f).contains(&u)
        || (0x2028..=0x202f).contains(&u)
        || (0x205f..=0x2064).contains(&u)
        || u == 0x3000
        || u == 0xfeff)
}

/// Bir parçanın birim sayısı: görsel etiketi görsel başına 1 (`[`), ayırıcı 0.
fn units(c: char, s: Sty) -> u32 {
    if s.sep {
        0
    } else if s.image {
        u32::from(c == '[')
    } else {
        u32::from(significant(c))
    }
}

fn piece_units(t: &str, s: Sty) -> u32 {
    t.chars().map(|c| units(c, s)).sum()
}

/// `starts` dizisinde `c` birimini içeren satır; 0 her zaman ilk satırdır. Aynı başlangıçlı
/// satırlardan (paragraf arası boş satır + metin) metnin olduğu sonuncusu seçilir.
pub fn line_of(starts: &[u32], c: u32) -> usize {
    if c == 0 {
        return 0;
    }
    starts.partition_point(|&s| s <= c).saturating_sub(1)
}

/// Dizilmiş bölüm.
pub struct Laid {
    pub lines: Vec<Line<'static>>,
    /// Satır başına, o satırdan önceki anlamlı birim sayısı (`lines` ile aynı uzunlukta).
    pub starts: Vec<u32>,
}

impl Laid {
    fn push(&mut self, line: Line<'static>, start: u32) {
        self.lines.push(line);
        self.starts.push(start);
    }
}

type Word = Vec<(String, Sty)>;

enum Tok {
    Word(Word),
    Space,
    Break,
}

fn tokens(spans: &[(String, Sty)]) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut word: Word = Vec::new();
    let end_word = |word: &mut Word, out: &mut Vec<Tok>| {
        if !word.is_empty() {
            out.push(Tok::Word(std::mem::take(word)));
        }
    };
    for (text, sty) in spans {
        for c in text.chars() {
            match c {
                '\n' => {
                    end_word(&mut word, &mut out);
                    out.push(Tok::Break);
                }
                c if c.is_whitespace() && c != '\u{a0}' => {
                    end_word(&mut word, &mut out);
                    if !matches!(out.last(), Some(Tok::Space)) {
                        out.push(Tok::Space);
                    }
                }
                c => {
                    let c = if c == '\u{a0}' { ' ' } else { c };
                    match word.last_mut() {
                        Some((t, s)) if s == sty => t.push(c),
                        _ => word.push((c.to_string(), *sty)),
                    }
                }
            }
        }
    }
    end_word(&mut word, &mut out);
    out
}

fn word_width(w: &Word) -> usize {
    w.iter().map(|(t, _)| t.width()).sum()
}

/// Sığmayan uzun sözcüğü karakter sınırından parçalar.
fn split_word(w: Word, avail: usize) -> Vec<Word> {
    let mut parts = Vec::new();
    let mut cur: Word = Vec::new();
    let mut cur_w = 0;
    for (text, sty) in w {
        for c in text.chars() {
            let cw = c.width().unwrap_or(0);
            if cur_w + cw > avail && cur_w > 0 {
                parts.push(std::mem::take(&mut cur));
                cur_w = 0;
            }
            match cur.last_mut() {
                Some((t, s)) if *s == sty => t.push(c),
                _ => cur.push((c.to_string(), sty)),
            }
            cur_w += cw;
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

struct LineBuilder<'a> {
    out: &'a mut Laid,
    /// Bölümde şu ana dek geçen birim sayısı.
    pos: &'a mut u32,
    /// Geçerli satırın ilk parçasındaki konum.
    line_start: Option<u32>,
    first_prefix: String,
    cont_prefix: String,
    prefix_style: Style,
    base: Style,
    spans: Vec<Span<'static>>,
    width: usize,
    first: bool,
}

impl LineBuilder<'_> {
    fn emit(&mut self) {
        let prefix = if self.first {
            &self.first_prefix
        } else {
            &self.cont_prefix
        };
        let mut spans = Vec::with_capacity(self.spans.len() + 1);
        if !prefix.is_empty() {
            spans.push(Span::styled(prefix.clone(), self.prefix_style));
        }
        spans.append(&mut self.spans);
        let start = self.line_start.take().unwrap_or(*self.pos);
        self.out.push(Line::from(spans), start);
        self.width = 0;
        self.first = false;
    }

    fn push_word(&mut self, w: Word) {
        if self.line_start.is_none() {
            self.line_start = Some(*self.pos);
        }
        for (t, s) in w {
            *self.pos += piece_units(&t, s);
            self.width += t.width();
            self.spans
                .push(Span::styled(t, self.base.patch(style_of(s))));
        }
    }
}

fn wrap_block(b: &Block, width: usize, out: &mut Laid, pos: &mut u32) {
    let quote = "│ ".repeat(b.quote as usize);
    let first_prefix = format!("{quote}{}", b.prefix);
    let cont_prefix = format!("{quote}{}", " ".repeat(b.prefix.width()));
    let avail = width.saturating_sub(first_prefix.width()).max(8);
    let base = match b.kind {
        Kind::Heading(_) => Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
        _ => Style::default(),
    };
    let mut lb = LineBuilder {
        out,
        pos,
        line_start: None,
        first_prefix,
        cont_prefix,
        prefix_style: Style::default().fg(theme::DIM),
        base,
        spans: Vec::new(),
        width: 0,
        first: true,
    };
    let mut pending_space = false;
    for tok in tokens(&b.spans) {
        match tok {
            Tok::Space => pending_space = true,
            Tok::Break => {
                lb.emit();
                pending_space = false;
            }
            Tok::Word(w) => {
                let ww = word_width(&w);
                if lb.width > 0 && lb.width + usize::from(pending_space) + ww <= avail {
                    if pending_space {
                        lb.spans.push(Span::styled(" ", base));
                        lb.width += 1;
                    }
                    lb.push_word(w);
                } else {
                    if lb.width > 0 {
                        lb.emit();
                    }
                    if ww > avail {
                        let parts = split_word(w, avail);
                        let n = parts.len();
                        for (i, part) in parts.into_iter().enumerate() {
                            lb.push_word(part);
                            if i + 1 < n {
                                lb.emit();
                            }
                        }
                    } else {
                        lb.push_word(w);
                    }
                }
                pending_space = false;
            }
        }
    }
    if lb.width > 0 || lb.first {
        lb.emit();
    }
}

fn pre_block(b: &Block, width: usize, out: &mut Laid, pos: &mut u32) {
    let quote = "│ ".repeat(b.quote as usize);
    let avail = width.saturating_sub(quote.width()).max(8);
    let sty = style_of(Sty {
        code: true,
        ..Sty::default()
    });
    let quote_style = Style::default().fg(theme::DIM);
    let mut cur = String::new();
    let mut w = 0;
    let mut start = *pos;
    let flush = |cur: &mut String, start: u32, out: &mut Laid| {
        out.push(
            Line::from(vec![
                Span::styled(quote.clone(), quote_style),
                Span::styled(std::mem::take(cur), sty),
            ]),
            start,
        );
    };
    let mut pushed_in_raw = false;
    for (text, s) in &b.spans {
        for c in text.chars() {
            if c == '\n' {
                if !cur.is_empty() || !pushed_in_raw {
                    flush(&mut cur, start, out);
                }
                w = 0;
                start = *pos;
                pushed_in_raw = false;
                continue;
            }
            let (c, n) = if c == '\t' { (' ', 4) } else { (c, 1) };
            for _ in 0..n {
                let cw = c.width().unwrap_or(0);
                if w + cw > avail {
                    flush(&mut cur, start, out);
                    w = 0;
                    start = *pos;
                    pushed_in_raw = true;
                }
                cur.push(c);
                w += cw;
            }
            *pos += units(c, *s);
        }
    }
    if !cur.is_empty() || !pushed_in_raw {
        flush(&mut cur, start, out);
    }
}

/// Bölümü `width` sütuna dizer. Paragraflar arasında bir boş satır bırakılır.
pub fn layout(blocks: &[Block], width: usize) -> Laid {
    let width = width.max(12);
    let mut out = Laid {
        lines: Vec::new(),
        starts: Vec::new(),
    };
    let mut pos = 0u32;
    for (i, b) in blocks.iter().enumerate() {
        let tight = i > 0 && b.list_item && blocks[i - 1].list_item;
        if i > 0 && !tight {
            out.push(Line::default(), pos);
        }
        match b.kind {
            Kind::Rule => {
                let mark = "*   *   *";
                let pad = width.saturating_sub(mark.width()) / 2;
                out.push(
                    Line::styled(
                        format!("{}{mark}", " ".repeat(pad)),
                        Style::default().fg(theme::DIM),
                    ),
                    pos,
                );
            }
            Kind::Pre => pre_block(b, width, &mut out, &mut pos),
            Kind::Para | Kind::Heading(_) => wrap_block(b, width, &mut out, &mut pos),
        }
    }
    if out.lines.is_empty() {
        out.push(
            Line::styled("(Bu bölümde metin yok.)", Style::default().fg(theme::DIM)),
            0,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::html::parse;

    fn plain(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn wraps_on_word_boundaries() {
        let b = parse("<p>bir iki üç dört beş altı yedi sekiz</p>");
        let l = plain(&layout(&b, 12).lines);
        assert_eq!(l, vec!["bir iki üç", "dört beş", "altı yedi", "sekiz"]);
        for s in &l {
            assert!(s.width() <= 12);
        }
    }

    #[test]
    fn paragraph_spacing_and_lists() {
        let b = parse("<p>a</p><p>b</p><ul><li>x</li><li>y</li></ul>");
        let l = plain(&layout(&b, 20).lines);
        assert_eq!(l, vec!["a", "", "b", "", "• x", "• y"]);
    }

    #[test]
    fn hanging_indent_and_quotes() {
        let b = parse("<ol><li>uzun bir liste öğesi</li></ol><blockquote><p>alıntı metni burada</p></blockquote>");
        let l = plain(&layout(&b, 14).lines);
        assert_eq!(l[0], "1. uzun bir");
        assert!(l[1].starts_with("   "));
        assert!(l.iter().any(|s| s.starts_with("│ alıntı")));
    }

    #[test]
    fn long_words_are_split() {
        let b = parse("<p>aaaaaaaaaaaaaaaaaaaaaaaaa</p>");
        let l = plain(&layout(&b, 12).lines);
        assert_eq!(l, vec!["aaaaaaaaaaaa", "aaaaaaaaaaaa", "a"]);
    }

    #[test]
    fn hard_breaks_and_styles_inside_words() {
        let b = parse("<p>üst<br/>alt Mer<b>ha</b>ba</p>");
        let lines = layout(&b, 30).lines;
        let l = plain(&lines);
        assert_eq!(l, vec!["üst", "alt Merhaba"]);
        assert!(lines[1]
            .spans
            .iter()
            .any(|s| s.content == "ha" && s.style.add_modifier.contains(Modifier::BOLD)));
    }

    #[test]
    fn empty_chapter_has_placeholder() {
        assert_eq!(layout(&[], 40).lines.len(), 1);
    }

    #[test]
    fn significant_matches_anchor_js() {
        // web/anchor.test.mjs ile aynı dizi ve sonuç
        let sample = "  Merhaba,\u{a0}d\u{fc}nya!\u{ad}\u{200b} \u{1F600} a\tb\u{3000}c\u{feff}";
        assert_eq!(sample.chars().filter(|&c| significant(c)).count(), 18);
        assert!(!significant('\u{a0}') && !significant('\u{2009}') && !significant('\n'));
        assert!(significant('·') && significant('ş'));
    }

    #[test]
    fn line_starts_count_text_only() {
        // "bir iki üç" | "dört beş" ... ; boşluklar ve paragraf arası sayılmaz
        let b = parse("<p>bir iki üç dört beş</p><p>altı</p>");
        let laid = layout(&b, 12);
        assert_eq!(
            plain(&laid.lines),
            vec!["bir iki üç", "dört beş", "", "altı"]
        );
        assert_eq!(laid.starts, vec![0, 8, 15, 15]);
        assert_eq!(line_of(&laid.starts, 0), 0);
        assert_eq!(line_of(&laid.starts, 7), 0);
        assert_eq!(line_of(&laid.starts, 8), 1);
        assert_eq!(line_of(&laid.starts, 14), 1);
        // paragraf arası boş satır değil, metnin olduğu satır
        assert_eq!(line_of(&laid.starts, 15), 3);
        assert_eq!(line_of(&laid.starts, 999), 3);
    }

    #[test]
    fn line_starts_do_not_depend_on_width() {
        let b = parse("<h1>Başlık</h1><p>bir iki üç dört beş altı yedi sekiz dokuz on</p>");
        for w in [12, 20, 40, 80] {
            let laid = layout(&b, w);
            // her satır başı, satırdaki ilk sözcüğün metindeki gerçek sırasıdır
            for (line, &start) in laid.lines.iter().zip(&laid.starts) {
                let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                if let Some(word) = text.split_whitespace().next() {
                    let all: String = b.iter().map(|b| b.plain_text()).collect();
                    let before: usize = all
                        .chars()
                        .filter(|&c| significant(c))
                        .take(start as usize)
                        .count();
                    assert_eq!(before, start as usize);
                    let rest: String = all
                        .chars()
                        .filter(|&c| significant(c))
                        .skip(start as usize)
                        .collect();
                    assert!(rest.starts_with(word), "{w}: {word} @ {start}");
                }
            }
        }
    }

    #[test]
    fn images_count_once_and_separators_not_at_all() {
        let b = parse(
            "<p>a<img alt=\"x [y] z\"/>b</p><table><tr><td>c</td><td>d</td></tr></table><p>e</p>",
        );
        let laid = layout(&b, 80);
        let l = plain(&laid.lines);
        assert_eq!(l[0], "a[görsel: x (y) z]b");
        assert_eq!(l[2], "c · d");
        // a=0, görsel=1, b=2 | c=3, d=4 | e=5
        assert_eq!(laid.starts[2], 3);
        assert_eq!(laid.starts[4], 5);
    }

    #[test]
    fn pre_lines_have_starts() {
        let b = parse("<pre>ab cd\nef</pre><p>g</p>");
        let laid = layout(&b, 40);
        assert_eq!(
            plain(&laid.lines),
            vec![
                "abcd".replace("abcd", "ab cd"),
                "ef".into(),
                "".into(),
                "g".into()
            ]
        );
        assert_eq!(laid.starts, vec![0, 4, 6, 6]);
    }
}
