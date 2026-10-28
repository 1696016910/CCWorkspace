//! 工作区与会话的读写。语义与 Node 版 services/session.ts、workspace.ts 一致。

use crate::db;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

pub const PAGE_SIZE: i64 = 80;
const TITLE_MAX: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub path: String,
    pub name: String,
    pub added_at: String,
    pub pinned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub updated_at: String,
    pub message_count: i64,
    pub running: bool,
    pub renamed: bool,
    pub context_percent: f64,
    pub context_tokens: i64,
    pub context_max: i64,
    pub pinned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub id: String,
    pub role: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stopped: bool,
}

/// 从首条消息生成标题，按字符截断不切坏 emoji。
pub fn derive_title(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.is_empty() {
        return "(新会话)".into();
    }
    let chars: Vec<char> = one_line.chars().collect();
    if chars.len() <= TITLE_MAX {
        one_line
    } else {
        format!("{}…", chars[..TITLE_MAX].iter().collect::<String>())
    }
}

// ── 工作区 ──

pub fn list_workspaces(conn: &Connection) -> Result<Vec<Workspace>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, path, name, created_at, pinned_at FROM workspaces
              ORDER BY pinned_at IS NULL, pinned_at DESC, created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Workspace {
                id: r.get(0)?,
                path: r.get(1)?,
                name: r.get(2)?,
                added_at: r.get(3)?,
                pinned_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn add_workspace(conn: &Connection, dir: &str, custom_name: &str) -> Result<Workspace, String> {
    let dir = dir.trim();
    if dir.is_empty() {
        return Err("项目目录不能为空".into());
    }
    let p = std::path::Path::new(dir);
    if !p.is_dir() {
        return Err(format!("目录不存在: {dir}"));
    }
    // 规范化后再比较，避免同一目录因斜杠差异被重复添加
    let abs = std::fs::canonicalize(p).map_err(|e| format!("无法访问目录: {e}"))?;
    let abs = abs.to_string_lossy().to_string();

    let exists: Option<String> = conn
        .query_row("SELECT id FROM workspaces WHERE path = ?1", params![abs], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    if exists.is_some() {
        return Err("这个目录已经添加过了".into());
    }

    let name = if custom_name.trim().is_empty() {
        p.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| abs.clone())
    } else {
        custom_name.trim().to_string()
    };

    let id = uuid::Uuid::new_v4().to_string();
    let now = db::now();
    conn.execute(
        "INSERT INTO workspaces(id, path, name, created_at) VALUES(?1, ?2, ?3, ?4)",
        params![id, abs, name, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(Workspace {
        id,
        path: abs,
        name,
        added_at: now,
        pinned_at: None,
    })
}

pub fn remove_workspace(conn: &Connection, id: &str) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM workspaces WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("工作区不存在".into());
    }
    Ok(())
}

pub fn rename_workspace(conn: &Connection, id: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("名称不能为空".into());
    }
    let n = conn
        .execute("UPDATE workspaces SET name = ?1 WHERE id = ?2", params![name, id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("工作区不存在".into());
    }
    Ok(())
}

pub fn pin_workspace(conn: &Connection, id: &str, pinned: bool) -> Result<(), String> {
    let at: Option<String> = if pinned { Some(db::now()) } else { None };
    conn.execute(
        "UPDATE workspaces SET pinned_at = ?1 WHERE id = ?2",
        params![at, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ── 会话 ──

const SESSION_COLS: &str = "s.id, s.workspace_id, s.title, s.auto_title, s.created_at,
    s.updated_at, s.deleted_at, s.claude_session_id, s.context_tokens, s.context_max,
    s.context_percent, s.pinned_at";

fn map_session(r: &rusqlite::Row) -> rusqlite::Result<SessionSummary> {
    let auto_title: i64 = r.get(3)?;
    Ok(SessionSummary {
        id: r.get(0)?,
        workspace_id: r.get(1)?,
        title: r.get(2)?,
        updated_at: r.get(5)?,
        message_count: r.get(12).unwrap_or(0),
        running: false,
        renamed: auto_title == 0,
        context_percent: r.get(10).unwrap_or(0.0),
        context_tokens: r.get(8).unwrap_or(0),
        context_max: r.get(9).unwrap_or(0),
        pinned_at: r.get(11)?,
    })
}

pub fn list_sessions(conn: &Connection) -> Result<Vec<SessionSummary>, String> {
    let sql = format!(
        "SELECT {SESSION_COLS}, COUNT(m.id) AS message_count
           FROM sessions s
           LEFT JOIN messages m ON m.session_id = s.id
          WHERE s.deleted_at IS NULL
          GROUP BY s.id
          ORDER BY s.pinned_at IS NULL, s.pinned_at DESC, s.updated_at DESC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], map_session).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn list_deleted_sessions(conn: &Connection) -> Result<Vec<SessionSummary>, String> {
    let sql = format!(
        "SELECT {SESSION_COLS}, COUNT(m.id) AS message_count
           FROM sessions s
           LEFT JOIN messages m ON m.session_id = s.id
          WHERE s.deleted_at IS NOT NULL
          GROUP BY s.id
          ORDER BY s.deleted_at DESC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], map_session).map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn create_session(conn: &Connection, workspace_id: &str) -> Result<SessionSummary, String> {
    if workspace_id.trim().is_empty() {
        return Err("请先选择一个工作区".into());
    }
    let exists: Option<String> = conn
        .query_row(
            "SELECT id FROM workspaces WHERE id = ?1",
            params![workspace_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        return Err("指定的工作区不存在".into());
    }
    let id = uuid::Uuid::new_v4().to_string();
    let now = db::now();
    conn.execute(
        "INSERT INTO sessions(id, workspace_id, title, auto_title, created_at, updated_at)
         VALUES(?1, ?2, '', 1, ?3, ?3)",
        params![id, workspace_id, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(SessionSummary {
        id,
        workspace_id: workspace_id.to_string(),
        title: String::new(),
        updated_at: now,
        message_count: 0,
        running: false,
        renamed: false,
        context_percent: 0.0,
        context_tokens: 0,
        context_max: 0,
        pinned_at: None,
    })
}

pub fn rename_session(conn: &Connection, id: &str, title: &str) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("标题不能为空".into());
    }
    let n = conn
        .execute(
            "UPDATE sessions SET title = ?1, auto_title = 0 WHERE id = ?2",
            params![title, id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("会话不存在".into());
    }
    Ok(())
}

pub fn delete_session(conn: &Connection, id: &str) -> Result<(), String> {
    let n = conn
        .execute(
            "UPDATE sessions SET deleted_at = ?1 WHERE id = ?2",
            params![db::now(), id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("会话不存在".into());
    }
    Ok(())
}

pub fn restore_session(conn: &Connection, id: &str) -> Result<(), String> {
    let n = conn
        .execute("UPDATE sessions SET deleted_at = NULL WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("会话不存在".into());
    }
    Ok(())
}

pub fn purge_session(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM messages WHERE session_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    let n = conn
        .execute("DELETE FROM sessions WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("会话不存在".into());
    }
    Ok(())
}

pub fn pin_session(conn: &Connection, id: &str, pinned: bool) -> Result<(), String> {
    let at: Option<String> = if pinned { Some(db::now()) } else { None };
    conn.execute(
        "UPDATE sessions SET pinned_at = ?1 WHERE id = ?2",
        params![at, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn duplicate_session(conn: &Connection, id: &str) -> Result<SessionSummary, String> {
    let src: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT workspace_id, title, auto_title FROM sessions WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((workspace_id, title, auto_title)) = src else {
        return Err("会话不存在".into());
    };
    let new_id = uuid::Uuid::new_v4().to_string();
    let now = db::now();
    let copy_title = format!("{title} 副本");
    conn.execute(
        "INSERT INTO sessions(id, workspace_id, title, auto_title, created_at, updated_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?5)",
        params![new_id, workspace_id, copy_title, auto_title, now],
    )
    .map_err(|e| e.to_string())?;
    // 复制消息时重新生成 id，避免主键冲突
    conn.execute(
        "INSERT INTO messages(id, session_id, role, content, tools, created_at, stopped)
         SELECT lower(hex(randomblob(16))), ?1, role, content, tools, created_at, stopped
           FROM messages WHERE session_id = ?2 ORDER BY created_at",
        params![new_id, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(SessionSummary {
        id: new_id,
        workspace_id,
        title: copy_title,
        updated_at: now,
        message_count: 0,
        running: false,
        renamed: auto_title == 0,
        context_percent: 0.0,
        context_tokens: 0,
        context_max: 0,
        pinned_at: None,
    })
}

// ── 消息 ──

pub fn list_messages(
    conn: &Connection,
    session_id: &str,
    limit: i64,
) -> Result<Vec<ChatMessage>, String> {
    // 取最近 N 条再按时间正序，保证界面从旧到新
    let mut stmt = conn
        .prepare(
            "SELECT id, role, content, tools, created_at, stopped FROM (
               SELECT * FROM messages WHERE session_id = ?1
                ORDER BY created_at DESC LIMIT ?2
             ) ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![session_id, limit], |r| {
            let tools_json: Option<String> = r.get(3)?;
            let tools: Vec<ToolCall> = tools_json
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            Ok(ChatMessage {
                id: r.get(0)?,
                role: r.get(1)?,
                text: r.get(2)?,
                tools,
                timestamp: r.get(4)?,
                stopped: r.get::<_, i64>(5).unwrap_or(0) != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn count_messages(conn: &Connection, session_id: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
        params![session_id],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}

pub fn append_message(
    conn: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
    tools: &[ToolCall],
    stopped: bool,
) -> Result<ChatMessage, String> {
    // 会话已删除时拒绝写入，避免消息挂到回收站里的会话上
    let exists: Option<String> = conn
        .query_row(
            "SELECT id FROM sessions WHERE id = ?1 AND deleted_at IS NULL",
            params![session_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        return Err("会话不存在或已删除".into());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = db::now();
    let tools_json = if tools.is_empty() {
        None
    } else {
        Some(serde_json::to_string(tools).map_err(|e| e.to_string())?)
    };
    conn.execute(
        "INSERT INTO messages(id, session_id, role, content, tools, created_at, stopped)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, session_id, role, content, tools_json, now, if stopped { 1 } else { 0 }],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE sessions SET updated_at = ?1 WHERE id = ?2",
        params![now, session_id],
    )
    .map_err(|e| e.to_string())?;

    // 首条用户消息自动作为标题
    if role == "user" {
        let auto: Option<i64> = conn
            .query_row(
                "SELECT auto_title FROM sessions WHERE id = ?1",
                params![session_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if auto == Some(1) {
            let title = derive_title(content);
            conn.execute(
                "UPDATE sessions SET title = ?1 WHERE id = ?2",
                params![title, session_id],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    Ok(ChatMessage {
        id,
        role: role.to_string(),
        text: content.to_string(),
        tools: tools.to_vec(),
        timestamp: Some(now),
        stopped,
    })
}

pub fn get_claude_session_id(conn: &Connection, session_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT claude_session_id FROM sessions WHERE id = ?1",
        params![session_id],
        |r| r.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
}

pub fn set_claude_session_id(conn: &Connection, session_id: &str, cid: &str) {
    let _ = conn.execute(
        "UPDATE sessions SET claude_session_id = ?1 WHERE id = ?2",
        params![cid, session_id],
    );
}

pub fn set_context_usage(conn: &Connection, session_id: &str, tokens: i64, max: i64, pct: f64) {
    let _ = conn.execute(
        "UPDATE sessions SET context_tokens = ?1, context_max = ?2, context_percent = ?3
          WHERE id = ?4",
        params![tokens, max, pct, session_id],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        for (_, sql) in crate::db::migrations_for_test() {
            c.execute_batch(sql).unwrap();
        }
        c
    }

    #[test]
    fn title_truncates_by_char() {
        let s = "一".repeat(50);
        let t = derive_title(&s);
        assert_eq!(t.chars().count(), 41, "40 字 + 省略号");
    }

    #[test]
    fn title_blank() {
        assert_eq!(derive_title("   "), "(新会话)");
    }

    #[test]
    fn workspace_and_session_crud() {
        let c = mem();
        let dir = std::env::temp_dir().join(format!("cw-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let w = add_workspace(&c, &dir.to_string_lossy(), "测试").unwrap();
        assert_eq!(w.name, "测试");

        let s = create_session(&c, &w.id).unwrap();
        let list = list_sessions(&c).unwrap();
        assert_eq!(list.len(), 1);

        // 首条用户消息自动成标题
        append_message(&c, &s.id, "user", "帮我改代码", &[], false).unwrap();
        let list = list_sessions(&c).unwrap();
        assert_eq!(list[0].title, "帮我改代码");

        delete_session(&c, &s.id).unwrap();
        assert_eq!(list_sessions(&c).unwrap().len(), 0);
        assert_eq!(list_deleted_sessions(&c).unwrap().len(), 1);

        restore_session(&c, &s.id).unwrap();
        assert_eq!(list_sessions(&c).unwrap().len(), 1);

        purge_session(&c, &s.id).unwrap();
        assert_eq!(list_deleted_sessions(&c).unwrap().len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn append_to_deleted_session_rejected() {
        let c = mem();
        let dir = std::env::temp_dir().join(format!("cw-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let w = add_workspace(&c, &dir.to_string_lossy(), "t").unwrap();
        let s = create_session(&c, &w.id).unwrap();
        delete_session(&c, &s.id).unwrap();
        assert!(append_message(&c, &s.id, "user", "x", &[], false).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicate_copies_messages() {
        let c = mem();
        let dir = std::env::temp_dir().join(format!("cw-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let w = add_workspace(&c, &dir.to_string_lossy(), "t").unwrap();
        let s = create_session(&c, &w.id).unwrap();
        append_message(&c, &s.id, "user", "第一句", &[], false).unwrap();
        append_message(&c, &s.id, "assistant", "回答", &[], false).unwrap();
        let copy = duplicate_session(&c, &s.id).unwrap();
        assert_eq!(count_messages(&c, &copy.id).unwrap(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
