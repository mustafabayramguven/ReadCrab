use std::time::{Duration, Instant};

use serde::Serialize;

use crate::api::library::fold_char;

const MAX_HITS: usize = 100;
const CONTEXT: usize = 40;
const MAX_SCANNED_BYTES: usize = 64 * 1024 * 1024;
const MAX_DURATION: Duration = Duration::from_secs(10);
const BLOCK_TAGS: [&str; 15] = [
    "p",
    "div",
    "br",
    "li",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "tr",
    "td",
    "th",
    "blockquote",
    "section",
];

#[derive(Serialize, Debug)]
pub struct SearchHit {
    pub ch: usize,
    pub p: f64,
    pub before: String,
    pub hit: String,
    pub after: String,
}

fn push_decoded(out: &mut String, text: &str) {
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let decoded = rest[1..]
            .find(';')
            .filter(|&j| j <= 8)
            .and_then(|j| decode_entity(&rest[1..=j]).map(|c| (c, j + 2)));
        match decoded {
            Some((c, used)) => {
                out.push(c);
                rest = &rest[used..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
}

fn decode_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let n = name.strip_prefix('#')?;
            let code = match n.strip_prefix(['x', 'X']) {
                Some(h) => u32::from_str_radix(h, 16).ok()?,
                None => n.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

/// Büyük/küçük harfe bakmadan `needle` (küçük harf ASCII) konumunu bulur.
fn find_ci(hay: &str, needle: &str) -> Option<usize> {
    let n = needle.as_bytes();
    hay.as_bytes()
        .windows(n.len())
        .position(|w| w.eq_ignore_ascii_case(n))
}

// shortcut: öznitelik değeri içindeki '>' etiketi erken bitirir, gerekirse tırnak izlenir
pub fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        push_decoded(&mut out, &rest[..i]);
        rest = &rest[i + 1..];
        if let Some(r) = rest.strip_prefix("!--") {
            rest = r.find("-->").map_or("", |j| &r[j + 3..]);
            continue;
        }
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        rest = rest.get(end + 1..).unwrap_or("");
        let closing = tag.starts_with('/');
        let name: String = tag
            .trim_start_matches('/')
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        if !closing && matches!(name.as_str(), "script" | "style" | "title") {
            rest = find_ci(rest, &format!("</{name}"))
                .and_then(|j| rest[j..].find('>').map(|k| &rest[j + k + 1..]))
                .unwrap_or("");
        }
        if BLOCK_TAGS.contains(&name.as_str()) {
            out.push(' ');
        }
    }
    push_decoded(&mut out, rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn slice(text: &[char], a: usize, b: usize) -> String {
    text[a..b].iter().collect::<String>().trim().to_string()
}

pub struct Collector {
    query: Vec<char>,
    pub results: Vec<SearchHit>,
    pub truncated: bool,
    scanned: usize,
    start: Instant,
}

impl Collector {
    /// `query_folded`: `fold_tr` uygulanmış sorgu.
    pub fn new(query_folded: &str) -> Self {
        Collector {
            query: query_folded.chars().collect(),
            results: Vec::new(),
            truncated: false,
            scanned: 0,
            start: Instant::now(),
        }
    }

    /// Bölümü tarar. Sınırlar aşıldıysa `false` döner (daha fazla bölüm taranmamalı).
    pub fn add_chapter(&mut self, ch: usize, html: &str) -> bool {
        self.scanned += html.len();
        if self.scanned > MAX_SCANNED_BYTES || self.start.elapsed() > MAX_DURATION {
            self.truncated = true;
            return false;
        }
        let text: Vec<char> = strip_tags(html).chars().collect();
        let folded: Vec<char> = text.iter().map(|&c| fold_char(c)).collect();
        let q = &self.query;
        let mut i = 0;
        while i + q.len() <= folded.len() {
            if folded[i..i + q.len()] != q[..] {
                i += 1;
                continue;
            }
            if self.results.len() >= MAX_HITS {
                self.truncated = true;
                return false;
            }
            let end = i + q.len();
            self.results.push(SearchHit {
                ch,
                p: (i as f64 / text.len() as f64 * 1e6).round() / 1e6,
                before: slice(&text, i.saturating_sub(CONTEXT), i),
                hit: slice(&text, i, end),
                after: slice(&text, end, (end + CONTEXT).min(text.len())),
            });
            i = end;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::library::fold_tr;

    #[test]
    fn strips_tags_entities_and_skipped_blocks() {
        let html = "<html><head><title>Gizli</title><style>p{}</style></head>\
            <body><p>şeker&amp;<b>tuz</b></p><script>x()</script><div>a&nbsp;&#65;&#x42;</div>\
            <!-- yorum --><br/>son &bilinmez; &lt;</body></html>";
        assert_eq!(strip_tags(html), "şeker&tuz a AB son &bilinmez; <");
    }

    #[test]
    fn finds_folded_non_overlapping_hits() {
        let mut c = Collector::new(&fold_tr("ışık"));
        assert!(c.add_chapter(2, "<p>IŞIK ve İŞİK ve ışık</p>"));
        assert_eq!(c.results.len(), 3);
        assert_eq!(c.results[0].hit, "IŞIK");
        assert_eq!(c.results[0].ch, 2);
        assert_eq!(c.results[1].before, "IŞIK ve");
        let mut c = Collector::new("aa");
        c.add_chapter(0, "<p>aaaaa</p>");
        assert_eq!(c.results.len(), 2);
    }

    #[test]
    fn caps_hits_at_100() {
        let mut c = Collector::new("ab");
        let html = format!("<p>{}</p>", "ab ".repeat(150));
        assert!(!c.add_chapter(0, &html));
        assert_eq!(c.results.len(), 100);
        assert!(c.truncated);
    }
}
