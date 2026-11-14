use std::{
    collections::HashSet,
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    error::into_core,
    http::{HErr, PagesDto},
    library::{self, is_comic},
    reader::{self, PErr},
    util, Core, CoreError, DownloadInfo, DownloadRun, DownloadState, StorageUsage,
};

/// `parts` sayaçlarını indirme satırına yansıtır (satır yoksa etkisiz).
pub(crate) fn refresh_counts(conn: &Connection, book_id: i64, now: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE downloads SET
            parts_done = (SELECT COUNT(*) FROM parts WHERE book_id = ?1),
            bytes = (SELECT COALESCE(SUM(bytes), 0) FROM parts WHERE book_id = ?1),
            updated_at = ?2
         WHERE book_id = ?1",
        params![book_id, now],
    )
}

/// `run_downloads` her çıkışta (hata/panik dahil) `dl_running`'i düşürür.
struct RunGuard<'a>(&'a AtomicBool);

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// `download_book` hangi yoldan dönerse dönsün (erken `?` dahil) kendi iptal işaretini temizler.
struct CancelGuard<'a>(&'a Core, i64);

impl Drop for CancelGuard<'_> {
    fn drop(&mut self) {
        self.0
            .cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.1);
    }
}

enum Outcome {
    Done,
    Cancelled,
    Stopped,
}

fn is_cancelled(core: &Core, book_id: i64) -> bool {
    core.cancel
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains(&book_id)
}

/// Yalnız ağ hatası ve 5xx yeniden denenir; 4xx kalıcıdır.
fn retriable(e: &PErr) -> bool {
    match e {
        PErr::Http(HErr::Net(_)) => true,
        PErr::Http(HErr::Status { code, .. }) => *code >= 500,
        _ => false,
    }
}

/// En çok 3 deneme; bekleme 1 sn, 2 sn.
fn with_retry<T>(mut f: impl FnMut() -> Result<T, PErr>) -> Result<T, PErr> {
    let mut attempt: u64 = 0;
    loop {
        match f() {
            Ok(v) => return Ok(v),
            Err(e) => {
                attempt += 1;
                if attempt >= 3 || !retriable(&e) {
                    return Err(e);
                }
                thread::sleep(Duration::from_secs(attempt));
            }
        }
    }
}

/// Satır hâlâ `running` ise yeni duruma geçirir (silinmiş/yeniden kuyruğa alınmışsa dokunmaz).
fn finish_running(
    core: &Core,
    book_id: i64,
    state: &str,
    error: Option<&str>,
) -> Result<(), CoreError> {
    let now = core.now();
    core.with_db(|c| {
        c.execute(
            "UPDATE downloads SET state = ?2, error = ?3, updated_at = ?4
             WHERE book_id = ?1 AND state = 'running'",
            params![book_id, state, error, now],
        )?;
        Ok(())
    })
}

fn download_book(core: &Core, id: i64) -> Result<Outcome, CoreError> {
    let _cancel = CancelGuard(core, id);
    let info: Option<(String, i64, i64)> = core.with_db(|c| {
        Ok(c.query_row(
            "SELECT format, page_count, cover_v FROM books WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
    })?;
    let Some((format, page_count, cover_v)) = info else {
        // kitap yerelde yok (sunucuda silinmiş): indirme satırı da gider
        core.with_db(|c| {
            c.execute("DELETE FROM downloads WHERE book_id = ?1", [id])?;
            Ok(())
        })?;
        return Ok(Outcome::Cancelled);
    };
    let comic = is_comic(&format);

    // hazırlık: parça sayısı, içindekiler, kapak
    let total: i64 = if comic {
        if page_count > 0 {
            page_count
        } else {
            let dto: PagesDto = core
                .http
                .get_json(&format!("/api/books/{id}/pages"))
                .map_err(into_core)?;
            dto.count
        }
    } else {
        let toc = reader::ensure_toc(core, id)?;
        if page_count > 0 {
            page_count
        } else {
            toc.len() as i64
        }
    };
    if total <= 0 {
        return Err(CoreError::Server("İndirilecek içerik yok.".to_string()));
    }
    if !core.root.join("covers").join(format!("{id}-{cover_v}.jpg")).exists() {
        let _ = reader::fetch_cover(core, id, cover_v); // hata yutulur
    }
    let now = core.now();
    core.with_db(|c| {
        c.execute(
            "UPDATE downloads SET parts_total = ?2 WHERE book_id = ?1",
            params![id, total],
        )?;
        refresh_counts(c, id, now)?;
        Ok(())
    })?;

    // eksik parçalar
    let have: HashSet<i64> = core.with_db(|c| {
        let mut st = c.prepare("SELECT idx FROM parts WHERE book_id = ?1")?;
        let rows = st.query_map([id], |r| r.get::<_, i64>(0))?;
        Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
    })?;
    let missing: Vec<i64> = (0..total).filter(|i| !have.contains(i)).collect();

    let results = util::par_map(missing, 3, |idx| {
        // her parçadan önce durdurma/iptal denetlenir
        if core.stop.load(Ordering::SeqCst) || is_cancelled(core, id) {
            return (idx, Ok(false));
        }
        let r = with_retry(|| {
            if comic {
                reader::fetch_page(core, id, idx)
            } else {
                reader::fetch_chapter(core, id, idx as i32)
            }
        });
        (idx, r.map(|_| true).map_err(PErr::into_core))
    });

    // çalışırken download_remove çağrıldıysa satır zaten silindi
    if core
        .cancel
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id)
    {
        return Ok(Outcome::Cancelled);
    }
    let mut skipped = false;
    let mut failure: Option<CoreError> = None;
    for (idx, r) in results {
        match r {
            Ok(true) => {}
            Ok(false) => skipped = true,
            Err(CoreError::Unauthorized) => return Err(CoreError::Unauthorized),
            Err(e) => {
                if failure.is_none() {
                    let what = if comic { "Sayfa" } else { "Bölüm" };
                    failure = Some(CoreError::Server(format!(
                        "{what} {} indirilemedi: {e}",
                        idx + 1
                    )));
                }
            }
        }
    }
    if let Some(e) = failure {
        return Err(e);
    }
    if skipped {
        return Ok(Outcome::Stopped);
    }
    Ok(Outcome::Done)
}

#[uniffi::export]
impl Core {
    pub fn download_enqueue(&self, book_ids: Vec<i64>) -> Result<(), CoreError> {
        let now = self.now();
        self.with_db(|c| {
            let tx = c.transaction()?;
            for id in book_ids {
                let err: Option<Option<String>> = tx
                    .query_row("SELECT error FROM books WHERE id = ?1", [id], |r| r.get(0))
                    .optional()?;
                // yerelde olmayan ya da hatalı (açılamayan) kitap atlanır
                if !matches!(err, Some(None)) {
                    continue;
                }
                let state: Option<String> = tx
                    .query_row(
                        "SELECT state FROM downloads WHERE book_id = ?1",
                        [id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if state.as_deref() == Some("done") {
                    continue;
                }
                tx.execute(
                    "INSERT INTO downloads(book_id, state, parts_total, parts_done, bytes, error,
                                           updated_at)
                     VALUES (?1, 'queued', 0, 0, 0, NULL, ?2)
                     ON CONFLICT(book_id) DO UPDATE SET
                        state = 'queued', error = NULL, updated_at = excluded.updated_at",
                    params![id, now],
                )?;
                // eski bir iptal işareti yeni indirmeyi durdurmasın
                self.cancel
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .remove(&id);
            }
            tx.commit()?;
            Ok(())
        })
    }

    /// Çalışan işi iptal eder; dosya + satırları siler (önbellek dosyaları dahil).
    pub fn download_remove(&self, book_ids: Vec<i64>) -> Result<(), CoreError> {
        self.with_db(|c| {
            let tx = c.transaction()?;
            for id in &book_ids {
                let state: Option<String> = tx
                    .query_row(
                        "SELECT state FROM downloads WHERE book_id = ?1",
                        [*id],
                        |r| r.get(0),
                    )
                    .optional()?;
                if state.as_deref() == Some("running") {
                    self.cancel
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(*id);
                }
                for t in ["downloads", "parts", "toc"] {
                    tx.execute(&format!("DELETE FROM {t} WHERE book_id = ?1"), [*id])?;
                }
            }
            tx.commit()?;
            Ok(())
        })?;
        library::remove_book_dirs(&self.root, &book_ids);
        Ok(())
    }

    /// Yalnız satırı olan kitaplar.
    pub fn download_infos(&self) -> Result<Vec<DownloadInfo>, CoreError> {
        self.with_db(|c| {
            let mut st = c.prepare(
                "SELECT book_id, state, parts_done, parts_total, bytes, error
                 FROM downloads ORDER BY book_id",
            )?;
            let rows = st.query_map([], |r| {
                let state: String = r.get(1)?;
                Ok(DownloadInfo {
                    book_id: r.get(0)?,
                    state: DownloadState::from_db(Some(state.as_str())),
                    parts_done: r.get::<_, i64>(2)? as i32,
                    parts_total: r.get::<_, i64>(3)? as i32,
                    bytes: r.get(4)?,
                    error: r.get(5)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// BLOKLAR; kuyruk boşalana dek.
    pub fn run_downloads(&self) -> Result<DownloadRun, CoreError> {
        if self.dl_running.swap(true, Ordering::SeqCst) {
            return Ok(DownloadRun {
                finished: 0,
                failed: 0,
            });
        }
        let _guard = RunGuard(&self.dl_running);
        self.stop.store(false, Ordering::SeqCst);
        let r = self.run_loop();
        // hangi yoldan çıkılırsa çıkılsın `running`de asılı satır kalmaz
        let _ = self.with_db(|c| {
            c.execute(
                "UPDATE downloads SET state = 'queued' WHERE state = 'running'",
                [],
            )?;
            Ok(())
        });
        r
    }

    /// Çalışan `run_downloads`'ı nazikçe durdurur (sıradaki parçadan önce).
    pub fn download_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn storage_usage(&self) -> Result<StorageUsage, CoreError> {
        let (downloads_bytes, cache_bytes) = self.with_db(|c| {
            let dl: i64 = c.query_row(
                "SELECT COALESCE(SUM(bytes), 0) FROM parts
                 WHERE book_id IN (SELECT book_id FROM downloads)",
                [],
                |r| r.get(0),
            )?;
            let cache: i64 = c.query_row(
                "SELECT COALESCE(SUM(bytes), 0) FROM parts
                 WHERE book_id NOT IN (SELECT book_id FROM downloads)",
                [],
                |r| r.get(0),
            )?;
            Ok((dl, cache))
        })?;
        Ok(StorageUsage {
            downloads_bytes,
            cache_bytes,
            covers_bytes: dir_size(&self.root.join("covers")),
            db_bytes: fs::metadata(self.root.join("readcrab.db"))
                .map(|m| m.len() as i64)
                .unwrap_or(0),
        })
    }

    /// İndirme satırı olmayan kitapların dosyalarını/parça satırlarını siler; serbest kalan baytı döner.
    pub fn clear_cache(&self) -> Result<i64, CoreError> {
        let (ids, bytes) = self.with_db(|c| {
            let tx = c.transaction()?;
            let ids: Vec<i64> = {
                let mut st = tx.prepare(
                    "SELECT DISTINCT book_id FROM parts
                     WHERE book_id NOT IN (SELECT book_id FROM downloads)",
                )?;
                let rows = st.query_map([], |r| r.get::<_, i64>(0))?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            let bytes: i64 = tx.query_row(
                "SELECT COALESCE(SUM(bytes), 0) FROM parts
                 WHERE book_id NOT IN (SELECT book_id FROM downloads)",
                [],
                |r| r.get(0),
            )?;
            for t in ["parts", "toc"] {
                tx.execute(
                    &format!(
                        "DELETE FROM {t} WHERE book_id NOT IN (SELECT book_id FROM downloads)"
                    ),
                    [],
                )?;
            }
            tx.commit()?;
            Ok((ids, bytes))
        })?;
        library::remove_book_dirs(&self.root, &ids);
        Ok(bytes)
    }
}

impl Core {
    fn run_loop(&self) -> Result<DownloadRun, CoreError> {
        let mut finished = 0;
        let mut failed = 0;
        loop {
            if self.stop.load(Ordering::SeqCst) {
                break;
            }
            let next: Option<i64> = self.with_db(|c| {
                Ok(c.query_row(
                    "SELECT book_id FROM downloads WHERE state = 'queued'
                     ORDER BY updated_at ASC, book_id ASC LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .optional()?)
            })?;
            let Some(id) = next else { break };
            let now = self.now();
            self.with_db(|c| {
                c.execute(
                    "UPDATE downloads SET state = 'running', error = NULL, updated_at = ?2
                     WHERE book_id = ?1 AND state = 'queued'",
                    params![id, now],
                )?;
                Ok(())
            })?;
            match download_book(self, id) {
                Ok(Outcome::Done) => {
                    finish_running(self, id, "done", None)?;
                    finished += 1;
                }
                // satır silindiyse etkisiz; hâlâ `running` ise kuyruğa döner (takılı kalmaz)
                Ok(Outcome::Cancelled) => finish_running(self, id, "queued", None)?,
                Ok(Outcome::Stopped) => {
                    finish_running(self, id, "queued", None)?;
                    break;
                }
                Err(CoreError::Unauthorized) => {
                    finish_running(self, id, "queued", None)?;
                    return Err(CoreError::Unauthorized);
                }
                Err(e) => {
                    finish_running(self, id, "failed", Some(&e.to_string()))?;
                    failed += 1;
                }
            }
        }
        Ok(DownloadRun { finished, failed })
    }
}

fn dir_size(p: &Path) -> i64 {
    fs::read_dir(p)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.metadata().ok())
                .filter(|m| m.is_file())
                .map(|m| m.len() as i64)
                .sum::<i64>()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_core() -> (tempfile::TempDir, std::sync::Arc<Core>) {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::new(dir.path().to_string_lossy().to_string()).unwrap();
        (dir, core)
    }

    fn add_book(core: &Core, id: i64, error: Option<&str>) {
        core.with_db(|c| {
            c.execute(
                "INSERT INTO books(id, series_id, ord, title, folded_title, format,
                                   page_count, size, error)
                 VALUES (?1, 1, ?1, 'K', 'k', 'cbz', 2, 10, ?2)",
                params![id, error],
            )?;
            Ok(())
        })
        .unwrap();
    }

    fn add_part(core: &Core, id: i64, idx: i64, bytes: i64) {
        let p = core.root.join("books").join(id.to_string());
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join(format!("p{idx}")), vec![0u8; bytes as usize]).unwrap();
        core.with_db(|c| {
            c.execute(
                "INSERT INTO parts(book_id, idx, bytes) VALUES (?1, ?2, ?3)",
                params![id, idx, bytes],
            )?;
            Ok(())
        })
        .unwrap();
    }

    fn state_of(core: &Core, id: i64) -> DownloadState {
        core.download_infos()
            .unwrap()
            .into_iter()
            .find(|i| i.book_id == id)
            .map(|i| i.state)
            .unwrap_or(DownloadState::None)
    }

    #[test]
    fn enqueue_skips_error_done_and_unknown_books() {
        let (_d, core) = new_core();
        add_book(&core, 1, None);
        add_book(&core, 2, Some("bozuk"));
        add_book(&core, 3, None);
        core.with_db(|c| {
            c.execute(
                "INSERT INTO downloads(book_id, state, updated_at) VALUES (3, 'done', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        core.download_enqueue(vec![1, 2, 3, 99]).unwrap();
        assert_eq!(state_of(&core, 1), DownloadState::Queued);
        assert_eq!(state_of(&core, 2), DownloadState::None);
        assert_eq!(state_of(&core, 3), DownloadState::Done);
        assert_eq!(state_of(&core, 99), DownloadState::None);
    }

    #[test]
    fn failed_requeues_and_keeps_parts() {
        let (_d, core) = new_core();
        add_book(&core, 1, None);
        add_part(&core, 1, 0, 5);
        core.with_db(|c| {
            c.execute(
                "INSERT INTO downloads(book_id, state, parts_done, bytes, error, updated_at)
                 VALUES (1, 'failed', 1, 5, 'x', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        core.download_enqueue(vec![1]).unwrap();
        let info = core.download_infos().unwrap().remove(0);
        assert_eq!(info.state, DownloadState::Queued);
        assert_eq!(info.parts_done, 1);
        assert_eq!(info.error, None);
    }

    #[test]
    fn new_resets_running_to_queued() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_string_lossy().to_string();
        let core = Core::new(path.clone()).unwrap();
        core.with_db(|c| {
            c.execute(
                "INSERT INTO downloads(book_id, state, updated_at) VALUES (1, 'running', 1)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        drop(core);
        let core = Core::new(path).unwrap();
        assert_eq!(state_of(&core, 1), DownloadState::Queued);
    }

    #[test]
    fn remove_deletes_rows_and_files() {
        let (_d, core) = new_core();
        add_book(&core, 1, None);
        add_part(&core, 1, 0, 4);
        core.download_enqueue(vec![1]).unwrap();
        core.download_remove(vec![1]).unwrap();
        assert_eq!(state_of(&core, 1), DownloadState::None);
        assert!(!core.root.join("books").join("1").exists());
        let n: i64 = core
            .with_db(|c| Ok(c.query_row("SELECT COUNT(*) FROM parts", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn clear_cache_only_touches_books_without_download_row() {
        let (_d, core) = new_core();
        add_book(&core, 1, None); // indirilmiş
        add_book(&core, 2, None); // yalnız önbellek
        add_part(&core, 1, 0, 10);
        add_part(&core, 2, 0, 7);
        add_part(&core, 2, 1, 3);
        core.download_enqueue(vec![1]).unwrap();
        let u = core.storage_usage().unwrap();
        assert_eq!((u.downloads_bytes, u.cache_bytes), (10, 10));
        assert_eq!(core.clear_cache().unwrap(), 10);
        assert!(core.root.join("books").join("1").exists());
        assert!(!core.root.join("books").join("2").exists());
        let u = core.storage_usage().unwrap();
        assert_eq!((u.downloads_bytes, u.cache_bytes), (10, 0));
    }

    fn has_cancel(core: &Core, id: i64) -> bool {
        core.cancel.lock().unwrap().contains(&id)
    }

    #[test]
    fn cancel_flag_is_cleared_on_early_exit_and_on_enqueue() {
        let (_d, core) = new_core();
        // sayfa sayısı 0 ve oturum yok: `download_book` erken `?` ile döner
        core.with_db(|c| {
            c.execute(
                "INSERT INTO books(id, series_id, ord, title, folded_title, format, page_count, size)
                 VALUES (1, 1, 0, 'K', 'k', 'cbz', 0, 10)",
                [],
            )?;
            Ok(())
        })
        .unwrap();
        core.cancel.lock().unwrap().insert(1);
        assert!(matches!(download_book(&core, 1), Err(CoreError::Unauthorized)));
        assert!(!has_cancel(&core, 1));

        // yeniden kuyruğa alma da işareti temizler
        core.cancel.lock().unwrap().insert(1);
        core.download_enqueue(vec![1]).unwrap();
        assert!(!has_cancel(&core, 1));

        // satır `running`de asılı kalmaz; sonraki çalıştırma da engellenmez
        assert!(matches!(core.run_downloads(), Err(CoreError::Unauthorized)));
        assert_eq!(state_of(&core, 1), DownloadState::Queued);
        assert!(matches!(core.run_downloads(), Err(CoreError::Unauthorized)));
    }

    #[test]
    fn stale_cancel_does_not_leave_row_running() {
        let (_d, core) = new_core();
        add_book(&core, 1, None); // 2 sayfa, ikisi de yerelde: ağ gerekmez
        add_part(&core, 1, 0, 3);
        add_part(&core, 1, 1, 3);
        core.download_enqueue(vec![1]).unwrap();
        // enqueue sonrası takılmış bir iptal işareti: ilk tur iptal sayılır, satır kuyruğa döner
        core.cancel.lock().unwrap().insert(1);
        let run = core.run_downloads().unwrap();
        assert_eq!((run.finished, run.failed), (1, 0));
        assert_eq!(state_of(&core, 1), DownloadState::Done);
        assert!(core.cancel.lock().unwrap().is_empty());
    }

    #[test]
    fn retry_policy() {
        let mut calls = 0;
        let r: Result<(), PErr> = with_retry(|| {
            calls += 1;
            Err(PErr::Http(HErr::Status {
                code: 404,
                error: String::new(),
                message: String::new(),
                retry_after: None,
            }))
        });
        assert!(r.is_err());
        assert_eq!(calls, 1, "4xx yeniden denenmez");
    }
}
