use std::{
    collections::{HashMap, HashSet},
    fmt::Write as _,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering},
        Arc,
    },
    time::UNIX_EPOCH,
};

use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};

use crate::{
    app::AppState,
    auth::unix_now,
    error::AppError,
    formats::{
        comic::comic_pages,
        cover::{publish_tmp, render_cover, BookRow},
        epub::epub_open,
        mobi::mobi_open,
        natural::natural_cmp,
        Format, FormatError,
    },
};

#[derive(Default)]
pub struct ScanStatus {
    pub running: AtomicBool,
    pub done: AtomicU32,
    pub total: AtomicU32,
    pub last_finished_at: AtomicI64,
}

impl ScanStatus {
    pub fn new() -> Self {
        Self::default()
    }
}

struct ScanGuard(Arc<ScanStatus>);

impl Drop for ScanGuard {
    fn drop(&mut self) {
        self.0.last_finished_at.store(unix_now(), Ordering::SeqCst);
        self.0.running.store(false, Ordering::SeqCst);
    }
}

pub fn start_scan(state: AppState) -> bool {
    if state
        .scan
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return false;
    }
    state.scan.done.store(0, Ordering::SeqCst);
    state.scan.total.store(0, Ordering::SeqCst);
    tokio::spawn(async move {
        let _guard = ScanGuard(state.scan.clone());
        tokio::task::yield_now().await;
        run_scan(&state).await;
    });
    true
}

pub async fn scan_once(state: &AppState) -> bool {
    if state
        .scan
        .running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return false;
    }
    state.scan.done.store(0, Ordering::SeqCst);
    state.scan.total.store(0, Ordering::SeqCst);
    let _guard = ScanGuard(state.scan.clone());
    run_scan(state).await;
    true
}

#[derive(Clone, Debug)]
struct DiscoveredFile {
    full_path: PathBuf,
    rel_path: String,
    series_rel: String,
    series_name: String,
    file_name: String,
    default_title: String,
    format: Format,
    sort_index: i64,
    size: i64,
    mtime: i64,
}

async fn run_scan(state: &AppState) {
    let books_dir = state.cfg.books_dir.clone();
    let covers_dir = state.cfg.data_dir.join("covers");

    let Ok((discovered, read_dir_errors)) =
        tokio::task::spawn_blocking(move || walk_books(&books_dir)).await
    else {
        return;
    };

    state
        .scan
        .total
        .store(discovered.len() as u32, Ordering::SeqCst);

    if read_dir_errors == 0 {
        let seen_rel_paths: HashSet<String> =
            discovered.iter().map(|d| d.rel_path.clone()).collect();
        let _ = state
            .db
            .run(move |conn| {
                let mut stmt = conn.prepare("SELECT id, rel_path FROM books WHERE missing = 0")?;
                let rows = stmt
                    .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                drop(stmt);
                for (id, rel) in rows {
                    if !seen_rel_paths.contains(&rel) {
                        conn.execute("UPDATE books SET missing = 1 WHERE id = ?1", [id])?;
                    }
                }
                Ok(())
            })
            .await;
    }

    for disc in discovered {
        process_single_book(state, &disc, &covers_dir).await;
        state.scan.done.fetch_add(1, Ordering::SeqCst);
    }
}

async fn process_single_book(state: &AppState, disc: &DiscoveredFile, covers_dir: &Path) {
    let series_rel = disc.series_rel.clone();
    let series_name = disc.series_name.clone();
    let rel_path = disc.rel_path.clone();
    let disc_size = disc.size;
    let disc_mtime = disc.mtime;
    let disc_sort = disc.sort_index;
    let now = unix_now();

    let Ok((series_id, unchanged)) = state
        .db
        .run(move |conn| {
            conn.execute(
                "INSERT INTO series(rel_path, name, added_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(rel_path) DO UPDATE SET name = excluded.name",
                rusqlite::params![series_rel, series_name, now],
            )?;
            let series_id: i64 = conn.query_row(
                "SELECT id FROM series WHERE rel_path = ?1",
                rusqlite::params![series_rel],
                |r| r.get(0),
            )?;

            let existing: Option<(i64, i64, i64, i64, i64, i64, Option<String>)> = conn
                .query_row(
                    "SELECT id, series_id, size, mtime, missing, sort_index, error
                     FROM books WHERE rel_path = ?1",
                    rusqlite::params![rel_path],
                    |r| {
                        Ok((
                            r.get(0)?,
                            r.get(1)?,
                            r.get(2)?,
                            r.get(3)?,
                            r.get(4)?,
                            r.get(5)?,
                            r.get(6)?,
                        ))
                    },
                )
                .optional()?;

            if let Some((book_id, old_series_id, size, mtime, missing, sort_idx, error)) =
                existing
            {
                // hatalı kayıt geçici nedenle olabilir (zaman aşımı), yeniden denenir
                if size == disc_size && mtime == disc_mtime && missing == 0 && error.is_none() {
                    if old_series_id != series_id || sort_idx != disc_sort {
                        conn.execute(
                            "UPDATE books SET series_id = ?1, sort_index = ?2 WHERE id = ?3",
                            rusqlite::params![series_id, disc_sort, book_id],
                        )?;
                    }
                    return Ok((series_id, true));
                }
            }
            Ok((series_id, false))
        })
        .await
    else {
        return;
    };

    if unchanged {
        return;
    }

    let path_for_hash = disc.full_path.clone();
    let content_id =
        match tokio::task::spawn_blocking(move || compute_content_id(&path_for_hash)).await {
            Ok(Ok(cid)) => cid,
            Ok(Err(e)) => {
                tracing::warn!("içerik özeti hesaplanamadı ({:?}): {e}", disc.full_path);
                return;
            }
            Err(e) => {
                tracing::warn!("içerik özeti görevi başarısız ({:?}): {e}", disc.full_path);
                return;
            }
        };

    let (title, page_count, err_str) =
        extract_metadata(&disc.full_path, disc.format, &disc.default_title).await;
    if let Some(reason) = err_str {
        tracing::warn!("kitap üstverisi okunamadı ({:?}): {reason}", disc.full_path);
    }

    let rel_path = disc.rel_path.clone();
    let fmt_str = disc.format.as_str().to_string();
    let disc_sort = disc.sort_index;
    let disc_size = disc.size;
    let disc_mtime = disc.mtime;
    let error_col = err_str.map(str::to_owned);

    let Ok(book_id) = state
        .db
        .run(move |conn| {
            upsert_or_revive_book(
                conn,
                series_id,
                &rel_path,
                &content_id,
                &title,
                &fmt_str,
                disc_sort,
                disc_size,
                disc_mtime,
                page_count,
                error_col.as_deref(),
                now,
            )
        })
        .await
    else {
        return;
    };

    // kapak önce geçici dosyaya üretilir; yayımlama cover_lock altında, özel kapak yeniden okunarak
    let rendered = if err_str.is_none() {
        let row = BookRow {
            id: book_id,
            rel_path: disc.rel_path.clone(),
            format: disc.format,
        };
        render_cover(&state.cfg.books_dir, &row, covers_dir).await
    } else {
        None
    };

    let _guard = state.cover_lock.lock().await;
    let custom: i64 = state
        .db
        .run(move |conn| {
            Ok(conn.query_row(
                "SELECT custom_cover FROM books WHERE id = ?1",
                [book_id],
                |r| r.get(0),
            )?)
        })
        .await
        .unwrap_or(0);
    if custom == 1 {
        if let Some(tmp) = rendered {
            let _ = std::fs::remove_file(tmp);
        }
        return;
    }

    let has_cover = match rendered {
        Some(tmp) => {
            let dst = covers_dir.join(format!("{book_id}.jpg"));
            publish_tmp(&tmp, &dst).is_ok()
        }
        None => false,
    };

    // yeniden çıkarılan kapakta sürüm artar (tarayıcıdaki ?v= bayatlamaz)
    let bump = i64::from(has_cover);
    let _ = state
        .db
        .run(move |conn| {
            conn.execute(
                "UPDATE books SET has_cover = ?1, cover_version = cover_version + ?1 WHERE id = ?2",
                rusqlite::params![bump, book_id],
            )?;
            Ok(())
        })
        .await;
}

#[allow(clippy::too_many_arguments)]
fn upsert_or_revive_book(
    conn: &mut rusqlite::Connection,
    series_id: i64,
    rel_path: &str,
    content_id: &str,
    title: &str,
    fmt_str: &str,
    sort_index: i64,
    size: i64,
    mtime: i64,
    page_count: i64,
    error: Option<&str>,
    now: i64,
) -> Result<i64, AppError> {
    let by_rel: Option<i64> = conn
        .query_row(
            "SELECT id FROM books WHERE rel_path = ?1",
            rusqlite::params![rel_path],
            |r| r.get(0),
        )
        .optional()?;

    if let Some(id) = by_rel {
        conn.execute(
            "UPDATE books
             SET series_id = ?1, content_id = ?2, title = ?3, format = ?4,
                 sort_index = ?5, size = ?6, mtime = ?7, page_count = ?8,
                 error = ?9, missing = 0
             WHERE id = ?10",
            rusqlite::params![
                series_id, content_id, title, fmt_str, sort_index, size, mtime, page_count, error,
                id
            ],
        )?;
        return Ok(id);
    }

    let by_content: Option<i64> = conn
        .query_row(
            "SELECT id FROM books WHERE content_id = ?1 AND missing = 1 ORDER BY id ASC LIMIT 1",
            rusqlite::params![content_id],
            |r| r.get(0),
        )
        .optional()?;

    if let Some(id) = by_content {
        conn.execute(
            "UPDATE books
             SET series_id = ?1, rel_path = ?2, title = ?3, format = ?4,
                 sort_index = ?5, size = ?6, mtime = ?7, page_count = ?8,
                 error = ?9, missing = 0
             WHERE id = ?10",
            rusqlite::params![
                series_id, rel_path, title, fmt_str, sort_index, size, mtime, page_count, error, id
            ],
        )?;
        return Ok(id);
    }

    conn.execute(
        "INSERT INTO books(
            series_id, rel_path, content_id, title, format,
            sort_index, size, mtime, page_count, has_cover,
            error, missing, added_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10, 0, ?11)",
        rusqlite::params![
            series_id, rel_path, content_id, title, fmt_str, sort_index, size, mtime, page_count,
            error, now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

async fn extract_metadata(
    full_path: &Path,
    format: Format,
    default_title: &str,
) -> (String, i64, Option<&'static str>) {
    let fallback = default_title.to_string();
    match format {
        Format::Cbz | Format::Zip | Format::Rar | Format::Cbr => {
            match comic_pages(full_path, format).await {
                Ok(pages) if !pages.is_empty() => (fallback, pages.len() as i64, None),
                Ok(_) => (fallback, 0, Some("corrupt")),
                Err(FormatError::Drm) => (fallback, 0, Some("drm")),
                Err(_) => (fallback, 0, Some("corrupt")),
            }
        }
        Format::Epub => {
            let p = full_path.to_path_buf();
            let fb = fallback.clone();
            tokio::task::spawn_blocking(move || match epub_open(&p) {
                Ok(info) => {
                    let title = info
                        .title
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .unwrap_or(fb);
                    if info.spine.is_empty() {
                        (title, 0, Some("corrupt"))
                    } else {
                        (title, info.spine.len() as i64, None)
                    }
                }
                Err(FormatError::Drm) => (fb, 0, Some("drm")),
                Err(_) => (fb, 0, Some("corrupt")),
            })
            .await
            .unwrap_or((fallback, 0, Some("corrupt")))
        }
        Format::Mobi => {
            let p = full_path.to_path_buf();
            let fb = fallback.clone();
            tokio::task::spawn_blocking(move || match mobi_open(&p) {
                Ok(book) => {
                    let trimmed = book.title.trim();
                    let title = if trimmed.is_empty() {
                        fb
                    } else {
                        trimmed.to_string()
                    };
                    if book.chapters.is_empty() {
                        (title, 0, Some("corrupt"))
                    } else {
                        (title, book.chapters.len() as i64, None)
                    }
                }
                Err(FormatError::Drm) => (fb, 0, Some("drm")),
                Err(_) => (fb, 0, Some("corrupt")),
            })
            .await
            .unwrap_or((fallback, 0, Some("corrupt")))
        }
    }
}

pub fn compute_content_id(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());

    if size <= 128 * 1024 {
        let mut buf = Vec::with_capacity(size as usize);
        file.read_to_end(&mut buf)?;
        hasher.update(&buf);
    } else {
        let mut buf = vec![0u8; 64 * 1024];
        file.read_exact(&mut buf)?;
        hasher.update(&buf);
        file.seek(SeekFrom::End(-(64 * 1024)))?;
        file.read_exact(&mut buf)?;
        hasher.update(&buf);
    }

    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for b in digest {
        let _ = write!(&mut hex, "{b:02x}");
    }
    Ok(hex)
}

fn walk_books(root: &Path) -> (Vec<DiscoveredFile>, usize) {
    let mut raw = Vec::new();
    let mut read_dir_errors = 0;
    walk_dir_rec(root, root, "", &mut raw, &mut read_dir_errors);

    let mut by_series: HashMap<String, Vec<DiscoveredFile>> = HashMap::new();
    for item in raw {
        by_series
            .entry(item.series_rel.clone())
            .or_default()
            .push(item);
    }

    let mut series_keys: Vec<String> = by_series.keys().cloned().collect();
    series_keys.sort_by(|a, b| natural_cmp(a, b));

    let mut out = Vec::new();
    for key in series_keys {
        if let Some(mut files) = by_series.remove(&key) {
            files.sort_by(|a, b| natural_cmp(&a.file_name, &b.file_name));
            for (idx, mut f) in files.into_iter().enumerate() {
                f.sort_index = idx as i64;
                out.push(f);
            }
        }
    }
    (out, read_dir_errors)
}

fn walk_dir_rec(
    root: &Path,
    dir: &Path,
    rel_dir: &str,
    out: &mut Vec<DiscoveredFile>,
    read_dir_errors: &mut usize,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            tracing::error!("dizin okunamadı ({:?}): {err}", dir);
            *read_dir_errors += 1;
            return;
        }
    };

    let mut subdirs = Vec::new();
    for entry_res in entries {
        let entry = match entry_res {
            Ok(e) => e,
            Err(err) => {
                tracing::error!("dizin girdisi okunamadı ({:?}): {err}", dir);
                *read_dir_errors += 1;
                continue;
            }
        };
        let path = entry.path();
        let ft = match entry.file_type() {
            Ok(ft) => ft,
            Err(err) => {
                tracing::error!("dosya türü okunamadı ({:?}): {err}", path);
                *read_dir_errors += 1;
                continue;
            }
        };
        if ft.is_symlink() {
            continue;
        }
        if path.to_str().is_none() {
            tracing::warn!("UTF-8 olmayan dosya adı atlandı: {:?}", path);
            continue;
        }
        let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
            tracing::warn!("UTF-8 olmayan dosya adı atlandı: {:?}", path);
            continue;
        };
        if file_name.starts_with('.') {
            continue;
        }

        if ft.is_dir() {
            let sub_rel = if rel_dir.is_empty() {
                file_name
            } else {
                format!("{rel_dir}/{file_name}")
            };
            subdirs.push((sub_rel, path));
        } else if ft.is_file() {
            let Some(format) = Format::from_path(&path) else {
                continue;
            };
            let meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!("dosya bilgisi okunamadı ({:?}): {e}", path);
                    continue;
                }
            };
            let size = meta.len() as i64;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let rel_path = if rel_dir.is_empty() {
                file_name.clone()
            } else {
                format!("{rel_dir}/{file_name}")
            };
            let series_rel = rel_dir.to_string();
            let series_name = if series_rel.is_empty() {
                "Diğer".to_string()
            } else {
                series_rel.split('/').collect::<Vec<_>>().join(" / ")
            };
            let default_title = Path::new(&file_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file_name)
                .to_string();

            out.push(DiscoveredFile {
                full_path: root.join(&rel_path),
                rel_path,
                series_rel,
                series_name,
                file_name,
                default_title,
                format,
                sort_index: 0,
                size,
                mtime,
            });
        }
    }

    subdirs.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    for (sub_rel, sub_path) in subdirs {
        walk_dir_rec(root, &sub_path, &sub_rel, out, read_dir_errors);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn content_id_small_and_large_files() {
        let tmp = tempfile::tempdir().unwrap();
        let p1 = tmp.path().join("a.bin");
        let p2 = tmp.path().join("b.bin");
        std::fs::write(&p1, b"hello world").unwrap();
        std::fs::write(&p2, b"hello world").unwrap();
        let id1 = compute_content_id(&p1).unwrap();
        let id2 = compute_content_id(&p2).unwrap();
        assert_eq!(id1.len(), 64);
        assert_eq!(id1, id2);

        let p3 = tmp.path().join("large.bin");
        let mut f = File::create(&p3).unwrap();
        f.write_all(&vec![b'A'; 200 * 1024]).unwrap();
        let id3 = compute_content_id(&p3).unwrap();
        assert_eq!(id3.len(), 64);
        assert_ne!(id1, id3);
    }
}
