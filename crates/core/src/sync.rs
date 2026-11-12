use std::{sync::Mutex, time::Duration};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

use crate::{
    db,
    error::into_core,
    http::{parse_json, BookmarkDto, BookmarksDto, HErr, ProgressDto, PutResDto},
    library, util, BookmarkItem, Core, CoreError, PositionSuggestion, ProgressInfo, ReadStatus,
    SyncReport, SyncStatus,
};

// Aynı anda iki itme (okuyucu çıkışı + elle eşitleme) yer imini iki kez göndermesin.
static PUSH_LOCK: Mutex<()> = Mutex::new(());

const MAX_BOOKMARKS_PER_BOOK: i64 = 100;

// ---- saf birleştirme: ilerleme ----

#[derive(Clone, Debug, PartialEq)]
pub struct LocalProg {
    pub updated_at: i64,
    pub pending: bool, // p_dirty || s_dirty
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerProg {
    pub updated_at: i64,
    pub has_row: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Merge {
    TakeServer,
    KeepLocal,
}

/// En büyük `updated_at` kazanır; eşitlikte yerel kazanır.
pub fn merge_progress(local: Option<&LocalProg>, srv: &ServerProg) -> Merge {
    if !srv.has_row {
        return Merge::KeepLocal;
    }
    let Some(l) = local else {
        return Merge::TakeServer;
    };
    if l.pending && srv.updated_at <= l.updated_at {
        Merge::KeepLocal
    } else {
        Merge::TakeServer
    }
}

/// Sunucuyla aynı yerel durum türetmesi.
pub fn derive_status(prev: Option<&str>, percent: f64) -> &'static str {
    if percent >= 0.9999 || prev == Some("finished") {
        "finished"
    } else {
        "reading"
    }
}

// ---- saf birleştirme: yer imleri ----

#[derive(Clone, Debug, PartialEq)]
pub struct LocalBm {
    pub local_id: i64,
    pub server_id: Option<i64>,
    pub locator: String,
    pub label: String,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SrvBm {
    pub id: i64,
    pub locator: String,
    pub label: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BmOp {
    Insert(SrvBm),
    Adopt {
        local_id: i64,
        server_id: i64,
    },
    Update {
        local_id: i64,
        locator: String,
        label: String,
        created_at: i64,
    },
    Remove {
        local_id: i64,
    },
}

pub fn merge_bookmarks(local: &[LocalBm], server: &[SrvBm]) -> Vec<BmOp> {
    let mut ops = Vec::new();
    let mut adopted: Vec<i64> = Vec::new(); // eşlenen yerel local_id'ler
    for s in server {
        if let Some(l) = local.iter().find(|l| l.server_id == Some(s.id)) {
            // yerelde silinmişse DELETE bekliyor: dokunma
            if !l.deleted {
                ops.push(BmOp::Update {
                    local_id: l.local_id,
                    locator: s.locator.clone(),
                    label: s.label.clone(),
                    created_at: s.created_at,
                });
            }
        } else if let Some(l) = local.iter().find(|l| {
            l.server_id.is_none()
                && !l.deleted
                && l.locator == s.locator
                && l.label == s.label
                && !adopted.contains(&l.local_id)
        }) {
            adopted.push(l.local_id);
            ops.push(BmOp::Adopt {
                local_id: l.local_id,
                server_id: s.id,
            });
        } else {
            ops.push(BmOp::Insert(s.clone()));
        }
    }
    for l in local {
        if let Some(sid) = l.server_id {
            if !server.iter().any(|s| s.id == sid) {
                ops.push(BmOp::Remove {
                    local_id: l.local_id,
                });
            }
        }
    }
    ops
}

fn clamp01(v: f64) -> f64 {
    if v.is_finite() {
        v.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn record_error(core: &Core, e: &CoreError) {
    let _ = core.with_db(|c| {
        db::kv_set(c, "sync_last_error", &e.to_string());
        Ok(())
    });
}

// ---- çekme: ilerleme ----

/// Yerelde bir konum var; sunucununki farklı ve daha yeni: sessizce ezmek yerine kullanıcıya sorulur.
pub fn suggest_instead(local_locator: &str, local_updated_at: i64, dto: &ProgressDto) -> bool {
    !local_locator.is_empty()
        && !dto.locator.is_empty()
        && local_locator != dto.locator
        && dto.updated_at > local_updated_at
}

/// Sunucu ilerlemesini birleştirir; sunucu verisi yerele UYGULANDIYSA `true`.
/// Yerel konum varken farklı ve daha yeni sunucu konumu uygulanmaz, öneri olarak saklanır.
pub(crate) fn apply_progress(
    c: &Connection,
    book_id: i64,
    dto: &ProgressDto,
) -> Result<bool, CoreError> {
    let local: Option<(String, i64, bool, i64)> = c
        .query_row(
            "SELECT locator, updated_at, p_dirty <> 0 OR s_dirty <> 0, declined_ts
             FROM progress WHERE book_id = ?1",
            [book_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0, r.get(3)?)),
        )
        .optional()?;
    let local_prog = local.as_ref().map(|(_, upd, pending, _)| LocalProg {
        updated_at: *upd,
        pending: *pending,
    });
    let srv = ServerProg {
        updated_at: dto.updated_at,
        has_row: !(dto.updated_at == 0 && dto.status == "unread" && dto.locator.is_empty()),
    };
    match merge_progress(local_prog.as_ref(), &srv) {
        Merge::KeepLocal => Ok(false),
        Merge::TakeServer => {
            if let Some((loc, upd, _, declined)) = &local {
                if suggest_instead(loc, *upd, dto) {
                    if *declined != dto.updated_at {
                        c.execute(
                            "UPDATE progress SET remote_locator = ?2, remote_percent = ?3,
                                    remote_status = ?4, remote_updated_at = ?5
                             WHERE book_id = ?1",
                            params![
                                book_id,
                                dto.locator,
                                clamp01(dto.percent),
                                ReadStatus::from_db(&dto.status).as_str(),
                                dto.updated_at
                            ],
                        )?;
                    }
                    return Ok(false);
                }
            }
            c.execute(
                "INSERT INTO progress(book_id, locator, percent, status, updated_at,
                                      p_dirty, s_dirty, last_op)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, '')
                 ON CONFLICT(book_id) DO UPDATE SET
                    locator = excluded.locator,
                    percent = excluded.percent,
                    status = excluded.status,
                    updated_at = excluded.updated_at,
                    p_dirty = 0, s_dirty = 0, last_op = '',
                    remote_locator = '', remote_percent = 0, remote_status = '',
                    remote_updated_at = 0, declined_ts = 0",
                params![
                    book_id,
                    dto.locator,
                    clamp01(dto.percent),
                    ReadStatus::from_db(&dto.status).as_str(),
                    dto.updated_at
                ],
            )?;
            Ok(true)
        }
    }
}

pub(crate) fn pull_progress(
    core: &Core,
    book_id: i64,
    total: Option<Duration>,
) -> Result<bool, CoreError> {
    let path = format!("/api/books/{book_id}/progress");
    let dto: ProgressDto = match total {
        Some(t) => core.http.get_json_timeout(&path, t),
        None => core.http.get_json(&path),
    }
    .map_err(into_core)?;
    core.with_db(|c| apply_progress(c, book_id, &dto))
}

// ---- çekme: yer imleri ----

fn apply_bookmarks(
    c: &mut Connection,
    book_id: i64,
    items: &[BookmarkDto],
) -> Result<(), CoreError> {
    let tx = c.transaction()?;
    let local: Vec<LocalBm> = {
        let mut st = tx.prepare(
            "SELECT local_id, server_id, locator, label, deleted FROM bookmarks
             WHERE book_id = ?1 ORDER BY local_id",
        )?;
        let rows = st.query_map([book_id], |r| {
            Ok(LocalBm {
                local_id: r.get(0)?,
                server_id: r.get(1)?,
                locator: r.get(2)?,
                label: r.get(3)?,
                deleted: r.get::<_, i64>(4)? != 0,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };
    let server: Vec<SrvBm> = items
        .iter()
        .map(|b| SrvBm {
            id: b.id,
            locator: b.locator.clone(),
            label: b.label.clone(),
            created_at: b.created_at,
        })
        .collect();
    for op in merge_bookmarks(&local, &server) {
        match op {
            BmOp::Insert(s) => {
                tx.execute(
                    "INSERT INTO bookmarks(server_id, book_id, locator, label, created_at, deleted)
                     VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                    params![s.id, book_id, s.locator, s.label, s.created_at],
                )?;
            }
            BmOp::Adopt {
                local_id,
                server_id,
            } => {
                tx.execute(
                    "UPDATE bookmarks SET server_id = ?1 WHERE local_id = ?2",
                    params![server_id, local_id],
                )?;
            }
            BmOp::Update {
                local_id,
                locator,
                label,
                created_at,
            } => {
                tx.execute(
                    "UPDATE bookmarks SET locator = ?1, label = ?2, created_at = ?3
                     WHERE local_id = ?4",
                    params![locator, label, created_at, local_id],
                )?;
            }
            BmOp::Remove { local_id } => {
                tx.execute("DELETE FROM bookmarks WHERE local_id = ?1", [local_id])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn pull_bookmarks(
    core: &Core,
    book_id: i64,
    total: Option<Duration>,
) -> Result<(), CoreError> {
    let path = format!("/api/books/{book_id}/bookmarks");
    let dto: BookmarksDto = match total {
        Some(t) => core.http.get_json_timeout(&path, t),
        None => core.http.get_json(&path),
    }
    .map_err(into_core)?;
    core.with_db(|c| apply_bookmarks(c, book_id, &dto.items))
}

/// `sync_now` adım 3: indirilen / okunan / yer imi olan kitapların yer imleri.
fn pull_all_bookmarks(core: &Core) -> Result<(), CoreError> {
    let ids: Vec<i64> = core.with_db(|c| {
        let mut st = c.prepare(
            "SELECT id FROM books WHERE id IN (
                SELECT book_id FROM downloads WHERE state = 'done'
                UNION SELECT book_id FROM progress WHERE status = 'reading'
                UNION SELECT book_id FROM bookmarks
             )",
        )?;
        let rows = st.query_map([], |r| r.get::<_, i64>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;
    for r in util::par_map(ids, 4, |id| pull_bookmarks(core, id, None)) {
        match r {
            Ok(()) | Err(CoreError::NotFound) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

// ---- itme ----

struct ProgRow {
    book_id: i64,
    locator: String,
    percent: f64,
    status: String,
    updated_at: i64,
    local_date: String,
    p_dirty: bool,
    s_dirty: bool,
    last_op: String,
}

fn clear_flags(core: &Core, book_id: i64, p: bool, s: bool, updated_at: i64) -> Result<(), CoreError> {
    core.with_db(|c| {
        if p {
            c.execute(
                "UPDATE progress SET p_dirty = 0 WHERE book_id = ?1 AND updated_at = ?2",
                params![book_id, updated_at],
            )?;
        }
        if s {
            c.execute(
                "UPDATE progress SET s_dirty = 0 WHERE book_id = ?1 AND updated_at = ?2",
                params![book_id, updated_at],
            )?;
        }
        Ok(())
    })
}

fn push_progress(core: &Core, count: &mut i32, pull: &mut Vec<i64>) -> Result<(), CoreError> {
    let rows: Vec<ProgRow> = core.with_db(|c| {
        let mut st = c.prepare(
            "SELECT book_id, locator, percent, status, updated_at, local_date,
                    p_dirty, s_dirty, last_op
             FROM progress WHERE p_dirty = 1 OR s_dirty = 1 ORDER BY updated_at",
        )?;
        let rows = st.query_map([], |r| {
            Ok(ProgRow {
                book_id: r.get(0)?,
                locator: r.get(1)?,
                percent: r.get(2)?,
                status: r.get(3)?,
                updated_at: r.get(4)?,
                local_date: r.get(5)?,
                p_dirty: r.get::<_, i64>(6)? != 0,
                s_dirty: r.get::<_, i64>(7)? != 0,
                last_op: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;

    for row in rows {
        // iki bayrak da kirliyse son yerel işlem SONA kalır
        let ops: Vec<char> = match (row.p_dirty, row.s_dirty) {
            (true, true) if row.last_op == "p" => vec!['s', 'p'],
            (true, true) => vec!['p', 's'],
            (true, false) => vec!['p'],
            _ => vec!['s'],
        };
        for op in ops {
            let (path, body) = if op == 'p' {
                (
                    format!("/api/books/{}/progress", row.book_id),
                    json!({
                        "locator": row.locator,
                        "percent": row.percent,
                        "local_date": row.local_date,
                        "client_ts": row.updated_at,
                    }),
                )
            } else {
                (
                    format!("/api/books/{}/status", row.book_id),
                    json!({
                        "status": row.status,
                        "local_date": row.local_date,
                        "client_ts": row.updated_at,
                    }),
                )
            };
            match core.http.send_json("PUT", &path, &body) {
                Ok(r) => {
                    *count += 1;
                    let applied = parse_json::<PutResDto>(&r.body)
                        .map(|d| d.applied)
                        .unwrap_or(true);
                    if applied {
                        clear_flags(core, row.book_id, op == 'p', op == 's', row.updated_at)?;
                    } else {
                        // sunucu daha yeni: bayraklar düşer, sonraki yenilemede kitap yeniden çekilir
                        core.with_db(|c| {
                            c.execute(
                                "UPDATE progress SET p_dirty = 0, s_dirty = 0, seen_sig = ''
                                 WHERE book_id = ?1 AND updated_at = ?2",
                                params![row.book_id, row.updated_at],
                            )?;
                            c.execute(
                                "UPDATE series SET sig = ''
                                 WHERE id = (SELECT series_id FROM books WHERE id = ?1)",
                                [row.book_id],
                            )?;
                            Ok(())
                        })?;
                        pull.push(row.book_id);
                        break;
                    }
                }
                Err(HErr::Status {
                    code: 400 | 404, ..
                }) => {
                    // kalıcı hata: işlem düşer
                    clear_flags(core, row.book_id, true, true, row.updated_at)?;
                    break;
                }
                Err(e) => return Err(into_core(e)),
            }
        }
    }
    Ok(())
}

fn push_bookmarks(core: &Core, count: &mut i32) -> Result<(), CoreError> {
    let deletes: Vec<(i64, i64)> = core.with_db(|c| {
        let mut st = c.prepare(
            "SELECT local_id, server_id FROM bookmarks
             WHERE deleted = 1 AND server_id IS NOT NULL ORDER BY local_id",
        )?;
        let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;
    for (local_id, server_id) in deletes {
        match core
            .http
            .send_empty("DELETE", &format!("/api/bookmarks/{server_id}"))
        {
            Ok(_) | Err(HErr::Status { code: 404, .. }) => {
                core.with_db(|c| {
                    c.execute("DELETE FROM bookmarks WHERE local_id = ?1", [local_id])?;
                    Ok(())
                })?;
                *count += 1;
            }
            Err(e) => return Err(into_core(e)),
        }
    }

    let adds: Vec<(i64, i64, String, String)> = core.with_db(|c| {
        let mut st = c.prepare(
            "SELECT local_id, book_id, locator, label FROM bookmarks
             WHERE server_id IS NULL AND deleted = 0 ORDER BY created_at, local_id",
        )?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;
    for (local_id, book_id, locator, label) in adds {
        let body = json!({ "locator": locator, "label": label });
        match core
            .http
            .send_json("POST", &format!("/api/books/{book_id}/bookmarks"), &body)
        {
            Ok(r) => {
                let dto: BookmarkDto = parse_json(&r.body).map_err(into_core)?;
                core.with_db(|c| {
                    // sunucu kimliği zaten başka satırdaysa bu satır yinelenendir
                    let n = c.execute(
                        "UPDATE bookmarks SET server_id = ?1
                         WHERE local_id = ?2
                           AND NOT EXISTS (SELECT 1 FROM bookmarks WHERE server_id = ?1)",
                        params![dto.id, local_id],
                    )?;
                    if n == 0 {
                        c.execute("DELETE FROM bookmarks WHERE local_id = ?1", [local_id])?;
                    }
                    Ok(())
                })?;
                *count += 1;
            }
            Err(HErr::Status {
                code: code @ (400 | 409),
                message,
                ..
            }) => {
                // kalıcı ret (sınır/doğrulama): satır düşer
                let msg = if message.is_empty() {
                    format!("Yer imi sunucuya eklenemedi (HTTP {code}).")
                } else {
                    message
                };
                core.with_db(|c| {
                    c.execute("DELETE FROM bookmarks WHERE local_id = ?1", [local_id])?;
                    db::kv_set(c, "sync_last_error", &msg);
                    Ok(())
                })?;
            }
            Err(e) => return Err(into_core(e)),
        }
    }
    Ok(())
}

struct PingRow {
    client_id: String,
    book_id: i64,
    active_seconds: i64,
    pages_read: i64,
    start_percent: f64,
    end_percent: f64,
    local_date: String,
    sent_seconds: i64,
}

fn push_pings(core: &Core, count: &mut i32) -> Result<(), CoreError> {
    let rows: Vec<PingRow> = core.with_db(|c| {
        let mut st = c.prepare(
            "SELECT client_id, book_id, active_seconds, pages_read, start_percent,
                    end_percent, local_date, sent_seconds
             FROM pings WHERE active_seconds > 0 AND active_seconds > sent_seconds",
        )?;
        let rows = st.query_map([], |r| {
            Ok(PingRow {
                client_id: r.get(0)?,
                book_id: r.get(1)?,
                active_seconds: r.get(2)?,
                pages_read: r.get(3)?,
                start_percent: r.get(4)?,
                end_percent: r.get(5)?,
                local_date: r.get(6)?,
                sent_seconds: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    })?;
    for p in rows {
        debug_assert!(p.active_seconds > p.sent_seconds);
        let body = json!({
            "client_id": p.client_id,
            "book_id": p.book_id,
            "active_seconds": p.active_seconds,
            "pages_read": p.pages_read,
            "start_percent": p.start_percent,
            "end_percent": p.end_percent,
            "local_date": p.local_date,
        });
        match core.http.send_json("POST", "/api/reading/ping", &body) {
            Ok(_) => {
                core.with_db(|c| {
                    c.execute(
                        "UPDATE pings SET sent_seconds = ?1 WHERE client_id = ?2",
                        params![p.active_seconds, p.client_id],
                    )?;
                    Ok(())
                })?;
                *count += 1;
            }
            Err(HErr::Status {
                code: 400 | 404, ..
            }) => {
                core.with_db(|c| {
                    c.execute("DELETE FROM pings WHERE client_id = ?1", [&p.client_id])?;
                    Ok(())
                })?;
            }
            Err(e) => return Err(into_core(e)),
        }
    }
    // kapanmış ve tamamen gönderilmiş (ya da hiç süre biriktirmemiş) satırlar silinir
    core.with_db(|c| {
        c.execute(
            "DELETE FROM pings
             WHERE closed = 1 AND (sent_seconds = active_seconds OR active_seconds = 0)",
            [],
        )?;
        Ok(())
    })
}

/// Giden kuyruk: ilerleme/durum -> yer imleri -> ping. İtilen işlem sayısını döner.
pub(crate) fn push(core: &Core) -> Result<i32, CoreError> {
    core.require_session()?;
    let guard = PUSH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut count = 0;
    let mut pull: Vec<i64> = Vec::new();
    push_progress(core, &mut count, &mut pull)?;
    push_bookmarks(core, &mut count)?;
    push_pings(core, &mut count)?;
    drop(guard);
    core.persist_offset();
    // sunucu daha yeni olan kitaplar: hata yutulur (sonraki yenileme yine çeker)
    for id in pull {
        let _ = pull_progress(core, id, None);
    }
    Ok(count)
}

fn sync_now_inner(core: &Core) -> Result<SyncReport, CoreError> {
    let pushed1 = push(core)?;
    let pulled = library::refresh(core)?;
    pull_all_bookmarks(core)?;
    let pushed2 = push(core)?;
    core.persist_offset();
    let now = core.now();
    core.with_db(|c| {
        db::kv_set(c, "sync_last_ok_at", &now.to_string());
        db::kv_del(c, "sync_last_error");
        Ok(())
    })?;
    Ok(SyncReport {
        pushed: pushed1 + pushed2,
        pulled_books: pulled,
    })
}

#[uniffi::export]
impl Core {
    /// push + kütüphane yenile + ilerleme/yer imi çek
    pub fn sync_now(&self) -> Result<SyncReport, CoreError> {
        self.require_session()?;
        let r = sync_now_inner(self);
        if let Err(e) = &r {
            record_error(self, e);
        }
        r
    }

    /// Yalnız giden kuyruk; itilen işlem sayısı.
    pub fn sync_push(&self) -> Result<i32, CoreError> {
        let r = push(self);
        match &r {
            Ok(_) => {
                // gösterge "eşitlendi"ye dönebilsin
                let now = self.now();
                let _ = self.with_db(|c| {
                    db::kv_set(c, "sync_last_ok_at", &now.to_string());
                    db::kv_del(c, "sync_last_error");
                    Ok(())
                });
            }
            Err(e) => record_error(self, e),
        }
        r
    }

    /// Bekleyen "başka cihaz konumu" önerisi; yoksa None. Ağ yok.
    pub fn position_suggestion(
        &self,
        book_id: i64,
    ) -> Result<Option<PositionSuggestion>, CoreError> {
        self.with_db(|c| {
            Ok(c.query_row(
                "SELECT remote_locator, remote_percent, remote_updated_at FROM progress
                 WHERE book_id = ?1 AND remote_updated_at > 0 AND remote_updated_at > updated_at
                   AND remote_updated_at > declined_ts AND remote_locator <> ''",
                [book_id],
                |r| {
                    Ok(PositionSuggestion {
                        locator: r.get(0)?,
                        percent: r.get(1)?,
                        updated_at: r.get(2)?,
                    })
                },
            )
            .optional()?)
        })
    }

    /// "Oraya git": sunucu konumu yerele uygulanır, bayraklar ve öneri temizlenir.
    pub fn accept_position(&self, book_id: i64) -> Result<(), CoreError> {
        self.with_db(|c| {
            c.execute(
                "UPDATE progress SET locator = remote_locator, percent = remote_percent,
                        status = CASE WHEN remote_status IN ('unread','reading','finished','dropped')
                                      THEN remote_status ELSE status END,
                        updated_at = remote_updated_at, p_dirty = 0, s_dirty = 0, last_op = '',
                        remote_locator = '', remote_percent = 0, remote_status = '',
                        remote_updated_at = 0, declined_ts = 0
                 WHERE book_id = ?1 AND remote_updated_at > 0",
                [book_id],
            )?;
            Ok(())
        })
    }

    /// "Burada kal": yerel konum korunur; aynı sunucu damgası için bir daha sorulmaz.
    pub fn decline_position(&self, book_id: i64) -> Result<(), CoreError> {
        self.with_db(|c| {
            c.execute(
                "UPDATE progress SET declined_ts = remote_updated_at, remote_locator = '',
                        remote_percent = 0, remote_status = '', remote_updated_at = 0
                 WHERE book_id = ?1 AND remote_updated_at > 0",
                [book_id],
            )?;
            Ok(())
        })
    }

    pub fn sync_status(&self) -> SyncStatus {
        self.with_db(|c| {
            let pending: i64 = c.query_row(
                "SELECT
                   (SELECT COUNT(*) FROM progress WHERE p_dirty = 1)
                 + (SELECT COUNT(*) FROM progress WHERE s_dirty = 1)
                 + (SELECT COUNT(*) FROM bookmarks
                      WHERE (server_id IS NULL AND deleted = 0)
                         OR (deleted = 1 AND server_id IS NOT NULL))
                 + (SELECT COUNT(*) FROM pings
                      WHERE active_seconds > 0 AND active_seconds > sent_seconds)",
                [],
                |r| r.get(0),
            )?;
            Ok(SyncStatus {
                pending_ops: pending as i32,
                last_ok_at: db::kv_get(c, "sync_last_ok_at").and_then(|v| v.parse().ok()),
                last_error: db::kv_get(c, "sync_last_error").filter(|v| !v.is_empty()),
            })
        })
        .unwrap_or(SyncStatus {
            pending_ops: 0,
            last_ok_at: None,
            last_error: None,
        })
    }

    /// Tek kitabın ilerleme + yer imi; istek başına 5 sn zaman aşımı.
    pub fn pull_book(&self, book_id: i64) -> Result<(), CoreError> {
        let t = Some(Duration::from_secs(5));
        pull_progress(self, book_id, t)?;
        pull_bookmarks(self, book_id, t)?;
        Ok(())
    }

    pub fn get_progress(&self, book_id: i64) -> Result<ProgressInfo, CoreError> {
        self.with_db(|c| {
            let row: Option<(String, f64, String)> = c
                .query_row(
                    "SELECT locator, percent, status FROM progress WHERE book_id = ?1",
                    [book_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            Ok(match row {
                Some((locator, percent, status)) => ProgressInfo {
                    locator,
                    percent,
                    status: ReadStatus::from_db(&status),
                },
                None => ProgressInfo {
                    locator: String::new(),
                    percent: 0.0,
                    status: ReadStatus::Unread,
                },
            })
        })
    }

    pub fn save_progress(
        &self,
        book_id: i64,
        locator: String,
        percent: f64,
        local_date: String,
    ) -> Result<(), CoreError> {
        if !percent.is_finite() || !(0.0..=1.0).contains(&percent) {
            return Err(CoreError::InvalidInput(
                "Geçersiz ilerleme yüzdesi.".to_string(),
            ));
        }
        if locator.len() > 200 {
            return Err(CoreError::InvalidInput(
                "Konum bilgisi çok uzun.".to_string(),
            ));
        }
        let now = self.now();
        self.with_db(|c| {
            let prev: Option<(String, String, f64)> = c
                .query_row(
                    "SELECT status, locator, percent FROM progress WHERE book_id = ?1",
                    [book_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            // aynı konum yeniden bildirilirse (ör. kitabı yalnız açmak) damga ve kuyruk değişmez
            if let Some((_, ref loc, pct)) = prev {
                if *loc == locator && pct == percent {
                    return Ok(());
                }
            }
            let status = derive_status(prev.as_ref().map(|p| p.0.as_str()), percent);
            c.execute(
                "INSERT INTO progress(book_id, locator, percent, status, updated_at,
                                      local_date, p_dirty, last_op)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, 'p')
                 ON CONFLICT(book_id) DO UPDATE SET
                    locator = excluded.locator,
                    percent = excluded.percent,
                    status = excluded.status,
                    updated_at = excluded.updated_at,
                    local_date = excluded.local_date,
                    p_dirty = 1, last_op = 'p'",
                params![book_id, locator, percent, status, now, local_date],
            )?;
            Ok(())
        })
    }

    pub fn set_status(
        &self,
        book_id: i64,
        status: ReadStatus,
        local_date: String,
    ) -> Result<(), CoreError> {
        let now = self.now();
        self.with_db(|c| {
            c.execute(
                "INSERT INTO progress(book_id, locator, percent, status, updated_at,
                                      local_date, s_dirty, last_op)
                 VALUES (?1, '', 0, ?2, ?3, ?4, 1, 's')
                 ON CONFLICT(book_id) DO UPDATE SET
                    status = excluded.status,
                    updated_at = excluded.updated_at,
                    local_date = excluded.local_date,
                    s_dirty = 1, last_op = 's'",
                params![book_id, status.as_str(), now, local_date],
            )?;
            Ok(())
        })
    }

    /// Okuma süresi oturumu başlatır; `client_id` döner.
    pub fn ping_start(
        &self,
        book_id: i64,
        start_percent: f64,
        local_date: String,
    ) -> Result<String, CoreError> {
        let id = util::client_id();
        let sp = clamp01(start_percent);
        self.with_db(|c| {
            c.execute(
                "INSERT INTO pings(client_id, book_id, active_seconds, pages_read,
                                   start_percent, end_percent, local_date, sent_seconds, closed)
                 VALUES (?1, ?2, 0, 0, ?3, ?3, ?4, -1, 0)",
                params![id, book_id, sp, local_date],
            )?;
            Ok(())
        })?;
        Ok(id)
    }

    pub fn ping_update(
        &self,
        client_id: String,
        active_seconds: i64,
        pages_read: i64,
        end_percent: f64,
    ) -> Result<(), CoreError> {
        let active = active_seconds.clamp(0, 86_400);
        let pages = pages_read.clamp(0, 100_000);
        let end = clamp01(end_percent);
        self.with_db(|c| {
            let n = c.execute(
                "UPDATE pings SET active_seconds = MAX(active_seconds, ?2),
                                  pages_read = MAX(pages_read, ?3),
                                  end_percent = ?4
                 WHERE client_id = ?1",
                params![client_id, active, pages, end],
            )?;
            if n == 0 {
                return Err(CoreError::NotFound);
            }
            Ok(())
        })
    }

    pub fn ping_close(&self, client_id: String) -> Result<(), CoreError> {
        self.with_db(|c| {
            let n = c.execute(
                "UPDATE pings SET closed = 1 WHERE client_id = ?1",
                [client_id],
            )?;
            if n == 0 {
                return Err(CoreError::NotFound);
            }
            Ok(())
        })
    }

    pub fn list_bookmarks(&self, book_id: i64) -> Result<Vec<BookmarkItem>, CoreError> {
        self.with_db(|c| {
            let mut st = c.prepare(
                "SELECT local_id, locator, label, created_at, server_id IS NULL
                 FROM bookmarks WHERE book_id = ?1 AND deleted = 0
                 ORDER BY created_at DESC, local_id DESC",
            )?;
            let rows = st.query_map([book_id], |r| {
                Ok(BookmarkItem {
                    local_id: r.get(0)?,
                    locator: r.get(1)?,
                    label: r.get(2)?,
                    created_at: r.get(3)?,
                    pending: r.get::<_, i64>(4)? != 0,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn add_bookmark(
        &self,
        book_id: i64,
        locator: String,
        label: String,
    ) -> Result<BookmarkItem, CoreError> {
        let label = label.trim().to_string();
        if locator.is_empty()
            || locator.len() > 200
            || label.is_empty()
            || label.chars().count() > 120
        {
            return Err(CoreError::InvalidInput("Geçersiz yer imi.".to_string()));
        }
        let now = self.now();
        self.with_db(|c| {
            let count: i64 = c.query_row(
                "SELECT COUNT(*) FROM bookmarks WHERE book_id = ?1 AND deleted = 0",
                [book_id],
                |r| r.get(0),
            )?;
            if count >= MAX_BOOKMARKS_PER_BOOK {
                return Err(CoreError::InvalidInput(
                    "Yer imi sınırına ulaşıldı.".to_string(),
                ));
            }
            c.execute(
                "INSERT INTO bookmarks(server_id, book_id, locator, label, created_at, deleted)
                 VALUES (NULL, ?1, ?2, ?3, ?4, 0)",
                params![book_id, locator, label, now],
            )?;
            Ok(BookmarkItem {
                local_id: c.last_insert_rowid(),
                locator,
                label,
                created_at: now,
                pending: true,
            })
        })
    }

    pub fn delete_bookmark(&self, local_id: i64) -> Result<(), CoreError> {
        self.with_db(|c| {
            let row: Option<Option<i64>> = c
                .query_row(
                    "SELECT server_id FROM bookmarks WHERE local_id = ?1 AND deleted = 0",
                    [local_id],
                    |r| r.get(0),
                )
                .optional()?;
            match row {
                None => Err(CoreError::NotFound),
                Some(None) => {
                    c.execute("DELETE FROM bookmarks WHERE local_id = ?1", [local_id])?;
                    Ok(())
                }
                Some(Some(_)) => {
                    c.execute(
                        "UPDATE bookmarks SET deleted = 1 WHERE local_id = ?1",
                        [local_id],
                    )?;
                    Ok(())
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lp(updated_at: i64, pending: bool) -> LocalProg {
        LocalProg {
            updated_at,
            pending,
        }
    }

    fn sp(updated_at: i64, has_row: bool) -> ServerProg {
        ServerProg {
            updated_at,
            has_row,
        }
    }

    #[test]
    fn merge_progress_table() {
        // sunucuda kayıt yok -> yerel
        assert_eq!(merge_progress(Some(&lp(5, false)), &sp(0, false)), Merge::KeepLocal);
        assert_eq!(merge_progress(None, &sp(0, false)), Merge::KeepLocal);
        // yerel yok -> sunucu
        assert_eq!(merge_progress(None, &sp(7, true)), Merge::TakeServer);
        // bekleyen + sunucu daha yeni -> sunucu
        assert_eq!(merge_progress(Some(&lp(5, true)), &sp(6, true)), Merge::TakeServer);
        // bekleyen + eşit -> yerel
        assert_eq!(merge_progress(Some(&lp(5, true)), &sp(5, true)), Merge::KeepLocal);
        // bekleyen + yerel daha yeni -> yerel
        assert_eq!(merge_progress(Some(&lp(9, true)), &sp(5, true)), Merge::KeepLocal);
        // bekleyen değil -> sunucu
        assert_eq!(merge_progress(Some(&lp(9, false)), &sp(5, true)), Merge::TakeServer);
    }

    fn dto(locator: &str, updated_at: i64) -> ProgressDto {
        ProgressDto {
            locator: locator.to_string(),
            percent: 0.5,
            status: "reading".to_string(),
            updated_at,
        }
    }

    #[test]
    fn suggest_instead_table() {
        // yerel konum boş: sunucu sessizce alınır
        assert!(!suggest_instead("", 10, &dto("9", 20)));
        // sunucu konumu boş
        assert!(!suggest_instead("5", 10, &dto("", 20)));
        // eşit konum
        assert!(!suggest_instead("5", 10, &dto("5", 20)));
        // sunucu damgası eşit ya da eski
        assert!(!suggest_instead("5", 10, &dto("9", 10)));
        assert!(!suggest_instead("5", 10, &dto("9", 9)));
        // farklı ve daha yeni
        assert!(suggest_instead("5", 10, &dto("9", 11)));
    }

    #[test]
    fn local_status_derivation() {
        assert_eq!(derive_status(None, 0.5), "reading");
        assert_eq!(derive_status(None, 1.0), "finished");
        assert_eq!(derive_status(Some("finished"), 0.3), "finished");
        assert_eq!(derive_status(Some("unread"), 0.3), "reading");
        assert_eq!(derive_status(Some("dropped"), 0.3), "reading");
        assert_eq!(derive_status(None, 0.995), "reading");
    }

    fn lb(local_id: i64, server_id: Option<i64>, loc: &str, label: &str, deleted: bool) -> LocalBm {
        LocalBm {
            local_id,
            server_id,
            locator: loc.to_string(),
            label: label.to_string(),
            deleted,
        }
    }

    fn sb(id: i64, loc: &str, label: &str) -> SrvBm {
        SrvBm {
            id,
            locator: loc.to_string(),
            label: label.to_string(),
            created_at: 100 + id,
        }
    }

    #[test]
    fn merge_bookmarks_insert_update_adopt_remove() {
        // Insert
        assert_eq!(
            merge_bookmarks(&[], &[sb(1, "a", "A")]),
            vec![BmOp::Insert(sb(1, "a", "A"))]
        );
        // Update
        assert_eq!(
            merge_bookmarks(&[lb(10, Some(1), "old", "Old", false)], &[sb(1, "a", "A")]),
            vec![BmOp::Update {
                local_id: 10,
                locator: "a".to_string(),
                label: "A".to_string(),
                created_at: 101
            }]
        );
        // Adopt: aynı locator+label; ikinci sunucu kaydına tekrar verilmez
        assert_eq!(
            merge_bookmarks(&[lb(10, None, "a", "A", false)], &[sb(1, "a", "A"), sb(2, "a", "A")]),
            vec![
                BmOp::Adopt {
                    local_id: 10,
                    server_id: 1
                },
                BmOp::Insert(sb(2, "a", "A"))
            ]
        );
        // sunucuda olmayan server_id'li yerel -> Remove (silinmiş olsa da)
        assert_eq!(
            merge_bookmarks(
                &[lb(10, Some(5), "a", "A", false), lb(11, Some(6), "b", "B", true)],
                &[]
            ),
            vec![BmOp::Remove { local_id: 10 }, BmOp::Remove { local_id: 11 }]
        );
        // yerelde silinmiş (DELETE bekliyor) -> işlem yok
        assert!(merge_bookmarks(&[lb(10, Some(1), "a", "A", true)], &[sb(1, "a", "A")]).is_empty());
        // server_id IS NULL satıra dokunulmaz (sunucuda eşleşme yoksa)
        assert!(merge_bookmarks(&[lb(10, None, "x", "X", false)], &[]).is_empty());
    }

    fn prog_row(core: &Core) -> (String, i64, i64, String) {
        core.with_db(|c| {
            Ok(c.query_row(
                "SELECT status, p_dirty, s_dirty, last_op FROM progress WHERE book_id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?)
        })
        .unwrap()
    }

    #[test]
    fn save_progress_and_set_status_flags() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::new(dir.path().to_string_lossy().to_string()).unwrap();
        core.save_progress(1, "3".into(), 0.5, "2026-11-08".into()).unwrap();
        assert_eq!(prog_row(&core), ("reading".to_string(), 1, 0, "p".to_string()));
        core.save_progress(1, "9".into(), 1.0, "2026-11-08".into()).unwrap();
        assert_eq!(prog_row(&core).0, "finished");
        core.save_progress(1, "1".into(), 0.3, "2026-11-08".into()).unwrap();
        assert_eq!(prog_row(&core).0, "finished");
        core.set_status(1, ReadStatus::Unread, "2026-11-08".into()).unwrap();
        assert_eq!(prog_row(&core), ("unread".to_string(), 1, 1, "s".to_string()));
        let p = core.get_progress(1).unwrap();
        assert_eq!(p.locator, "1");
        assert_eq!(p.status, ReadStatus::Unread);
        assert!(core.save_progress(1, "x".into(), 1.5, "d".into()).is_err());
        assert!(core.save_progress(1, "x".repeat(201), 0.5, "d".into()).is_err());
        assert_eq!(core.sync_status().pending_ops, 2);
    }

    #[test]
    fn save_progress_same_position_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::new(dir.path().to_string_lossy().to_string()).unwrap();
        core.save_progress(1, "3".into(), 0.5, "2026-11-08".into()).unwrap();
        // gönderilmiş gibi: bayrak temiz, damga eski
        core.with_db(|c| {
            c.execute("UPDATE progress SET p_dirty = 0, updated_at = 1000 WHERE book_id = 1", [])?;
            Ok(())
        })
        .unwrap();
        // aynı konum: hiçbir şey yazılmaz
        core.save_progress(1, "3".into(), 0.5, "2026-11-09".into()).unwrap();
        let row = |core: &Core| -> (i64, i64, String) {
            core.with_db(|c| {
                Ok(c.query_row(
                    "SELECT updated_at, p_dirty, local_date FROM progress WHERE book_id = 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?)
            })
            .unwrap()
        };
        assert_eq!(row(&core), (1000, 0, "2026-11-08".to_string()));
        assert_eq!(core.sync_status().pending_ops, 0);
        // farklı konum yazılır
        core.save_progress(1, "4".into(), 0.6, "2026-11-09".into()).unwrap();
        let (ts, dirty, date) = row(&core);
        assert!(ts > 1000 && dirty == 1 && date == "2026-11-09");
        // aynı locator, farklı yüzde de yazılır
        core.with_db(|c| {
            c.execute("UPDATE progress SET p_dirty = 0, updated_at = 1000 WHERE book_id = 1", [])?;
            Ok(())
        })
        .unwrap();
        core.save_progress(1, "4".into(), 0.7, "2026-11-09".into()).unwrap();
        assert_eq!(row(&core).1, 1);
    }

    #[test]
    fn bookmark_local_rules() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::new(dir.path().to_string_lossy().to_string()).unwrap();
        assert!(core.add_bookmark(1, "".into(), "x".into()).is_err());
        assert!(core.add_bookmark(1, "a".into(), "   ".into()).is_err());
        let b = core.add_bookmark(1, "a".into(), " Etiket ".into()).unwrap();
        assert_eq!(b.label, "Etiket");
        assert!(b.pending);
        assert_eq!(core.list_bookmarks(1).unwrap().len(), 1);
        core.delete_bookmark(b.local_id).unwrap();
        assert!(core.list_bookmarks(1).unwrap().is_empty());
        assert!(matches!(core.delete_bookmark(b.local_id), Err(CoreError::NotFound)));
        for i in 0..100 {
            core.add_bookmark(1, format!("{i}"), format!("b{i}")).unwrap();
        }
        assert!(core.add_bookmark(1, "z".into(), "z".into()).is_err());
    }

    #[test]
    fn ping_lifecycle_clamps_and_keeps_max() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::new(dir.path().to_string_lossy().to_string()).unwrap();
        let id = core.ping_start(1, 0.2, "2026-11-08".into()).unwrap();
        assert_eq!(id.len(), 32);
        core.ping_update(id.clone(), 30, 3, 0.4).unwrap();
        core.ping_update(id.clone(), 10, 1, 0.5).unwrap();
        core.ping_update(id.clone(), 999_999, 9, 7.0).unwrap();
        let (active, pages, end): (i64, i64, f64) = core
            .with_db(|c| {
                Ok(c.query_row(
                    "SELECT active_seconds, pages_read, end_percent FROM pings WHERE client_id = ?1",
                    [&id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )?)
            })
            .unwrap();
        assert_eq!((active, pages), (86_400, 9));
        assert_eq!(end, 1.0);
        core.ping_close(id).unwrap();
        assert!(matches!(
            core.ping_update("yok".into(), 1, 1, 0.1),
            Err(CoreError::NotFound)
        ));
    }
}
