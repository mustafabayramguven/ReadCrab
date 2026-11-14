use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::{
    download,
    error::into_core,
    http::{self, HErr, SearchDto, TocDto},
    util, Core, CoreError, SearchHitItem, SearchResults, TextInfo, TocEntry,
};

/// Parça (sayfa/bölüm/kapak) indirme hatası: ağ-HTTP ya da yerel depolama.
pub(crate) enum PErr {
    Http(HErr),
    Local(CoreError),
}

impl From<HErr> for PErr {
    fn from(e: HErr) -> Self {
        PErr::Http(e)
    }
}

impl From<std::io::Error> for PErr {
    fn from(e: std::io::Error) -> Self {
        PErr::Local(CoreError::Storage(e.to_string()))
    }
}

impl From<CoreError> for PErr {
    fn from(e: CoreError) -> Self {
        PErr::Local(e)
    }
}

impl PErr {
    pub(crate) fn into_core(self) -> CoreError {
        match self {
            PErr::Http(e) => into_core(e),
            PErr::Local(e) => e,
        }
    }

    /// Okuyucu çağrılarında ağ erişimi yoksa `Offline`.
    fn into_reader_err(self) -> CoreError {
        match self {
            PErr::Http(HErr::Net(_)) => CoreError::Offline,
            other => other.into_core(),
        }
    }
}

fn path_string(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

fn book_dir(core: &Core, book_id: i64) -> PathBuf {
    core.root.join("books").join(book_id.to_string())
}

fn page_file(core: &Core, book_id: i64, page: i64) -> PathBuf {
    book_dir(core, book_id).join(format!("p{page}"))
}

fn chapter_file(core: &Core, book_id: i64, n: i64) -> PathBuf {
    book_dir(core, book_id).join(format!("c{n}.html"))
}

fn cover_file(core: &Core, book_id: i64, version: i64) -> PathBuf {
    core.root
        .join("covers")
        .join(format!("{book_id}-{version}.jpg"))
}

/// Önce `.tmp`, sonra `rename`; eş zamanlı iki istek aynı geçici dosyayı ezmesin diye ad rastgeledir.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{}.tmp", &util::client_id()[..8]));
    let tmp = PathBuf::from(name);
    fs::write(&tmp, bytes)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

/// Parça dosyası yazıldıktan sonra `parts` satırı eklenir ve indirme sayaçları güncellenir.
fn store_part(core: &Core, book_id: i64, idx: i64, bytes: i64) -> Result<(), CoreError> {
    let now = core.now();
    core.with_db(|c| {
        c.execute(
            "INSERT OR REPLACE INTO parts(book_id, idx, bytes) VALUES (?1, ?2, ?3)",
            params![book_id, idx, bytes],
        )?;
        download::refresh_counts(c, book_id, now)?;
        Ok(())
    })
}

/// Çizgi roman sayfası; baytı döner.
pub(crate) fn fetch_page(core: &Core, book_id: i64, page: i64) -> Result<i64, PErr> {
    let r = core
        .http
        .get(&format!("/api/books/{book_id}/pages/{page}"))?;
    write_atomic(&page_file(core, book_id, page), &r.body)?;
    let n = r.body.len() as i64;
    store_part(core, book_id, page, n)?;
    Ok(n)
}

pub(crate) fn fetch_cover(core: &Core, book_id: i64, version: i64) -> Result<PathBuf, PErr> {
    let path = cover_file(core, book_id, version);
    let r = core.http.get(&format!("/api/books/{book_id}/cover"))?;
    write_atomic(&path, &r.body)?;
    // aynı kitabın eski sürüm kapakları silinir
    let keep = format!("{book_id}-{version}.jpg");
    let prefix = format!("{book_id}-");
    if let Ok(rd) = fs::read_dir(core.root.join("covers")) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) && name.ends_with(".jpg") && name != keep {
                let _ = fs::remove_file(e.path());
            }
        }
    }
    Ok(path)
}

fn image_ext(content_type: &str) -> &'static str {
    match content_type.to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "img",
    }
}

/// Bölüm + görselleri indirir, gövdeyi yeniden yazar; toplam baytı döner (bölüm 4.6).
pub(crate) fn fetch_chapter(core: &Core, book_id: i64, n: i32) -> Result<i64, PErr> {
    let reply = core
        .http
        .get(&format!("/api/books/{book_id}/chapters/{n}"))?;
    let doc = String::from_utf8_lossy(&reply.body).into_owned();
    let body = util::extract_body(&doc)?;

    let dir = book_dir(core, book_id);
    let mut total: i64 = 0;
    let mut map: HashMap<String, Option<String>> = HashMap::new();
    for p in util::asset_refs(body, book_id) {
        match core
            .http
            .get(&format!("/api/books/{book_id}/asset?p={p}"))
        {
            Ok(img) => {
                let hash = hex::encode(Sha256::digest(p.as_bytes()));
                let name = format!("{}.{}", &hash[..16], image_ext(&img.content_type));
                write_atomic(&dir.join("a").join(&name), &img.body)?;
                total += img.body.len() as i64;
                map.insert(p, Some(name));
            }
            // görsel yok: src boşaltılır
            Err(HErr::Status { code: 404, .. }) => {
                map.insert(p, None);
            }
            Err(e) => return Err(e.into()),
        }
    }

    let out = util::rewrite_assets(body, book_id, &map);
    write_atomic(&chapter_file(core, book_id, n as i64), out.as_bytes())?;
    total += out.len() as i64;
    store_part(core, book_id, n as i64, total)?;
    Ok(total)
}

pub(crate) fn chapter_count(page_count: i64, toc_len: usize) -> i32 {
    if page_count > 0 {
        page_count as i32
    } else if toc_len > 0 {
        toc_len as i32
    } else {
        1
    }
}

fn load_toc(core: &Core, book_id: i64) -> Result<Vec<TocEntry>, CoreError> {
    core.with_db(|c| {
        let mut st = c.prepare("SELECT idx, title FROM toc WHERE book_id = ?1 ORDER BY idx")?;
        let rows = st.query_map([book_id], |r| {
            Ok(TocEntry {
                index: r.get::<_, i64>(0)? as i32,
                title: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })
}

fn fetch_toc(core: &Core, book_id: i64) -> Result<Vec<TocEntry>, CoreError> {
    let dto: TocDto = core
        .http
        .get_json(&format!("/api/books/{book_id}/toc"))
        .map_err(into_core)?;
    core.with_db(|c| {
        let tx = c.transaction()?;
        for ch in &dto.chapters {
            tx.execute(
                "INSERT OR REPLACE INTO toc(book_id, idx, title) VALUES (?1, ?2, ?3)",
                params![book_id, ch.index, ch.title],
            )?;
        }
        tx.commit()?;
        Ok(())
    })?;
    Ok(dto
        .chapters
        .into_iter()
        .map(|c| TocEntry {
            index: c.index as i32,
            title: c.title,
        })
        .collect())
}

/// `toc` tablosu doluysa ondan, boşsa sunucudan çekip yazar.
pub(crate) fn ensure_toc(core: &Core, book_id: i64) -> Result<Vec<TocEntry>, CoreError> {
    let cur = load_toc(core, book_id)?;
    if cur.is_empty() {
        fetch_toc(core, book_id)
    } else {
        Ok(cur)
    }
}

#[uniffi::export]
impl Core {
    /// `covers/{id}-{v}.jpg`; yoksa ağdan indirir.
    pub fn cover_path(&self, book_id: i64, version: i64) -> Result<String, CoreError> {
        let path = cover_file(self, book_id, version);
        if path.exists() {
            return Ok(path_string(&path));
        }
        let p = fetch_cover(self, book_id, version).map_err(PErr::into_reader_err)?;
        Ok(path_string(&p))
    }

    /// `books/{id}/p{page}`; yoksa ağdan indirir ve `parts`'a ekler.
    pub fn page_path(&self, book_id: i64, page: i32) -> Result<String, CoreError> {
        if page < 0 {
            return Err(CoreError::InvalidInput("Geçersiz sayfa.".to_string()));
        }
        let path = page_file(self, book_id, page as i64);
        if path.exists() {
            return Ok(path_string(&path));
        }
        fetch_page(self, book_id, page as i64).map_err(PErr::into_reader_err)?;
        Ok(path_string(&path))
    }

    pub fn text_info(&self, book_id: i64) -> Result<TextInfo, CoreError> {
        let page_count: i64 = self.with_db(|c| {
            c.query_row(
                "SELECT page_count FROM books WHERE id = ?1",
                [book_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(CoreError::NotFound)
        })?;
        let mut toc = match ensure_toc(self, book_id) {
            Ok(t) => t,
            Err(CoreError::Unauthorized) => return Err(CoreError::Unauthorized),
            // ağ yok ya da sunucu içindekileri vermiyor: varsayılan girdiler
            Err(_) => Vec::new(),
        };
        let chapter_count = chapter_count(page_count, toc.len());
        if toc.is_empty() {
            toc = (0..chapter_count)
                .map(|i| TocEntry {
                    index: i,
                    title: format!("Bölüm {}", i + 1),
                })
                .collect();
        }
        Ok(TextInfo { chapter_count, toc })
    }

    /// `books/{id}/c{n}.html` yoksa bölüm + görselleri indirip yazar.
    pub fn ensure_chapter(&self, book_id: i64, chapter: i32) -> Result<(), CoreError> {
        if chapter < 0 {
            return Err(CoreError::InvalidInput("Geçersiz bölüm.".to_string()));
        }
        if chapter_file(self, book_id, chapter as i64).exists() {
            return Ok(());
        }
        fetch_chapter(self, book_id, chapter)
            .map(|_| ())
            .map_err(PErr::into_reader_err)
    }

    /// Yalnız çevrimiçi.
    pub fn search_text(&self, book_id: i64, query: String) -> Result<SearchResults, CoreError> {
        let q = query.split_whitespace().collect::<Vec<_>>().join(" ");
        if !(2..=64).contains(&q.chars().count()) {
            return Err(CoreError::InvalidInput(
                "Arama sorgusu 2-64 karakter olmalıdır.".to_string(),
            ));
        }
        let dto: SearchDto = self
            .http
            .get_json_timeout(
                &format!(
                    "/api/books/{book_id}/search?q={}",
                    http::encode_component(&q)
                ),
                Duration::from_secs(20),
            )
            .map_err(into_core)?;
        Ok(SearchResults {
            hits: dto
                .results
                .into_iter()
                .map(|h| SearchHitItem {
                    ch: h.ch as i32,
                    p: h.p,
                    before: h.before,
                    hit: h.hit,
                    after: h.after,
                })
                .collect(),
            truncated: dto.truncated,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_count_rules() {
        assert_eq!(chapter_count(5, 9), 5);
        assert_eq!(chapter_count(0, 9), 9);
        assert_eq!(chapter_count(0, 0), 1);
    }

    #[test]
    fn image_ext_mapping() {
        assert_eq!(image_ext("image/png"), "png");
        assert_eq!(image_ext("IMAGE/JPEG"), "jpg");
        assert_eq!(image_ext("image/svg+xml"), "svg");
        assert_eq!(image_ext("application/octet-stream"), "img");
    }

    #[test]
    fn write_atomic_replaces_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a").join("f");
        write_atomic(&p, b"bir").unwrap();
        write_atomic(&p, b"iki").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"iki");
        let n = fs::read_dir(dir.path().join("a")).unwrap().count();
        assert_eq!(n, 1, "geçici dosya kalmamalı");
    }

    #[test]
    fn text_info_defaults_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::new(dir.path().to_string_lossy().to_string()).unwrap();
        core.with_db(|c| {
            c.execute(
                "INSERT INTO books(id, series_id, ord, title, folded_title, format, page_count, size)
                 VALUES (1, 1, 0, 'K', 'k', 'epub', 3, 10)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        // sunucuya ulaşılamıyor (kapalı port): ağ hatası yutulur, varsayılan girdiler üretilir
        core.http.set_session("http://127.0.0.1:1", "tok");
        let info = core.text_info(1).unwrap();
        assert_eq!(info.chapter_count, 3);
        assert_eq!(info.toc.len(), 3);
        assert_eq!(info.toc[2].title, "Bölüm 3");
        assert!(matches!(core.text_info(99), Err(CoreError::NotFound)));
    }
}
