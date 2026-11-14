use std::{
    collections::HashMap,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use rand::RngCore;

use crate::http::{HErr, Http};

/// Sunucudaki `fold_char` ile aynı kural (Türkçe i/ı/İ/I -> i).
pub fn fold_char(c: char) -> char {
    match c {
        'İ' | 'I' | 'ı' => 'i',
        _ => c.to_lowercase().next().unwrap_or(c),
    }
}

pub fn fold_tr(s: &str) -> String {
    s.chars().map(fold_char).collect()
}

/// Yerel saat (UTC sn), sunucu ofseti uygulanmadan.
pub fn sys_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Sunucu saatine göre düzeltilmiş şimdi (sn).
pub fn now(h: &Http) -> i64 {
    sys_now() + h.clock_offset()
}

/// 16 rastgele bayttan 32 küçük harf hex.
pub fn client_id() -> String {
    let mut b = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

/// Sunucunun `frame_document` çıktısı sabittir: `<body>\n{gövde}\n</body>`.
pub fn extract_body(doc: &str) -> Result<&str, HErr> {
    let start = doc
        .find("<body>\n")
        .ok_or_else(|| HErr::Parse("bölümde <body> yok".to_string()))?
        + 7;
    let end = doc
        .rfind("\n</body>")
        .ok_or_else(|| HErr::Parse("bölümde </body> yok".to_string()))?;
    if end < start {
        return Err(HErr::Parse("bölüm gövdesi çözülemedi".to_string()));
    }
    Ok(&doc[start..end])
}

fn asset_prefix(book_id: i64) -> String {
    format!("src=\"/api/books/{book_id}/asset?p=")
}

/// Gövdedeki görsel `p` değerleri (yüzde-kodlu, tekilleştirilmiş, sıra korunur).
pub fn asset_refs(body: &str, book_id: i64) -> Vec<String> {
    let prefix = asset_prefix(book_id);
    let mut out: Vec<String> = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find(&prefix) {
        let after = &rest[i + prefix.len()..];
        let Some(end) = after.find('"') else { break };
        let p = &after[..end];
        if !out.iter().any(|x| x == p) {
            out.push(p.to_string());
        }
        rest = &after[end..];
    }
    out
}

/// `src="/api/books/{id}/asset?p=X"` -> `src="/book/{id}/a/{ad}"`; `None` (404) -> `src=""`.
pub fn rewrite_assets(
    body: &str,
    book_id: i64,
    map: &HashMap<String, Option<String>>,
) -> String {
    let prefix = asset_prefix(book_id);
    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(i) = rest.find(&prefix) {
        let after = &rest[i + prefix.len()..];
        let Some(end) = after.find('"') else { break };
        out.push_str(&rest[..i]);
        match map.get(&after[..end]) {
            Some(Some(name)) => {
                out.push_str(&format!("src=\"/book/{book_id}/a/{name}\""));
            }
            Some(None) => out.push_str("src=\"\""),
            None => out.push_str(&rest[i..i + prefix.len() + end + 1]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// En çok `n` iş parçacığıyla eşler; çıktı sırası girdi sırasıdır.
pub fn par_map<T: Send, R: Send>(items: Vec<T>, n: usize, f: impl Fn(T) -> R + Sync) -> Vec<R> {
    let len = items.len();
    if len == 0 {
        return Vec::new();
    }
    let queue = Mutex::new(items.into_iter().enumerate());
    let out: Mutex<Vec<(usize, R)>> = Mutex::new(Vec::with_capacity(len));
    std::thread::scope(|s| {
        for _ in 0..n.clamp(1, len) {
            s.spawn(|| loop {
                let next = queue.lock().unwrap_or_else(|e| e.into_inner()).next();
                let Some((i, item)) = next else { break };
                let r = f(item);
                out.lock().unwrap_or_else(|e| e.into_inner()).push((i, r));
            });
        }
    });
    let mut v = out.into_inner().unwrap_or_else(|e| e.into_inner());
    v.sort_by_key(|(i, _)| *i);
    v.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fold_turkish() {
        assert_eq!(fold_tr("İSTANBUL ıı"), "istanbul ii");
        assert_eq!(fold_tr("ISIK"), "isik");
    }

    #[test]
    fn extract_body_cuts_range() {
        let doc = "<!doctype html>\n<html><head></head>\n<body>\n<p>a</p>\n<p>b</p>\n</body>\n</html>";
        assert_eq!(extract_body(doc).unwrap(), "<p>a</p>\n<p>b</p>");
        assert_eq!(extract_body("<body>\n\n</body>").unwrap(), "");
        assert!(extract_body("<html><p>x</p></html>").is_err());
        assert!(extract_body("<body>\n<p>x</p>").is_err());
    }

    #[test]
    fn asset_refs_dedupes_and_filters_book() {
        let body = concat!(
            "<img src=\"/api/books/7/asset?p=a%2Epng\">",
            "<img src=\"/api/books/7/asset?p=b\">",
            "<img src=\"/api/books/7/asset?p=a%2Epng\">",
            "<img src=\"/api/books/77/asset?p=c\">",
        );
        assert_eq!(asset_refs(body, 7), vec!["a%2Epng", "b"]);
        assert_eq!(asset_refs(body, 77), vec!["c"]);
        assert!(asset_refs("<p>yok</p>", 7).is_empty());
    }

    #[test]
    fn rewrite_assets_only_touches_own_book() {
        let body = concat!(
            "<img src=\"/api/books/7/asset?p=a\" alt=\"x\">",
            "<img src=\"/api/books/7/asset?p=gone\">",
            "<img src=\"/api/books/77/asset?p=a\">",
        );
        let mut map = HashMap::new();
        map.insert("a".to_string(), Some("0123456789abcdef.png".to_string()));
        map.insert("gone".to_string(), None);
        let out = rewrite_assets(body, 7, &map);
        assert_eq!(
            out,
            concat!(
                "<img src=\"/book/7/a/0123456789abcdef.png\" alt=\"x\">",
                "<img src=\"\">",
                "<img src=\"/api/books/77/asset?p=a\">",
            )
        );
    }

    #[test]
    fn par_map_keeps_order() {
        let items: Vec<i32> = (0..50).collect();
        let out = par_map(items, 4, |x| {
            std::thread::sleep(std::time::Duration::from_millis(((50 - x) % 5) as u64));
            x * 2
        });
        assert_eq!(out, (0..50).map(|x| x * 2).collect::<Vec<_>>());
        assert!(par_map(Vec::<i32>::new(), 4, |x| x).is_empty());
    }

    #[test]
    fn client_id_is_32_hex() {
        let id = client_id();
        assert_eq!(id.len(), 32);
        assert!(id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    }
}
