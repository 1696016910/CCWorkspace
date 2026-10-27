//! 工作区、会话、配置、Skill 的业务逻辑。
//!
//! 与 Node 版的 services/*.ts 行为一致，读写同一个数据库。

use crate::db;
use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::PathBuf;

const TITLE_MAX: usize = 40;
pub const PAGE_SIZE: usize = 80;

/// 从首条消息生成标题。按字符截断，不会把 emoji 切开。
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

pub fn list_workspaces(conn: &Connection) -> Vec<Workspace> {
    let mut stmt = match conn.prepare(
        "SELECT id, path, name, created_at, pinned_at FROM workspaces
          ORDER BY pinned_at IS NULL, pinned_at DESC, created_at ASC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let rows = stmt.query_map([], |r| {
        Ok(Workspace {
            id: r.get(0)?,
            path: r.get(1)?,
            name: r.get(2)?,
            added_at: r.get(3)?,
            pinned_at: r.get(4)?,
        })
    });
    rows.map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default()
}

pub fn add_workspace(conn: &Connection, dir: &str, custom_name: &str) -> Result<(), String> {
    let dir = dir.trim();
    if dir.is_empty() {
        return Err("请选择一个目录".into());
    }
    let abs = fs::canonicalize(dir).map_err(|_| format!("目录不存在或不可读：{dir}"))?;
    if !abs.is_dir() {
        return Err(format!("目录不存在或不可读：{}", abs.display()));
    }
    let abs_str = abs.to_string_lossy().trim_start_matches(r"\\?\").to_string();

    // 大小写不敏感地查重：Windows/macOS 上同一目录可能大小写不同
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM workspaces WHERE path = ?1 COLLATE NOCASE",
            params![abs_str],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if existing.is_some() {
        return Err("该工作区已存在".into());
    }

    let name = if custom_name.trim().is_empty() {
        abs.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
    } else {
        custom_name.trim().to_string()
    };

    conn.execute(
        "INSERT INTO workspaces(id, path, name, created_at) VALUES(?1, ?2, ?3, ?4)",
        params![uuid::Uuid::new_v4().to_string(), abs_str, name, db::now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove_workspace(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM workspaces WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn rename_workspace(conn: &Connection, id: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("名称不能为空".into());
    }
    conn.execute("UPDATE workspaces SET name = ?1 WHERE id = ?2", params![name, id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn pin_workspace(conn: &Connection, id: &str, pinned: bool) -> Result<(), String> {
    let exists: Option<String> = conn
        .query_row("SELECT id FROM workspaces WHERE id = ?1", params![id], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    if exists.is_none() {
        return Err("工作区不存在".into());
    }
    let at: Option<String> = if pinned { Some(db::now()) } else { None };
    conn.execute("UPDATE workspaces SET pinned_at = ?1 WHERE id = ?2", params![at, id])
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
        pinned_at: r.get(11)?,
    })
}

pub fn list_sessions(conn: &Connection) -> Vec<SessionSummary> {
    let sql = format!(
        "SELECT {SESSION_COLS}, COUNT(m.id) AS message_count
           FROM sessions s
           LEFT JOIN messages m ON m.session_id = s.id
          WHERE s.deleted_at IS NULL
          GROUP BY s.id
          ORDER BY s.pinned_at IS NULL, s.pinned_at DESC, s.updated_at DESC"
    );
    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let rows = stmt.query_map([], |r| map_session(r));
    rows.map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default()
}

pub fn create_session(conn: &Connection, workspace_id: &str) -> Result<SessionSummary, String> {
    if workspace_id.trim().is_empty() {
        return Err("请先选择一个工作区".into());
    }
    let exists: Option<String> = conn
        .query_row("SELECT id FROM workspaces WHERE id = ?1", params![workspace_id], |r| r.get(0))
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
        .execute("UPDATE sessions SET deleted_at = ?1 WHERE id = ?2", params![db::now(), id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("会话不存在".into());
    }
    Ok(())
}

pub fn pin_session(conn: &Connection, id: &str, pinned: bool) -> Result<(), String> {
    let at: Option<String> = if pinned { Some(db::now()) } else { None };
    conn.execute("UPDATE sessions SET pinned_at = ?1 WHERE id = ?2", params![at, id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 追加消息。会话已删除时返回错误，不静默丢弃。
pub fn append_message(
    conn: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
    tools: &[ToolCall],
    stopped: bool,
) -> Result<ChatMessage, String> {
    let deleted: Option<Option<String>> = conn
        .query_row("SELECT deleted_at FROM sessions WHERE id = ?1", params![session_id], |r| r.get(0))
        .optional()
        .map_err(|e| e.to_string())?;
    match deleted {
        None => return Err("会话不存在".into()),
        Some(Some(_)) => return Err("会话已被删除，消息未保存".into()),
        Some(None) => {}
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
    conn.execute("UPDATE sessions SET updated_at = ?1 WHERE id = ?2", params![now, session_id])
        .map_err(|e| e.to_string())?;

    if role == "user" {
        let auto: Option<(i64, String)> = conn
            .query_row(
                "SELECT auto_title, title FROM sessions WHERE id = ?1",
                params![session_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some((auto_title, title)) = auto {
            if auto_title == 1 && title.is_empty() {
                conn.execute(
                    "UPDATE sessions SET title = ?1 WHERE id = ?2",
                    params![derive_title(content), session_id],
                )
                .map_err(|e| e.to_string())?;
            }
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

pub fn list_messages(conn: &Connection, session_id: &str, limit: usize) -> Vec<ChatMessage> {
    let sql = if limit > 0 {
        format!(
            "SELECT * FROM (
               SELECT id, role, content, tools, created_at, stopped FROM messages
                WHERE session_id = ?1 ORDER BY created_at DESC LIMIT {limit}
             ) ORDER BY created_at ASC"
        )
    } else {
        "SELECT id, role, content, tools, created_at, stopped FROM messages
          WHERE session_id = ?1 ORDER BY created_at ASC"
            .to_string()
    };

    let mut stmt = match conn.prepare(&sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    let rows = stmt.query_map(params![session_id], |r| {
        let tools_json: Option<String> = r.get(3)?;
        let stopped: i64 = r.get(5)?;
        Ok(ChatMessage {
            id: r.get(0)?,
            role: r.get(1)?,
            text: r.get(2)?,
            tools: tools_json
                .and_then(|s| serde_json::from_str::<Vec<ToolCall>>(&s).ok())
                .unwrap_or_default(),
            timestamp: r.get(4)?,
            stopped: stopped == 1,
        })
    });
    rows.map(|it| it.filter_map(Result::ok).collect()).unwrap_or_default()
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

// ── 配置 ──

pub fn claude_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)?;
    Some(home.join(".claude"))
}

pub fn settings_path() -> Option<PathBuf> {
    claude_dir().map(|d| d.join("settings.json"))
}

/// 读取 settings.json。解析失败返回 Err，调用方必须据此拒绝写入。
pub fn read_raw_settings() -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let Some(p) = settings_path() else {
        return Ok(serde_json::Map::new());
    };
    if !p.exists() {
        return Ok(serde_json::Map::new());
    }
    let text = fs::read_to_string(&p).map_err(|e| format!("读取 settings.json 失败: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "settings.json 无法解析，请先修复该文件".to_string())?;
    Ok(v.as_object().cloned().unwrap_or_default())
}

/// 安全写入：备份 → 临时文件 → rename。
pub fn write_raw_settings(next: &serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
    let Some(p) = settings_path() else {
        return Err("无法定位 settings.json".into());
    };

    // 先备份
    if p.exists() {
        if let Some(dir) = claude_dir() {
            let backup_dir = dir.join("backups");
            if fs::create_dir_all(&backup_dir).is_ok() {
                if let Ok(orig) = fs::read(&p) {
                    let stamp = db::now().replace([':', '.'], "-");
                    let _ = fs::write(backup_dir.join(format!("settings-{stamp}.json")), orig);
                }
            }
        }
    }

    let body = serde_json::to_string_pretty(next).map_err(|e| e.to_string())?;
    let tmp = p.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, body).map_err(|e| format!("写入失败: {e}"))?;
    fs::rename(&tmp, &p).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("替换文件失败: {e}")
    })
}

/// 校验环境变量条目。返回 Some(错误) 表示不合法。
pub fn validate_env(entries: &[EnvEntry]) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    for e in entries {
        let k = e.key.trim();
        if k.is_empty() {
            return Some("键名不能为空".into());
        }
        let ok = !k.is_empty()
            && k.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !ok {
            return Some(format!("键名不合法：{k}（只允许字母、数字、下划线，且不能以数字开头）"));
        }
        if matches!(k, "__proto__" | "constructor" | "prototype") {
            return Some(format!("键名不允许使用保留名：{k}"));
        }
        if !seen.insert(k.to_string()) {
            return Some(format!("键名重复：{k}"));
        }
        if e.value.contains(['\r', '\n', '\0']) {
            return Some(format!("值不能包含换行或空字符：{k}"));
        }
    }
    None
}

pub fn read_env() -> Vec<EnvEntry> {
    let Ok(raw) = read_raw_settings() else {
        return vec![];
    };
    let Some(env) = raw.get("env").and_then(|v| v.as_object()) else {
        return vec![];
    };
    let mut out: Vec<EnvEntry> = env
        .iter()
        .filter_map(|(k, v)| {
            v.as_str().map(|s| EnvEntry { key: k.clone(), value: s.to_string() })
        })
        .collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

pub fn save_env(entries: &[EnvEntry]) -> Result<(), String> {
    if let Some(msg) = validate_env(entries) {
        return Err(msg);
    }
    let mut raw = read_raw_settings()?;
    let mut env = serde_json::Map::new();
    for e in entries {
        env.insert(e.key.trim().to_string(), serde_json::Value::String(e.value.clone()));
    }
    raw.insert("env".into(), serde_json::Value::Object(env));
    write_raw_settings(&raw)
}

pub fn read_models() -> std::collections::HashMap<String, String> {
    read_env().into_iter().map(|e| (e.key, e.value)).collect()
}

// ── Skill ──

/// 扫描 ~/.claude/skills。只列全局 Skill。
pub fn list_skills() -> Vec<SkillInfo> {
    let Some(dir) = claude_dir() else {
        return vec![];
    };
    let skills_dir = dir.join("skills");
    let Ok(entries) = fs::read_dir(&skills_dir) else {
        return vec![];
    };
    let disabled = disabled_skills();

    let mut out = vec![];
    for e in entries.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let Ok(content) = fs::read_to_string(e.path().join("SKILL.md")) else {
            continue;
        };
        let fallback = e.file_name().to_string_lossy().to_string();
        let (name, desc) = parse_frontmatter(&content, &fallback);
        out.push(SkillInfo {
            enabled: !disabled.contains(&name),
            name,
            description: desc,
            source: "user".into(),
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn toggle_skill(name: &str, enabled: bool) -> Result<(), String> {
    let Some(dir) = claude_dir() else {
        return Err("无法定位 ~/.claude".into());
    };
    let mut set = disabled_skills();
    if enabled {
        set.remove(name);
    } else {
        set.insert(name.to_string());
    }
    let mut list: Vec<String> = set.into_iter().collect();
    list.sort();
    let body = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    fs::write(dir.join("disabled-skills.json"), body).map_err(|e| e.to_string())
}

fn disabled_skills() -> std::collections::HashSet<String> {
    let Some(dir) = claude_dir() else {
        return Default::default();
    };
    let Ok(raw) = fs::read_to_string(dir.join("disabled-skills.json")) else {
        return Default::default();
    };
    serde_json::from_str::<Vec<String>>(&raw)
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

pub fn enabled_skill_names() -> Vec<String> {
    list_skills().into_iter().filter(|s| s.enabled).map(|s| s.name).collect()
}

/// 从 SKILL.md 的 frontmatter 取 name 与 description。
///
/// 只处理单行 `key: value`；多行写法退化为空，不把 `>` 当描述。
fn parse_frontmatter(content: &str, fallback: &str) -> (String, String) {
    let mut name = fallback.to_string();
    let mut desc = String::new();
    let mut lines = content.lines();
    if lines.next().map(str::trim) != Some("---") {
        return (name, desc);
    }
    for l in lines {
        let t = l.trim();
        if t == "---" {
            break;
        }
        if let Some(v) = t.strip_prefix("name:") {
            let s = v.trim();
            if !s.is_empty() && s != ">" && s != "|" {
                name = s.to_string();
            }
        }
        if let Some(v) = t.strip_prefix("description:") {
            let s = v.trim();
            if !s.is_empty() && s != ">" && s != "|" {
                desc = s.to_string();
            }
        }
    }
    (name, desc)
}

pub fn claude_settings_path_json() -> String {
    settings_path().map(|p| p.display().to_string()).unwrap_or_else(|| "(未知)".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_plain() {
        assert_eq!(derive_title("把订单接口改成幂等"), "把订单接口改成幂等");
    }

    #[test]
    fn title_multiline_collapsed() {
        assert_eq!(derive_title("第一行\n\n第二行"), "第一行 第二行");
    }

    #[test]
    fn title_blank() {
        assert_eq!(derive_title("   "), "(新会话)");
    }

    #[test]
    fn title_truncates_by_char() {
        let t = derive_title(&"一".repeat(60));
        assert_eq!(t.chars().count(), TITLE_MAX + 1);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn title_does_not_split_emoji() {
        let t = derive_title(&format!("{}😀😀😀", "a".repeat(39)));
        assert!(!t.contains('\u{FFFD}'), "代理对被切开: {t}");
    }

    #[test]
    fn env_validation() {
        assert!(validate_env(&[EnvEntry { key: "A".into(), value: "1".into() }]).is_none());
        assert!(validate_env(&[EnvEntry { key: "".into(), value: "".into() }]).is_some());
        assert!(validate_env(&[EnvEntry { key: "A B".into(), value: "".into() }]).is_some());
        assert!(validate_env(&[EnvEntry { key: "__proto__".into(), value: "".into() }]).is_some());
        assert!(validate_env(&[EnvEntry { key: "A".into(), value: "a\nb".into() }]).is_some());
        assert!(validate_env(&[EnvEntry { key: "A".into(), value: "a\0b".into() }]).is_some());
    }

    #[test]
    fn frontmatter_parsed() {
        let (n, d) = parse_frontmatter("---\nname: foo\ndescription: bar\n---\n正文", "dir");
        assert_eq!(n, "foo");
        assert_eq!(d, "bar");
    }

    #[test]
    fn frontmatter_missing_falls_back() {
        let (n, d) = parse_frontmatter("# 普通 Markdown", "dir-name");
        assert_eq!(n, "dir-name");
        assert_eq!(d, "");
    }
}
