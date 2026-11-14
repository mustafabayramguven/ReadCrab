use std::{path::Path, time::Duration};

use rusqlite::{Connection, OptionalExtension};

const MIGRATION_0001: &str = include_str!("migrations/0001_init.sql");
const MIGRATION_0002: &str = include_str!("migrations/0002_remote_pos.sql");

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
    let mut conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_millis(5000))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;

    let user_version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if user_version < 1 {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATION_0001)?;
        tx.pragma_update(None, "user_version", 1)?;
        tx.commit()?;
    }
    let uv: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if uv < 2 {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATION_0002)?;
        tx.pragma_update(None, "user_version", 2)?;
        tx.commit()?;
    }
    Ok(conn)
}

pub fn kv_get(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM kv WHERE key = ?1", [key], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

pub fn kv_set(conn: &Connection, key: &str, value: &str) {
    let _ = conn.execute(
        "INSERT INTO kv(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    );
}

pub fn kv_del(conn: &Connection, key: &str) {
    let _ = conn.execute("DELETE FROM kv WHERE key = ?1", [key]);
}

/// Tüm tabloları boşaltır (kv dahil). Dosyaları `Core` siler. Hata yukarı taşınır.
pub fn wipe_all(conn: &Connection) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute_batch(
        "DELETE FROM kv;
         DELETE FROM series;
         DELETE FROM books;
         DELETE FROM progress;
         DELETE FROM bookmarks;
         DELETE FROM pings;
         DELETE FROM toc;
         DELETE FROM downloads;
         DELETE FROM parts;",
    )?;
    tx.commit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_and_kv_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join("t.db")).unwrap();
        assert_eq!(kv_get(&conn, "a"), None);
        kv_set(&conn, "a", "1");
        kv_set(&conn, "a", "2");
        assert_eq!(kv_get(&conn, "a").as_deref(), Some("2"));
        kv_del(&conn, "a");
        assert_eq!(kv_get(&conn, "a"), None);
        let uv: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(uv, 2);
        // ikinci açılışta göç yeniden çalışmaz
        drop(conn);
        assert!(open(&dir.path().join("t.db")).is_ok());
    }

    #[test]
    fn wipe_all_clears_everything() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join("t.db")).unwrap();
        kv_set(&conn, "token", "x");
        conn.execute("INSERT INTO series(id, ord, name, folded_name) VALUES (1, 0, 'A', 'a')", [])
            .unwrap();
        wipe_all(&conn).unwrap();
        assert_eq!(kv_get(&conn, "token"), None);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM series", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
