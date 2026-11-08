use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use rusqlite::Connection;

use crate::error::AppError;

const MIGRATION_0001: &str = include_str!("migrations/0001_init.sql");

#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Db> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_millis(5000))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;

        let user_version: i32 =
            conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if user_version < 1 {
            let tx = conn.transaction()?;
            tx.execute_batch(MIGRATION_0001)?;
            tx.pragma_update(None, "user_version", 1)?;
            tx.commit()?;
        }

        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    pub async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, AppError> + Send + 'static,
    ) -> Result<T, AppError> {
        let inner = self.0.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = inner.lock().unwrap_or_else(|e| e.into_inner());
            f(&mut conn)
        })
        .await
        .map_err(|e| {
            tracing::error!("veritabanı görev hatası: {e}");
            AppError::Internal
        })?
    }
}
