//! SQLite：翻译缓存与术语表。
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

pub struct Db {
    conn: Mutex<Connection>,
}

impl Db {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS cache (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL,
               created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS glossary (
               source TEXT PRIMARY KEY,
               target TEXT NOT NULL
             );",
        )?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn cache_get(&self, key: &str) -> Option<String> {
        let conn = self.conn.lock().ok()?;
        let mut stmt = conn.prepare("SELECT value FROM cache WHERE key = ?1").ok()?;
        stmt.query_row(params![key], |row| row.get::<_, String>(0)).ok()
    }

    pub fn cache_put(&self, key: &str, value: &str) {
        if let Ok(conn) = self.conn.lock() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = conn.execute(
                "INSERT OR REPLACE INTO cache (key, value, created_at) VALUES (?1, ?2, ?3)",
                params![key, value, now as i64],
            );
        }
    }

    pub fn glossary(&self) -> Vec<(String, String)> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut stmt = match conn.prepare("SELECT source, target FROM glossary LIMIT 200") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        });
        match rows {
            Ok(iter) => iter.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn cache_trim(&self, keep: usize) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "DELETE FROM cache WHERE key NOT IN (SELECT key FROM cache ORDER BY created_at DESC LIMIT ?1)",
                params![keep as i64],
            );
        }
    }
}
