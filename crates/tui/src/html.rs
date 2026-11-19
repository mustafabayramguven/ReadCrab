//! Bölüm gövdesi (sunucuda ammonia ile temizlenmiş HTML) → terminalde gösterilecek bloklar.
//!
//! Tam bir HTML ayrıştırıcısı değildir; temizlenmiş, iyi biçimli girdide blok yapısını,
//! kalın/italik vurguları, listeleri, alıntıları ve görsel yer tutucularını korur.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sty {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub code: bool,
    /// Görsel yer tutucusu ("[görsel: …]"); satır başı sayımında görsel başına 1 birim (`[`).
    pub image: bool,
    /// Ekranda olup HTML metninde olmayan ayırıcı (tablo hücreleri arası); sayılmaz.
    pub sep: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Para,
    Heading(u8),
    Pre,
    Rule,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Block {
    pub kind: Kind,
    /// Alıntı derinliği.
    pub quote: u16,
    /// Liste işareti ("• ", "3. "); yalnız ilk satırda yazılır.
    pub prefix: String,
    /// Metin parçaları; `\n` zorunlu satır sonudur.
    pub spans: Vec<(String, Sty)>,
    /// Liste öğesi: önceki öğeyle arasında boş satır olmaz.
    pub list_item: bool,
}

impl Block {
    #[cfg(test)]
    pub fn plain_text(&self) -> String {
        self.spans.iter().map(|(t, _)| t.as_str()).collect()
    }
}

#[derive(Default)]
struct Parser {
    blocks: Vec<Block>,
    cur: Vec<(String, Sty)>,
    kind: Option<Kind>,
    prefix: String,
    in_li: bool,
    quote: u16,
    pre: u32,
    bold: u32,
    italic: u32,
    underline: u32,
    code: u32,
    skip: u32,
    lists: Vec<Option<u32>>,
}

impl Parser {
    fn sty(&self) -> Sty {
        Sty {
            bold: self.bold > 0 || matches!(self.kind, Some(Kind::Heading(_))),
            italic: self.italic > 0,
            underline: self.underline > 0,
            code: self.code > 0 || self.pre > 0,
            image: false,
            sep: false,
        }
    }

    fn push_text(&mut self, text: &str, sty: Sty) {
        if text.is_empty() {
            return;
        }
        if let Some((last, s)) = self.cur.last_mut() {
            if *s == sty {
                last.push_str(text);
                return;
            }
        }
        self.cur.push((text.to_string(), sty));
    }

    fn text(&mut self, raw: &str) {
        if self.skip > 0 {
            return;
        }
        let decoded = decode_entities(raw);
        let sty = self.sty();
        if self.pre > 0 {
            self.push_text(&decoded, sty);
            return;
        }
        // boşluk dizileri tek boşluğa iner; blok başındaki boşluk atılır
        let mut out = String::with_capacity(decoded.len());
        let mut prev_space = self
            .cur
            .last()
            .map(|(t, _)| t.ends_with([' ', '\n']))
            .unwrap_or(true);
        for c in decoded.chars() {
            if c.is_whitespace() && c != '\u{a0}' {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            } else {
                out.push(c);
                prev_space = false;
            }
        }
        self.push_text(&out, sty);
    }

    fn flush(&mut self) {
        let kind = self
            .kind
            .unwrap_or(if self.pre > 0 { Kind::Pre } else { Kind::Para });
        let mut spans = std::mem::take(&mut self.cur);
        if kind != Kind::Pre {
            // baştaki/sondaki boşluk ve satır sonları
            while let Some((t, _)) = spans.first_mut() {
                let trimmed = t.trim_start_matches([' ', '\n']).to_string();
                if trimmed.is_empty() {
                    spans.remove(0);
                } else {
                    *t = trimmed;
                    break;
                }
            }
            while let Some((t, _)) = spans.last_mut() {
                let trimmed = t.trim_end_matches([' ', '\n']).to_string();
                if trimmed.is_empty() {
                    spans.pop();
                } else {
                    *t = trimmed;
                    break;
                }
            }
        } else {
            while let Some((t, _)) = spans.last_mut() {
                let trimmed = t.trim_end().to_string();
                if trimmed.is_empty() {
                    spans.pop();
                } else {
                    *t = trimmed;
                    break;
                }
            }
            if let Some((t, _)) = spans.first_mut() {
                if let Some(rest) = t.strip_prefix('\n') {
                    *t = rest.to_string();
                }
            }
        }
        let has_text = spans
            .iter()
            .any(|(t, _)| t.chars().any(|c| !c.is_whitespace()));
        if has_text {
            self.blocks.push(Block {
                kind,
                quote: self.quote,
                prefix: std::mem::take(&mut self.prefix),
                spans,
                list_item: self.in_li,
            });
        }
    }

    fn tag(&mut self, name: &str, closing: bool, self_closing: bool, attrs: &str) {
        let skip_tag = matches!(
            name,
            "script" | "style" | "head" | "title" | "noscript" | "template"
        );
        if skip_tag {
            if self_closing {
                return;
            }
            if closing {
                self.skip = self.skip.saturating_sub(1);
            } else {
                self.skip += 1;
            }
            return;
        }
        if self.skip > 0 {
            return;
        }
        match name {
            "p" | "div" | "section" | "article" | "header" | "footer" | "aside" | "nav"
            | "figure" | "figcaption" | "main" | "dd" | "dt" | "dl" | "tr" | "table" | "tbody"
            | "thead" | "tfoot" | "caption" | "center" | "address" | "details" | "summary"
            | "body" | "html" => self.flush(),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                self.kind = if closing {
                    None
                } else {
                    Some(Kind::Heading(name.as_bytes()[1] - b'0'))
                };
            }
            "br" => {
                let sty = self.sty();
                self.push_text("\n", sty);
            }
            "hr" => {
                self.flush();
                self.blocks.push(Block {
                    kind: Kind::Rule,
                    quote: self.quote,
                    prefix: String::new(),
                    spans: Vec::new(),
                    list_item: false,
                });
            }
            "ul" | "ol" => {
                self.flush();
                if closing {
                    self.lists.pop();
                } else if !self_closing {
                    self.lists.push(if name == "ol" {
                        Some(
                            attr(attrs, "start")
                                .and_then(|s| s.parse().ok())
                                .unwrap_or(1),
                        )
                    } else {
                        None
                    });
                }
            }
            "li" => {
                self.flush();
                if closing {
                    self.in_li = false;
                    return;
                }
                self.in_li = true;
                let depth = self.lists.len().saturating_sub(1);
                let indent = "  ".repeat(depth);
                self.prefix = match self.lists.last_mut() {
                    Some(Some(n)) => {
                        let s = format!("{indent}{n}. ");
                        *n += 1;
                        s
                    }
                    _ => format!("{indent}• "),
                };
            }
            "blockquote" => {
                self.flush();
                if closing {
                    self.quote = self.quote.saturating_sub(1);
                } else if !self_closing {
                    self.quote += 1;
                }
            }
            "pre" => {
                self.flush();
                if closing {
                    self.pre = self.pre.saturating_sub(1);
                } else if !self_closing {
                    self.pre += 1;
                }
            }
            "b" | "strong" => count(&mut self.bold, closing, self_closing),
            "i" | "em" | "cite" | "var" | "dfn" => count(&mut self.italic, closing, self_closing),
            "u" | "ins" => count(&mut self.underline, closing, self_closing),
            "code" | "kbd" | "samp" | "tt" => count(&mut self.code, closing, self_closing),
            "img" | "image" if !closing => {
                // köşeli parantez etikette yalnız başta olsun: görsel sayımı `[` ile yapılır
                let alt = attr(attrs, "alt")
                    .map(|a| {
                        decode_entities(&a)
                            .trim()
                            .replace('[', "(")
                            .replace(']', ")")
                    })
                    .filter(|a| !a.is_empty());
                let label = match alt {
                    Some(a) => format!("[görsel: {a}]"),
                    None => "[görsel]".to_string(),
                };
                let sty = Sty {
                    image: true,
                    ..Sty::default()
                };
                self.push_text(&label, sty);
            }
            "td" | "th" if !closing => {
                let sty = self.sty();
                let needs_sep = self
                    .cur
                    .last()
                    .map(|(t, _)| !t.ends_with(' ') && !t.ends_with('\n'))
                    .unwrap_or(false);
                if needs_sep {
                    self.push_text(" · ", Sty { sep: true, ..sty });
                }
            }
            _ => {}
        }
    }
}

fn count(n: &mut u32, closing: bool, self_closing: bool) {
    if self_closing {
        return;
    }
    if closing {
        *n = n.saturating_sub(1);
    } else {
        *n += 1;
    }
}

/// `name="değer"` / `name='değer'` / `name=değer` (büyük-küçük harf duyarsız).
fn attr(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(name) {
        let i = from + rel;
        from = i + name.len();
        let before_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'-';
        let mut j = i + name.len();
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if !before_ok || j >= bytes.len() || bytes[j] != b'=' {
            continue;
        }
        j += 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() {
            return None;
        }
        let q = bytes[j];
        let value = if q == b'"' || q == b'\'' {
            let end = attrs[j + 1..].find(q as char)? + j + 1;
            &attrs[j + 1..end]
        } else {
            let end = attrs[j..]
                .find(|c: char| c.is_whitespace() || c == '>')
                .map(|e| e + j)
                .unwrap_or(attrs.len());
            &attrs[j..end]
        };
        return Some(value.to_string());
    }
    None
}

pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after
            .char_indices()
            .take(32)
            .find(|&(_, c)| c == ';')
            .map(|(e, _)| e);
        let decoded = end.and_then(|e| entity(&after[..e]).map(|c| (c, e)));
        match decoded {
            Some((c, e)) => {
                if let Some(c) = c {
                    out.push(c);
                }
                rest = &after[e + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `Some(None)`: tanınan ama görünmeyen karakter (yumuşak tire vb.).
fn entity(name: &str) -> Option<Option<char>> {
    if let Some(num) = name.strip_prefix('#') {
        let v = if let Some(hex) = num.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            num.parse::<u32>().ok()?
        };
        return Some(match v {
            0xad | 0x200b..=0x200d | 0xfeff => None,
            _ => Some(char::from_u32(v).unwrap_or('\u{fffd}')),
        });
    }
    let c = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "shy" | "zwnj" | "zwj" => return Some(None),
        "hellip" => '…',
        "mdash" => '—',
        "ndash" => '–',
        "lsquo" => '‘',
        "rsquo" => '’',
        "sbquo" => '‚',
        "ldquo" => '“',
        "rdquo" => '”',
        "bdquo" => '„',
        "laquo" => '«',
        "raquo" => '»',
        "lsaquo" => '‹',
        "rsaquo" => '›',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "middot" => '·',
        "bull" => '•',
        "deg" => '°',
        "times" => '×',
        "divide" => '÷',
        "euro" => '€',
        "pound" => '£',
        "sect" => '§',
        "para" => '¶',
        "dagger" => '†',
        "Dagger" => '‡',
        "prime" => '′',
        "Prime" => '″',
        "thinsp" | "ensp" | "emsp" => ' ',
        "iexcl" => '¡',
        "iquest" => '¿',
        "ccedil" => 'ç',
        "Ccedil" => 'Ç',
        "ouml" => 'ö',
        "Ouml" => 'Ö',
        "uuml" => 'ü',
        "Uuml" => 'Ü',
        "auml" => 'ä',
        "Auml" => 'Ä',
        "eacute" => 'é',
        "Eacute" => 'É',
        "egrave" => 'è',
        "agrave" => 'à',
        "aacute" => 'á',
        "acirc" => 'â',
        "icirc" => 'î',
        "ucirc" => 'û',
        "ecirc" => 'ê',
        "ocirc" => 'ô',
        "iacute" => 'í',
        "oacute" => 'ó',
        "uacute" => 'ú',
        "ntilde" => 'ñ',
        "szlig" => 'ß',
        _ => return None,
    };
    Some(Some(c))
}

/// HTML gövdesini bloklara çevirir.
pub fn parse(html: &str) -> Vec<Block> {
    let mut p = Parser::default();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            p.text(rest);
            break;
        };
        if lt > 0 {
            p.text(&rest[..lt]);
        }
        let after = &rest[lt..];
        if let Some(comment) = after.strip_prefix("<!--") {
            rest = match comment.find("-->") {
                Some(e) => &comment[e + 3..],
                None => "",
            };
            continue;
        }
        let Some(gt) = find_tag_end(after) else {
            // kapanmamış '<': düz metin say
            p.text(after);
            break;
        };
        let inner = &after[1..gt];
        rest = &after[gt + 1..];
        if inner.starts_with('!') || inner.starts_with('?') {
            continue;
        }
        let closing = inner.starts_with('/');
        let body = inner.trim_start_matches('/');
        let self_closing = body.trim_end().ends_with('/');
        let name_end = body
            .find(|c: char| c.is_whitespace() || c == '/')
            .unwrap_or(body.len());
        let name = body[..name_end].to_ascii_lowercase();
        if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            p.text(&after[..gt + 1]);
            continue;
        }
        let attrs = &body[name_end..];
        let void = matches!(name.as_str(), "br" | "hr" | "img" | "wbr" | "meta" | "link");
        p.tag(&name, closing, self_closing || void, attrs);
    }
    p.flush();
    p.blocks
}

/// Tırnak içindeki `>` karakterlerini atlayarak etiket sonunu bulur.
fn find_tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices().skip(1) {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => return Some(i),
            None if c == '<' => return None,
            None => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(b: &[Block]) -> Vec<String> {
        b.iter().map(|b| b.plain_text()).collect()
    }

    #[test]
    fn paragraphs_and_whitespace() {
        let b = parse("<p>  Merhaba\n   dünya </p>\n<p>İkinci <b>kalın</b> söz.</p>");
        assert_eq!(texts(&b), vec!["Merhaba dünya", "İkinci kalın söz."]);
        assert_eq!(
            b[1].spans[1],
            (
                "kalın".to_string(),
                Sty {
                    bold: true,
                    ..Sty::default()
                }
            )
        );
    }

    #[test]
    fn headings_lists_quotes() {
        let b = parse(
            "<h2>Bölüm 1</h2><ul><li>bir</li><li>iki</li></ul>\
             <ol start=\"3\"><li>üç</li></ol><blockquote><p>alıntı</p></blockquote><hr/>",
        );
        assert_eq!(b[0].kind, Kind::Heading(2));
        assert_eq!(b[0].plain_text(), "Bölüm 1");
        assert_eq!(b[1].prefix, "• ");
        assert!(b[1].list_item);
        assert_eq!(b[3].prefix, "3. ");
        assert_eq!(b[4].quote, 1);
        assert_eq!(b[5].kind, Kind::Rule);
    }

    #[test]
    fn entities_and_breaks() {
        let b = parse("<p>A&amp;B &lt;c&gt; &#8217;x&#x2019; &hellip;&shy;<br/>yeni&nbsp;satır &bilinmez;</p>");
        assert_eq!(
            b[0].plain_text(),
            "A&B <c> ’x’ …\nyeni\u{a0}satır &bilinmez;"
        );
    }

    #[test]
    fn images_and_skipped_content() {
        let b = parse(
            "<style>p{}</style><p><img src=\"/book/1/a/x.png\" alt=\"Harita\"/></p>\
             <div><img src=\"\"></div><!-- yorum <p>gizli</p> --><script>alert(1)</script>",
        );
        assert_eq!(texts(&b), vec!["[görsel: Harita]", "[görsel]"]);
        assert!(b[0].spans[0].1.image);
    }

    #[test]
    fn pre_keeps_layout() {
        let b = parse("<pre>\nsatır 1\n  girintili\n</pre><p>son</p>");
        assert_eq!(b[0].kind, Kind::Pre);
        assert_eq!(b[0].plain_text(), "satır 1\n  girintili");
        assert_eq!(b[1].kind, Kind::Para);
    }

    #[test]
    fn inline_nesting_inside_words() {
        let b = parse("<p>Mer<i>ha</i>ba <a href=\"x\">bağ</a></p>");
        assert_eq!(b[0].plain_text(), "Merhaba bağ");
        assert_eq!(b[0].spans.len(), 3);
    }

    #[test]
    fn attr_parsing() {
        assert_eq!(
            attr(" src=\"a.png\" alt='Bir > iki'", "alt").as_deref(),
            Some("Bir > iki")
        );
        assert_eq!(attr(" data-alt=\"x\" alt=y", "alt").as_deref(), Some("y"));
        assert_eq!(attr(" src=\"a\"", "alt"), None);
        let b = parse("<p><img alt=\"a > b\" src=\"x\"></p>");
        assert_eq!(b[0].plain_text(), "[görsel: a > b]");
    }
}
