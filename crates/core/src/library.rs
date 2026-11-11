use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::{
    db,
    error::into_core,
    http::{HErr, ProgressDto, SeriesDetailDto, SeriesListDto, SeriesPageDto},
    sync, util, BookDetail, BookItem, ContinueItem, Core, CoreError, DownloadInfo, DownloadState,
    ReadStatus, SeriesDetail, SeriesItem,
};

pub(crate) fn is_comic(format: &str) -> bool {
    matches!(format, "cbz" | "zip" | "rar" | "cbr")
}

const BOOK_SELECT: &str = "SELECT b.id, b.series_id, b.title, b.format, b.page_count, b.size,
        b.error, b.cover_v, COALESCE(p.status, 'unread'),
        CASE WHEN p.status = 'finished' THEN 1.0 ELSE COALESCE(p.percent, 0.0) END,
        d.state, d.parts_done, d.parts_total, d.bytes, d.error
    FROM books b
    LEFT JOIN progress p ON p.book_id = b.id
    LEFT JOIN downloads d ON d.book_id = b.id";

fn book_item(r: &Row<'_>) -> rusqlite::Result<BookItem> {
    let id: i64 = r.get(0)?;
    let format: String = r.get(3)?;
    let comic = is_comic(&format);
    let status: String = r.get(8)?;
    let state: Option<String> = r.get(10)?;
    Ok(BookItem {
        id,
        series_id: r.get(1)?,
        title: r.get(2)?,
        format,
        is_comic: comic,
        page_count: r.get::<_, i64>(4)? as i32,
        size: r.get(5)?,
        error: r.get(6)?,
        cover_v: r.get(7)?,
        status: ReadStatus::from_db(&status),
        percent: r.get(9)?,
        download: DownloadInfo {
            book_id: id,
            state: DownloadState::from_db(state.as_deref()),
            parts_done: r.get::<_, Option<i64>>(11)?.unwrap_or(0) as i32,
            parts_total: r.get::<_, Option<i64>>(12)?.unwrap_or(0) as i32,
            bytes: r.get::<_, Option<i64>>(13)?.unwrap_or(0),
            error: r.get(14)?,
        },
    })
}

#[uniffi::export]
impl Core {
    /// Yalnız yerel veri. Sorgu, seri adında veya herhangi bir kitap başlığında geçer.
    pub fn list_series(
        &self,
        query: String,
        only_downloaded: bool,
    ) -> Result<Vec<SeriesItem>, CoreError> {
        let q = util::fold_tr(query.trim());
        self.with_db(|c| {
            let mut st = c.prepare(
                "SELECT s.id, s.name, s.cover_book_id, s.cover_v,
                        COUNT(b.id),
                        COALESCE(SUM(CASE WHEN p.status = 'finished' THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN p.status = 'reading' THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN d.state = 'done' THEN 1 ELSE 0 END), 0),
                        COALESCE(AVG(CASE WHEN p.status = 'finished' THEN 1.0
                                          ELSE COALESCE(p.percent, 0.0) END), 0.0),
                        NULLIF(MAX(p.updated_at), 0)
                 FROM series s
                 LEFT JOIN books b ON b.series_id = s.id
                 LEFT JOIN progress p ON p.book_id = b.id
                 LEFT JOIN downloads d ON d.book_id = b.id
                 WHERE (?1 = ''
                        OR instr(s.folded_name, ?1) > 0
                        OR EXISTS (SELECT 1 FROM books b2
                                   WHERE b2.series_id = s.id AND instr(b2.folded_title, ?1) > 0))
                 GROUP BY s.id
                 HAVING (?2 = 0
                         OR COALESCE(SUM(CASE WHEN d.state = 'done' THEN 1 ELSE 0 END), 0) > 0)
                 ORDER BY s.ord",
            )?;
            let rows = st.query_map(params![q, only_downloaded as i64], |r| {
                Ok(SeriesItem {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    cover_book_id: r.get(2)?,
                    cover_v: r.get(3)?,
                    book_count: r.get::<_, i64>(4)? as i32,
                    read_count: r.get::<_, i64>(5)? as i32,
                    reading_count: r.get::<_, i64>(6)? as i32,
                    downloaded_count: r.get::<_, i64>(7)? as i32,
                    percent: r.get(8)?,
                    last_read_at: r.get(9)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn continue_reading(&self) -> Result<Vec<ContinueItem>, CoreError> {
        self.with_db(|c| {
            let mut st = c.prepare(
                "SELECT b.id, b.title, b.series_id, s.name, b.format, p.percent, b.cover_v
                 FROM progress p
                 JOIN books b ON b.id = p.book_id
                 JOIN series s ON s.id = b.series_id
                 WHERE p.status = 'reading' AND b.error IS NULL
                 ORDER BY p.updated_at DESC, b.id DESC
                 LIMIT 20",
            )?;
            let rows = st.query_map([], |r| {
                let format: String = r.get(4)?;
                Ok(ContinueItem {
                    book_id: r.get(0)?,
                    title: r.get(1)?,
                    series_id: r.get(2)?,
                    series_name: r.get(3)?,
                    is_comic: is_comic(&format),
                    percent: r.get(5)?,
                    cover_v: r.get(6)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn get_series(&self, series_id: i64) -> Result<SeriesDetail, CoreError> {
        self.with_db(|c| {
            let name: Option<String> = c
                .query_row("SELECT name FROM series WHERE id = ?1", [series_id], |r| {
                    r.get(0)
                })
                .optional()?;
            let Some(name) = name else {
                return Err(CoreError::NotFound);
            };
            let sql = format!("{} WHERE b.series_id = ?1 ORDER BY b.ord, b.id", BOOK_SELECT);
            let mut st = c.prepare(&sql)?;
            let books = st
                .query_map([series_id], book_item)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(SeriesDetail {
                id: series_id,
                name,
                books,
            })
        })
    }

    pub fn get_book(&self, book_id: i64) -> Result<BookDetail, CoreError> {
        self.with_db(|c| {
            let sql = format!("{} WHERE b.id = ?1", BOOK_SELECT);
            let book = c
                .query_row(&sql, [book_id], book_item)
                .optional()?
                .ok_or(CoreError::NotFound)?;
            let series_name: String = c
                .query_row(
                    "SELECT name FROM series WHERE id = ?1",
                    [book.series_id],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or_default();
            let locator: String = c
                .query_row(
                    "SELECT locator FROM progress WHERE book_id = ?1",
                    [book_id],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or_default();
            let ord: i64 = c.query_row("SELECT ord FROM books WHERE id = ?1", [book_id], |r| {
                r.get(0)
            })?;
            let next: Option<(i64, String)> = c
                .query_row(
                    "SELECT id, title FROM books
                     WHERE series_id = ?1 AND ord > ?2 AND error IS NULL
                     ORDER BY ord ASC LIMIT 1",
                    params![book.series_id, ord],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let prev: Option<i64> = c
                .query_row(
                    "SELECT id FROM books
                     WHERE series_id = ?1 AND ord < ?2 AND error IS NULL
                     ORDER BY ord DESC LIMIT 1",
                    params![book.series_id, ord],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(BookDetail {
                book,
                series_name,
                locator,
                next_book_id: next.as_ref().map(|n| n.0),
                next_book_title: next.map(|n| n.1),
                prev_book_id: prev,
            })
        })
    }
}

// ---- yenileme (sync_now adım 2) ----

fn series_sig(it: &SeriesListDto) -> String {
    format!(
        "{}|{}|{}|{:.4}|{}|{}|{}|{}",
        it.name,
        it.book_count,
        it.read_count,
        it.reading_count,
        it.percent,
        it.last_read_at.unwrap_or(0),
        it.cover_book_id.unwrap_or(0),
        it.cover_v.unwrap_or(0)
    )
}

fn book_ids_of_series(conn: &Connection, series_id: i64) -> rusqlite::Result<Vec<i64>> {
    let mut st = conn.prepare("SELECT id FROM books WHERE series_id = ?1")?;
    let rows = st.query_map([series_id], |r| r.get::<_, i64>(0))?;
    rows.collect()
}

/// Kitap satırları ve bağlı yerel satırlar (ilerleme, yer imi, içindekiler, parça, indirme).
pub(crate) fn purge_books(conn: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    for id in ids {
        conn.execute("DELETE FROM books WHERE id = ?1", [*id])?;
        for t in ["progress", "bookmarks", "toc", "parts", "downloads"] {
            conn.execute(&format!("DELETE FROM {t} WHERE book_id = ?1"), [*id])?;
        }
    }
    Ok(())
}

pub(crate) fn remove_book_dirs(root: &Path, ids: &[i64]) {
    for id in ids {
        let _ = std::fs::remove_dir_all(root.join("books").join(id.to_string()));
    }
}

/// Sunucu kütüphanesini yerele yansıtır. Sunucu ilerlemesi uygulanan kitap sayısını döner.
pub(crate) fn refresh(core: &Core) -> Result<i32, CoreError> {
    // 1. seri listesi
    let mut items: Vec<SeriesListDto> = Vec::new();
    loop {
        let path = format!("/api/series?limit=200&offset={}&sort=name", items.len());
        let page: SeriesPageDto = core.http.get_json(&path).map_err(into_core)?;
        let got = page.items.len();
        items.extend(page.items);
        if got == 0 || items.len() as i64 >= page.total {
            break;
        }
    }

    // 2-3. imzalar ve detayı çekilecek seriler
    let sigs: Vec<String> = items.iter().map(series_sig).collect();
    let local: HashMap<i64, (String, bool)> = core.with_db(|c| {
        let mut st = c.prepare(
            "SELECT s.id, s.sig, EXISTS(SELECT 1 FROM books b WHERE b.series_id = s.id)
             FROM series s",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                (r.get::<_, String>(1)?, r.get::<_, i64>(2)? != 0),
            ))
        })?;
        let mut m = HashMap::new();
        for row in rows {
            let (id, v) = row?;
            m.insert(id, v);
        }
        Ok(m)
    })?;
    let mut to_fetch: Vec<i64> = Vec::new();
    for (it, sig) in items.iter().zip(sigs.iter()) {
        let stale = match local.get(&it.id) {
            Some((old_sig, has_books)) => old_sig != sig || !*has_books,
            None => true,
        };
        if stale {
            to_fetch.push(it.id);
        }
    }
    let results = util::par_map(to_fetch, 4, |id| {
        (
            id,
            core.http
                .get_json::<SeriesDetailDto>(&format!("/api/series/{id}")),
        )
    });
    // None: sunucuda kitapsız (404); serinin yerel kitapları silinir
    let mut fetched: Vec<(i64, Option<SeriesDetailDto>)> = Vec::new();
    for (id, r) in results {
        match r {
            Ok(d) => fetched.push((id, Some(d))),
            Err(HErr::Status { code: 404, .. }) => fetched.push((id, None)),
            Err(e) => return Err(into_core(e)),
        }
    }

    // 4-5. tek işlem: seri/kitap upsert, silinenler, ilerleme adayları
    let server_ids: HashSet<i64> = items.iter().map(|i| i.id).collect();
    let (removed, candidates) = core.with_db(|c| {
        let tx = c.transaction()?;
        let mut removed: Vec<i64> = Vec::new();
        let mut candidates: Vec<(i64, String)> = Vec::new();

        let local_series: Vec<i64> = {
            let mut st = tx.prepare("SELECT id FROM series")?;
            let rows = st.query_map([], |r| r.get::<_, i64>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        for sid in local_series.iter().filter(|s| !server_ids.contains(*s)) {
            let ids = book_ids_of_series(&tx, *sid)?;
            purge_books(&tx, &ids)?;
            tx.execute("DELETE FROM series WHERE id = ?1", [*sid])?;
            removed.extend(ids);
        }

        {
            let mut up = tx.prepare(
                "INSERT INTO series(id, ord, name, folded_name, cover_book_id, cover_v)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                    ord = excluded.ord, name = excluded.name,
                    folded_name = excluded.folded_name,
                    cover_book_id = excluded.cover_book_id, cover_v = excluded.cover_v",
            )?;
            for (i, it) in items.iter().enumerate() {
                up.execute(params![
                    it.id,
                    i as i64,
                    it.name,
                    util::fold_tr(&it.name),
                    it.cover_book_id,
                    it.cover_v.unwrap_or(0)
                ])?;
            }
        }

        for (sid, detail) in &fetched {
            let existing = book_ids_of_series(&tx, *sid)?;
            let Some(detail) = detail else {
                purge_books(&tx, &existing)?;
                removed.extend(existing);
                continue;
            };
            let new_ids: HashSet<i64> = detail.books.iter().map(|b| b.id).collect();
            let gone: Vec<i64> = existing
                .into_iter()
                .filter(|id| !new_ids.contains(id))
                .collect();
            purge_books(&tx, &gone)?;
            removed.extend(gone);

            for (idx, b) in detail.books.iter().enumerate() {
                tx.execute(
                    "INSERT INTO books(id, series_id, ord, title, folded_title, format,
                                       page_count, size, error, cover_v)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT(id) DO UPDATE SET
                        series_id = excluded.series_id, ord = excluded.ord,
                        title = excluded.title, folded_title = excluded.folded_title,
                        format = excluded.format, page_count = excluded.page_count,
                        size = excluded.size, error = excluded.error,
                        cover_v = excluded.cover_v",
                    params![
                        b.id,
                        sid,
                        idx as i64,
                        b.title,
                        util::fold_tr(&b.title),
                        b.format,
                        b.page_count,
                        b.size,
                        b.error,
                        b.cover_v
                    ],
                )?;
                let lsig = format!("{}|{:.4}", b.status, b.percent);
                let seen: Option<String> = tx
                    .query_row(
                        "SELECT seen_sig FROM progress WHERE book_id = ?1",
                        [b.id],
                        |r| r.get(0),
                    )
                    .optional()?;
                match seen {
                    None if b.status == "unread" && b.percent == 0.0 => {
                        tx.execute(
                            "INSERT INTO progress(book_id, seen_sig) VALUES (?1, ?2)",
                            params![b.id, lsig],
                        )?;
                    }
                    None => candidates.push((b.id, lsig)),
                    Some(s) if s != lsig => candidates.push((b.id, lsig)),
                    Some(_) => {}
                }
            }
        }
        tx.commit()?;
        Ok((removed, candidates))
    })?;
    remove_book_dirs(&core.root, &removed);

    // 6. ilerleme çekme (db kilidi tutulmadan), sonra birleştirme
    let ids: Vec<i64> = candidates.iter().map(|(id, _)| *id).collect();
    let progress = util::par_map(ids, 4, |id| {
        core.http
            .get_json::<ProgressDto>(&format!("/api/books/{id}/progress"))
    });
    let mut dtos: Vec<(i64, String, ProgressDto)> = Vec::new();
    for ((id, lsig), r) in candidates.into_iter().zip(progress) {
        match r {
            Ok(d) => dtos.push((id, lsig, d)),
            Err(HErr::Status { code: 404, .. }) => {}
            Err(e) => return Err(into_core(e)),
        }
    }
    let pulled = core.with_db(|c| {
        let tx = c.transaction()?;
        let mut pulled = 0;
        for (id, lsig, dto) in &dtos {
            if sync::apply_progress(&tx, *id, dto)? {
                pulled += 1;
            }
            tx.execute(
                "INSERT INTO progress(book_id, seen_sig) VALUES (?1, ?2)
                 ON CONFLICT(book_id) DO UPDATE SET seen_sig = excluded.seen_sig",
                params![id, lsig],
            )?;
        }
        // imza en sona yazılır: yarıda kalan yenileme bir sonrakinde detayı yeniden çeker
        for (sid, _) in &fetched {
            if let Some(pos) = items.iter().position(|i| i.id == *sid) {
                tx.execute(
                    "UPDATE series SET sig = ?1 WHERE id = ?2",
                    params![sigs[pos], sid],
                )?;
            }
        }
        db::kv_set(&tx, "sync_last_ok_at", &core.now().to_string());
        tx.commit()?;
        Ok(pulled)
    })?;
    Ok(pulled)
}
