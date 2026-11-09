use std::collections::HashSet;

use ammonia::{Builder, UrlRelative};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::formats::epub::normalize_zip_path;

pub const FRAME_CSP: &str =
    "default-src 'none'; img-src 'self'; style-src 'self'; font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SrcMode {
    Epub,
    Mobi,
}

pub fn svg_image_to_img(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;

    while let Some(svg_start) = find_tag_ci(rest, "<svg") {
        out.push_str(&rest[..svg_start]);
        let after_svg = &rest[svg_start..];
        let Some(svg_end_rel) = find_close_svg_ci(after_svg) else {
            out.push_str(after_svg);
            return out;
        };
        let svg_block = &after_svg[..svg_end_rel];
        if let Some(href) = extract_svg_image_href(svg_block) {
            let escaped = escape_attr(&href);
            out.push_str(&format!(r#"<img src="{escaped}">"#));
        } else {
            out.push_str(svg_block);
        }
        rest = &after_svg[svg_end_rel..];
    }
    out.push_str(rest);
    out
}

fn find_tag_ci(haystack: &str, needle_lower: &str) -> Option<usize> {
    let nlen = needle_lower.len();
    let bytes = haystack.as_bytes();
    if bytes.len() < nlen {
        return None;
    }
    for i in 0..=(bytes.len() - nlen) {
        if bytes[i..i + nlen].eq_ignore_ascii_case(needle_lower.as_bytes()) {
            let next = bytes.get(i + nlen).copied();
            if matches!(
                next,
                None | Some(b' ' | b'\t' | b'\n' | b'\r' | b'>' | b'/')
            ) {
                return Some(i);
            }
        }
    }
    None
}

fn find_close_svg_ci(haystack: &str) -> Option<usize> {
    let needle = b"</svg>";
    let bytes = haystack.as_bytes();
    if bytes.len() < needle.len() {
        return None;
    }
    for i in 0..=(bytes.len() - needle.len()) {
        if bytes[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            return Some(i + needle.len());
        }
    }
    None
}

fn extract_svg_image_href(svg_block: &str) -> Option<String> {
    let img_pos = find_tag_ci(svg_block, "<image")?;
    let after_img = &svg_block[img_pos..];
    let tag_end = after_img.find('>')?;
    let tag_str = &after_img[..=tag_end];
    extract_attr_ci(tag_str, "xlink:href").or_else(|| extract_attr_ci(tag_str, "href"))
}

fn extract_attr_ci(tag: &str, attr_lower: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let alen = attr_lower.len();
    let mut i = 0;
    while i + alen <= bytes.len() {
        let prev_ok = i == 0
            || matches!(
                bytes[i - 1],
                b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'\'' | b'"'
            );
        if prev_ok && bytes[i..i + alen].eq_ignore_ascii_case(attr_lower.as_bytes()) {
            let mut j = i + alen;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                    let quote = bytes[j];
                    let start = j + 1;
                    let end_rel = tag[start..].find(quote as char)?;
                    let val = &tag[start..start + end_rel];
                    if !val.is_empty() {
                        return Some(val.to_string());
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn escape_attr(val: &str) -> String {
    let mut out = String::with_capacity(val.len());
    for c in val.chars() {
        match c {
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

fn rewrite_mobi_recindex(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = find_tag_ci(rest, "<img") {
        out.push_str(&rest[..pos]);
        let after = &rest[pos..];
        let Some(end) = after.find('>') else {
            out.push_str(after);
            return out;
        };
        let tag = &after[..=end];
        if let Some(rec) = extract_attr_ci(tag, "recindex") {
            let trimmed = rec.trim().trim_start_matches('0');
            if let Ok(idx) = trimmed.parse::<usize>() {
                if idx >= 1 {
                    out.push_str(&format!(r#"<img src="{idx}">"#));
                    rest = &after[end + 1..];
                    continue;
                }
            }
        }
        out.push_str(tag);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

pub fn sanitize_chapter(raw_html: &str, book_id: i64, chapter_dir: &str, mode: SrcMode) -> String {
    let step1 = svg_image_to_img(raw_html);
    let step2 = if mode == SrcMode::Mobi {
        rewrite_mobi_recindex(&step1)
    } else {
        step1
    };

    let ch_dir = chapter_dir.to_string();
    let mut builder = Builder::default();
    builder
        .clean_content_tags(HashSet::from(["script", "style", "title"]))
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .link_rel(Some("noopener noreferrer"))
        .url_relative(UrlRelative::PassThrough)
        .attribute_filter(move |element, attribute, value| {
            if attribute.eq_ignore_ascii_case("style") {
                return None;
            }
            match (element, attribute) {
                ("img", "src") => {
                    let v = value.trim();
                    if v.contains(':') || v.starts_with("//") {
                        return None;
                    }
                    match mode {
                        SrcMode::Epub => {
                            let norm = normalize_zip_path(&ch_dir, v)?;
                            let enc = utf8_percent_encode(&norm, NON_ALPHANUMERIC).to_string();
                            Some(format!("/api/books/{book_id}/asset?p={enc}").into())
                        }
                        SrcMode::Mobi => {
                            let trimmed = v.trim_start_matches('0');
                            let idx = trimmed.parse::<usize>().ok()?;
                            if idx == 0 {
                                return None;
                            }
                            Some(format!("/api/books/{book_id}/asset?p={idx}").into())
                        }
                    }
                }
                ("a", "href") => {
                    let v = value.trim();
                    let lower = v.to_ascii_lowercase();
                    if lower.starts_with("http://")
                        || lower.starts_with("https://")
                        || lower.starts_with("mailto:")
                    {
                        Some(v.to_string().into())
                    } else if has_scheme(v) {
                        None
                    } else {
                        Some("#".into())
                    }
                }
                ("a", "target") => None,
                _ => Some(value.into()),
            }
        });

    builder.clean(&step2).to_string()
}

fn has_scheme(url: &str) -> bool {
    for b in url.bytes() {
        match b {
            b':' => return true,
            b'/' | b'?' | b'#' => return false,
            _ => {}
        }
    }
    false
}

pub fn frame_document(body: &str, title: &str) -> String {
    let safe_title = escape_html_text(title);
    format!(
        "<!doctype html>\n<html lang=\"tr\">\n<head>\n<meta charset=\"utf-8\">\n<title>{safe_title}</title>\n<link rel=\"stylesheet\" href=\"/static/reader-fonts.css\">\n<link rel=\"stylesheet\" href=\"/static/frame.css\">\n</head>\n<body>\n{body}\n</body>\n</html>"
    )
}

fn escape_html_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}
