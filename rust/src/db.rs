//! SQLite 连接与结构校验。
//!
//! 三份实现共用一个 workbench.db。本模块**不主动迁移**：
//! 库版本低于期望值时提示用户先用 Node 版完成迁移，
//! 避免两个实现各自跑迁移把对方数据搞坏。

use rusqlite::Connection;
use std::path::PathBuf;

/// 本实现期望的库版本，与 Node 版一致。
pub const SCHEMA_VERSION: i64 = 4;

/// 数据库文件路径（与 Node 版同一位置）。
pub fn path() -> Option<PathBuf> {
    let base = dirs_config()?;
    Some(base.join("claude-workbench").join("workbench.db"))
}

fn dirs_config() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
    }
}

/// 打开数据库并做完整性校验。
pub fn open() -> Result<Connection, String> {
    let p = path().ok_or("无法定位用户配置目录")?;
    if !p.exists() {
        return Err(format!(
            "数据库不存在：{}\n请先用 Node 版启动一次以创建数据库",
            p.display()
        ));
    }

    let conn = Connection::open(&p).map_err(|e| format!("打开数据库失败: {e}"))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .map_err(|e| format!("设置连接参数失败: {e}"))?;

    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| format!("完整性检查失败: {e}"))?;
    if integrity != "ok" {
        return Err(format!("数据库损坏（integrity_check = {integrity}）"));
    }

    let version = schema_version(&conn)?;
    if version < SCHEMA_VERSION {
        return Err(format!(
            "数据库版本为 {version}，本实现需要 {SCHEMA_VERSION}。\n请先用 Node 版启动一次完成迁移"
        ));
    }

    Ok(conn)
}

fn schema_version(conn: &Connection) -> Result<i64, String> {
    let raw: Option<String> = conn
        .query_row("SELECT value FROM meta WHERE key = 'schema_version'", [], |r| r.get(0))
        .ok();
    Ok(raw.and_then(|s| s.parse().ok()).unwrap_or(0))
}

/// 与 Node 版一致的时间格式（ISO 8601 带毫秒，UTC）。
pub fn now() -> String {
    // 不引入 chrono，用系统时钟手算；格式只要保持字典序可比较即可
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let millis = d.subsec_millis();

    // 由 Unix 时间戳换算 UTC 年月日时分秒
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, da) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{da:02}T{h:02}:{mi:02}:{s:02}.{millis:03}Z")
}

/// Howard Hinnant 的 days → civil date 算法。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format_looks_right() {
        let t = now();
        // 2026-10-27T18:00:00.000Z
        assert_eq!(t.len(), 24, "格式长度不对: {t}");
        assert!(t.ends_with('Z'));
        assert_eq!(&t[4..5], "-");
        assert_eq!(&t[10..11], "T");
    }
}
