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
             );
             CREATE TABLE IF NOT EXISTS history (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               created_at INTEGER NOT NULL,
               src TEXT NOT NULL,
               tgt TEXT NOT NULL,
               tone TEXT NOT NULL,
               source TEXT NOT NULL,
               output TEXT NOT NULL,
               cached INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS usage (
               day TEXT PRIMARY KEY,
               count INTEGER NOT NULL DEFAULT 0
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

impl Db {
    pub fn bump_usage(&self) {
        let day = {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            // 粗略按 UTC+8 计日
            let days = (now + 8 * 3600) / 86400;
            let secs = days * 86400;
            let dt = time_from_unix(secs);
            format!("{}-{:02}-{:02}", dt.0, dt.1, dt.2)
        };
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT INTO usage (day, count) VALUES (?1, 1)
                 ON CONFLICT(day) DO UPDATE SET count = count + 1",
                rusqlite::params![day],
            );
        }
    }

    pub fn usage_today(&self) -> i64 {
        let day = {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let days = (now + 8 * 3600) / 86400;
            let secs = days * 86400;
            let dt = time_from_unix(secs);
            format!("{}-{:02}-{:02}", dt.0, dt.1, dt.2)
        };
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return 0,
        };
        conn.query_row(
            "SELECT count FROM usage WHERE day = ?1",
            rusqlite::params![day],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
    }

    pub fn cache_count(&self) -> i64 {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return 0,
        };
        conn.query_row("SELECT COUNT(*) FROM cache", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0)
    }

    pub fn glossary_put(&self, source: &str, target: &str) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO glossary (source, target) VALUES (?1, ?2)",
                rusqlite::params![source, target],
            );
        }
    }
}

fn time_from_unix(secs: u64) -> (i64, u32, u32) {
    // 简化版公历换算（1970-2100 范围内够用）
    let days = (secs / 86400) as i64;
    let mut year = 1970i64;
    let mut remaining = days;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let year_days = if leap { 366 } else { 365 };
        if remaining < year_days {
            break;
        }
        remaining -= year_days;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let months = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0usize;
    for (i, m) in months.iter().enumerate() {
        if remaining < *m as i64 {
            month = i;
            break;
        }
        remaining -= *m as i64;
    }
    (year, (month + 1) as u32, (remaining + 1) as u32)
}

impl Db {
    /// 今日翻译次数：取 usage 表与今日缓存条目的较大值
    pub fn translations_today(&self) -> i64 {
        let usage = self.usage_today();
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return usage,
        };
        let start = {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() + 8 * 3600)
                .unwrap_or(0);
            (now / 86400) * 86400 - 8 * 3600
        };
        let cached: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cache WHERE created_at >= ?1",
                rusqlite::params![start as i64],
                |row| row.get(0),
            )
            .unwrap_or(0);
        usage.max(cached)
    }
}

impl Db {
    pub fn add_history(&self, src: &str, tgt: &str, tone: &str, source: &str, output: &str, cached: bool) {
        if let Ok(conn) = self.conn.lock() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = conn.execute(
                "INSERT INTO history (created_at, src, tgt, tone, source, output, cached) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![now as i64, src, tgt, tone, source, output, cached as i64],
            );
        }
    }

    pub fn history(&self, limit: usize) -> Vec<(i64, String, String, String, String, String, bool)> {
        let conn = match self.conn.lock() {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut stmt = match conn.prepare(
            "SELECT created_at, src, tgt, tone, source, output, cached FROM history ORDER BY id DESC LIMIT ?1",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map(rusqlite::params![limit as i64], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)? != 0,
            ))
        });
        match rows {
            Ok(iter) => iter.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn clear_history(&self) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute("DELETE FROM history", []);
        }
    }
}

impl Db {
    pub fn glossary_remove(&self, source: &str) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute("DELETE FROM glossary WHERE source = ?1", rusqlite::params![source]);
        }
    }
}

impl Db {
    /// 只保留最近 N 条历史
    pub fn trim_history(&self, keep: usize) {
        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY id DESC LIMIT ?1)",
                rusqlite::params![keep as i64],
            );
        }
    }
}
