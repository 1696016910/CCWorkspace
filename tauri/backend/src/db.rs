//! 数据库：schema 与迁移。
//!
//! 与 Node 版共用同一个库文件，schema 版本号必须与其一致（当前 4）。
//! 迁移只增不改——已发布的结构不再变动，避免用户库漂移。

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;

pub const SCHEMA_VERSION: i32 = 4;

/// 与 Node 版逐条对应的迁移脚本。
const MIGRATIONS: &[(i32, &str)] = &[
    (
        1,
        "
        CREATE TABLE IF NOT EXISTS workspaces (
          id          TEXT PRIMARY KEY,
          path        TEXT NOT NULL UNIQUE,
          name        TEXT NOT NULL,
          created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
          id           TEXT PRIMARY KEY,
          workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
          title        TEXT NOT NULL DEFAULT '',
          auto_title   INTEGER NOT NULL DEFAULT 1,
          created_at   TEXT NOT NULL,
          updated_at   TEXT NOT NULL,
          deleted_at   TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_workspace
          ON sessions(workspace_id, updated_at DESC);

        CREATE TABLE IF NOT EXISTS messages (
          id         TEXT PRIMARY KEY,
          session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
          role       TEXT NOT NULL,
          content    TEXT NOT NULL,
          tools      TEXT,
          created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_messages_session
          ON messages(session_id, created_at);

        CREATE TABLE IF NOT EXISTS meta (
          key   TEXT PRIMARY KEY,
          value TEXT NOT NULL
        );
        ",
    ),
    (
        2,
        "
        ALTER TABLE sessions ADD COLUMN claude_session_id TEXT;
        ALTER TABLE sessions ADD COLUMN context_tokens INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE sessions ADD COLUMN context_max INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE sessions ADD COLUMN context_percent REAL NOT NULL DEFAULT 0;
        ",
    ),
    (
        3,
        "
        ALTER TABLE workspaces ADD COLUMN pinned_at TEXT;
        ALTER TABLE sessions ADD COLUMN pinned_at TEXT;
        ",
    ),
    (4, "ALTER TABLE messages ADD COLUMN stopped INTEGER NOT NULL DEFAULT 0;"),
];

/// 全局连接。Tauri 命令都是同步短事务，用一把锁足够。
static DB: Mutex<Option<Connection>> = Mutex::new(None);

/// 库文件路径：%APPDATA%/claude-workbench/workbench.db，与另外两版共用。
pub fn path() -> Result<PathBuf, String> {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .ok_or("无法定位应用数据目录")?;
    Ok(base.join("claude-workbench").join("workbench.db"))
}

/// 打开数据库并补齐迁移。
pub fn open() -> Result<(), String> {
    let p = path()?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    }
    let conn = Connection::open(&p).map_err(|e| format!("打开数据库失败: {e}"))?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| format!("初始化 PRAGMA 失败: {e}"))?;

    // 版本检测：比当前新说明是别的版本写的，直接拒绝，避免写坏
    let current: i32 = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if current > SCHEMA_VERSION {
        return Err(format!(
            "数据库版本 {current} 高于本程序支持的 {SCHEMA_VERSION}，请升级后再打开"
        ));
    }

    for (v, sql) in MIGRATIONS {
        if *v > current {
            conn.execute_batch(sql)
                .map_err(|e| format!("执行迁移 v{v} 失败: {e}"))?;
        }
    }
    if current < SCHEMA_VERSION {
        conn.execute(
            "INSERT INTO meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [SCHEMA_VERSION.to_string()],
        )
        .map_err(|e| format!("写入 schema_version 失败: {e}"))?;
    }

    *DB.lock().map_err(|_| "数据库锁中毒")? = Some(conn);
    Ok(())
}

/// 在连接上执行一段操作。连接未打开时返回错误。
pub fn with<T>(f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let guard = DB.lock().map_err(|_| "数据库锁中毒")?;
    let conn = guard.as_ref().ok_or("数据库未打开")?;
    f(conn)
}

/// 与 Node 版一致的时间格式：ISO 8601 带毫秒 + Z。
pub fn now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let secs = (ms / 1000) as i64;
    let millis = ms % 1000;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// unix 天数 → 年月日（Howard Hinnant 算法）。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// 供单元测试构建内存库使用。
#[cfg(test)]
pub fn migrations_for_test() -> &'static [(i32, &'static str)] {
    MIGRATIONS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format_matches_node() {
        let t = now();
        // 2026-10-28T05:06:07.123Z
        assert_eq!(t.len(), 24, "实际: {t}");
        assert!(t.ends_with('Z'));
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], "T");
    }

    #[test]
    fn civil_from_days_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }

    #[test]
    fn migrations_cover_all_versions() {
        // 每个版本都要有脚本，否则打开旧库会缺列
        for v in 1..=SCHEMA_VERSION {
            assert!(
                MIGRATIONS.iter().any(|(m, _)| *m == v),
                "缺少 v{v} 的迁移"
            );
        }
    }
}
