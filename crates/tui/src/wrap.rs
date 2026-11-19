//! Blokları verilen genişliğe sözcük sınırından kırarak satırlara dizer.

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
    out: &'a mut Vec<Line<'static>>,
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
        self.out.push(Line::from(spans));
        self.width = 0;
        self.first = false;
    }

    fn push_word(&mut self, w: Word) {
        for (t, s) in w {
            self.width += t.width();
            self.spans
                .push(Span::styled(t, self.base.patch(style_of(s))));
        }
    }
}

fn wrap_block(b: &Block, width: usize, out: &mut Vec<Line<'static>>) {
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

fn pre_block(b: &Block, width: usize, out: &mut Vec<Line<'static>>) {
    let quote = "│ ".repeat(b.quote as usize);
    let avail = width.saturating_sub(quote.width()).max(8);
    let text = b.plain_text();
    let sty = style_of(Sty {
        code: true,
        ..Sty::default()
    });
    for raw in text.split('\n') {
        let raw = raw.replace('\t', "    ");
        let mut cur = String::new();
        let mut w = 0;
        let mut pushed = false;
        for c in raw.chars() {
            let cw = c.width().unwrap_or(0);
            if w + cw > avail {
                out.push(Line::from(vec![
                    Span::styled(quote.clone(), Style::default().fg(theme::DIM)),
                    Span::styled(std::mem::take(&mut cur), sty),
                ]));
                w = 0;
                pushed = true;
            }
            cur.push(c);
            w += cw;
        }
        if !cur.is_empty() || !pushed {
            out.push(Line::from(vec![
                Span::styled(quote.clone(), Style::default().fg(theme::DIM)),
                Span::styled(cur, sty),
            ]));
        }
    }
}

/// Bölümü `width` sütuna dizer. Paragraflar arasında bir boş satır bırakılır.
pub fn layout(blocks: &[Block], width: usize) -> Vec<Line<'static>> {
    let width = width.max(12);
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, b) in blocks.iter().enumerate() {
        let tight = i > 0 && b.list_item && blocks[i - 1].list_item;
        if i > 0 && !tight {
            out.push(Line::default());
        }
        match b.kind {
            Kind::Rule => {
                let mark = "*   *   *";
                let pad = width.saturating_sub(mark.width()) / 2;
                out.push(Line::styled(
                    format!("{}{mark}", " ".repeat(pad)),
                    Style::default().fg(theme::DIM),
                ));
            }
            Kind::Pre => pre_block(b, width, &mut out),
            Kind::Para | Kind::Heading(_) => wrap_block(b, width, &mut out),
        }
    }
    if out.is_empty() {
        out.push(Line::styled(
            "(Bu bölümde metin yok.)",
            Style::default().fg(theme::DIM),
        ));
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
        let l = plain(&layout(&b, 12));
        assert_eq!(l, vec!["bir iki üç", "dört beş", "altı yedi", "sekiz"]);
        for s in &l {
            assert!(s.width() <= 12);
        }
    }

    #[test]
    fn paragraph_spacing_and_lists() {
        let b = parse("<p>a</p><p>b</p><ul><li>x</li><li>y</li></ul>");
        let l = plain(&layout(&b, 20));
        assert_eq!(l, vec!["a", "", "b", "", "• x", "• y"]);
    }

    #[test]
    fn hanging_indent_and_quotes() {
        let b = parse("<ol><li>uzun bir liste öğesi</li></ol><blockquote><p>alıntı metni burada</p></blockquote>");
        let l = plain(&layout(&b, 14));
        assert_eq!(l[0], "1. uzun bir");
        assert!(l[1].starts_with("   "));
        assert!(l.iter().any(|s| s.starts_with("│ alıntı")));
    }

    #[test]
    fn long_words_are_split() {
        let b = parse("<p>aaaaaaaaaaaaaaaaaaaaaaaaa</p>");
        let l = plain(&layout(&b, 12));
        assert_eq!(l, vec!["aaaaaaaaaaaa", "aaaaaaaaaaaa", "a"]);
    }

    #[test]
    fn hard_breaks_and_styles_inside_words() {
        let b = parse("<p>üst<br/>alt Mer<b>ha</b>ba</p>");
        let lines = layout(&b, 30);
        let l = plain(&lines);
        assert_eq!(l, vec!["üst", "alt Merhaba"]);
        assert!(lines[1]
            .spans
            .iter()
            .any(|s| s.content == "ha" && s.style.add_modifier.contains(Modifier::BOLD)));
    }

    #[test]
    fn empty_chapter_has_placeholder() {
        assert_eq!(layout(&[], 40).len(), 1);
    }
}
