//! ~/.claude/settings.json 的读写。
//!
//! 语义对齐 Node 版 services/claudeSettings.ts：
//! 备份 → 临时文件 → rename，写前校验 mtime，文件损坏时拒绝覆盖。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// 模型字段实际存在 env 里。
pub const MODEL_KEYS: &[(&str, &str)] = &[
    ("ANTHROPIC_MODEL", "主模型"),
    ("ANTHROPIC_DEFAULT_OPUS_MODEL", "Opus 映射"),
    ("ANTHROPIC_DEFAULT_SONNET_MODEL", "Sonnet 映射"),
    ("ANTHROPIC_DEFAULT_HAIKU_MODEL", "Haiku 映射"),
    ("CLAUDE_CODE_EFFORT_LEVEL", "推理强度"),
    ("MAX_THINKING_TOKENS", "思考预算"),
];

const MAX_BACKUPS: usize = 20;

/// 最近一次读取是否因文件损坏而降级，供界面提示。
static LAST_READ_ERROR: Mutex<Option<String>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvEntry {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    pub default_mode: String,
    pub allow: Vec<String>,
    pub deny: Vec<String>,
}

pub fn claude_dir() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)?;
    Some(home.join(".claude"))
}

pub fn settings_path() -> Option<PathBuf> {
    claude_dir().map(|d| d.join("settings.json"))
}

pub fn backups_dir() -> Option<PathBuf> {
    claude_dir().map(|d| d.join("backups"))
}

/// 读取原始 JSON。文件损坏时返回空对象并记录错误，让界面仍能打开。
pub fn read_raw() -> Result<Map<String, Value>, String> {
    let Some(p) = settings_path() else {
        return Ok(Map::new());
    };
    let Ok(text) = fs::read_to_string(&p) else {
        *LAST_READ_ERROR.lock().map_err(|_| "锁中毒")? = None;
        return Ok(Map::new());
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) => {
            *LAST_READ_ERROR.lock().map_err(|_| "锁中毒")? = None;
            Ok(m)
        }
        _ => {
            *LAST_READ_ERROR.lock().map_err(|_| "锁中毒")? = Some(
                "settings.json 无法解析，配置以空值展示；修复后请重启应用".into(),
            );
            Ok(Map::new())
        }
    }
}

pub fn last_read_error() -> Option<String> {
    LAST_READ_ERROR.lock().ok().and_then(|g| g.clone())
}

/// 文件 mtime（毫秒），用于并发冲突检测。
fn stat_mtime() -> Option<u128> {
    let p = settings_path()?;
    let meta = fs::metadata(p).ok()?;
    meta.modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_millis())
}

/// 备份 → 临时文件（0600）→ rename。
pub fn write_settings(next: &Map<String, Value>, expected_mtime: Option<u128>) -> Result<(), String> {
    // 文件当前无法解析时绝不能写：
    // read_raw 返回空对象，保存会用 {...raw, env} 覆盖整份文件，
    // hooks / permissions / statusLine 等会被全部抹掉。
    if last_read_error().is_some() {
        return Err(
            "settings.json 当前无法解析，为避免覆盖其它配置已阻止保存。请先修复该文件后重启应用".into(),
        );
    }

    if let Some(expected) = expected_mtime {
        if let Some(current) = stat_mtime() {
            let diff = current.abs_diff(expected);
            if diff > 1 {
                return Err("文件已被其它进程修改，请重新加载后再保存".into());
            }
        }
    }

    let Some(p) = settings_path() else {
        return Err("无法定位 ~/.claude".into());
    };
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("创建目录失败: {e}"))?;
    }

    // 备份
    if p.exists() {
        if let Some(bdir) = backups_dir() {
            let _ = fs::create_dir_all(&bdir);
            let stamp = crate::db::now().replace([':', '.'], "-");
            let _ = fs::copy(&p, bdir.join(format!("settings-{stamp}.json")));
            prune_backups(&bdir);
        }
    }

    let body = serde_json::to_string_pretty(&Value::Object(next.clone()))
        .map_err(|e| format!("序列化失败: {e}"))?;
    let tmp = p.with_extension(format!("json.tmp-{}", uuid::Uuid::new_v4()));
    fs::write(&tmp, body).map_err(|e| format!("写临时文件失败: {e}"))?;
    // 显式收紧权限：rename 会保留临时文件的权限，
    // 默认权限会让同机其它用户读到里面的 API Token
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600));
    }
    fs::rename(&tmp, &p).map_err(|e| format!("替换 settings.json 失败: {e}"))?;
    Ok(())
}

fn prune_backups(dir: &std::path::Path) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut files: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("settings-") && n.ends_with(".json"))
        })
        .collect();
    files.sort();
    let excess = files.len().saturating_sub(MAX_BACKUPS);
    for f in files.iter().take(excess) {
        let _ = fs::remove_file(f);
    }
}

fn env_object(raw: &Map<String, Value>) -> Map<String, Value> {
    match raw.get("env") {
        Some(Value::Object(m)) => m.clone(),
        _ => Map::new(),
    }
}

pub fn read_env() -> Result<Vec<EnvEntry>, String> {
    let raw = read_raw()?;
    let mut out: Vec<EnvEntry> = env_object(&raw)
        .into_iter()
        .map(|(k, v)| EnvEntry {
            key: k,
            value: match v {
                Value::String(s) => s,
                other => other.to_string(),
            },
        })
        .collect();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

/// 校验环境变量。返回错误信息，合法时 None。
pub fn validate_env(entries: &[EnvEntry]) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    for e in entries {
        let key = e.key.trim();
        if key.is_empty() {
            return Some("键名不能为空".into());
        }
        // 只允许字母/下划线开头，后面字母数字下划线
        let mut cs = key.chars();
        let first_ok = cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
        let rest_ok = cs.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !first_ok || !rest_ok {
            return Some(format!(
                "键名不合法：{key}（只允许字母、数字、下划线，且不能以数字开头）"
            ));
        }
        if matches!(key, "__proto__" | "constructor" | "prototype") {
            return Some(format!("键名不允许使用保留名：{key}"));
        }
        if !seen.insert(key.to_string()) {
            return Some(format!("键名重复：{key}"));
        }
        // NUL 会让 CLI 的 spawn 直接失败
        if e.value.contains(['\r', '\n', '\0']) {
            return Some(format!("值不能包含换行或空字符：{key}"));
        }
    }
    None
}

pub fn save_env(entries: &[EnvEntry]) -> Result<(), String> {
    if let Some(bad) = validate_env(entries) {
        return Err(bad);
    }
    let mtime = stat_mtime();
    let mut raw = read_raw()?;
    let mut env = Map::new();
    for e in entries {
        env.insert(e.key.trim().to_string(), Value::String(e.value.clone()));
    }
    raw.insert("env".into(), Value::Object(env));
    write_settings(&raw, mtime)
}

pub fn read_permissions() -> Result<Permissions, String> {
    let raw = read_raw()?;
    let p = raw.get("permissions").and_then(|v| v.as_object());
    let str_list = |v: Option<&Value>| -> Vec<String> {
        v.and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    };
    Ok(Permissions {
        default_mode: p
            .and_then(|p| p.get("defaultMode"))
            .and_then(|v| v.as_str())
            .unwrap_or("default")
            .to_string(),
        allow: str_list(p.and_then(|p| p.get("allow"))),
        deny: str_list(p.and_then(|p| p.get("deny"))),
    })
}

pub fn save_permissions(next: &Permissions) -> Result<(), String> {
    let mtime = stat_mtime();
    let mut raw = read_raw()?;
    raw.insert(
        "permissions".into(),
        serde_json::json!({
            "defaultMode": next.default_mode,
            "allow": next.allow,
            "deny": next.deny,
        }),
    );
    write_settings(&raw, mtime)
}

pub fn read_models() -> Result<std::collections::HashMap<String, String>, String> {
    let env = env_object(&read_raw()?);
    let mut out = std::collections::HashMap::new();
    for (k, _) in MODEL_KEYS {
        if let Some(v) = env.get(*k) {
            out.insert(
                (*k).to_string(),
                v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()),
            );
        }
    }
    Ok(out)
}

pub fn save_models(models: &[(String, String)]) -> Result<(), String> {
    let mtime = stat_mtime();
    let mut raw = read_raw()?;
    let mut env = env_object(&raw);
    for (k, v) in models {
        if v.trim().is_empty() {
            env.remove(k);
        } else {
            env.insert(k.clone(), Value::String(v.clone()));
        }
    }
    raw.insert("env".into(), Value::Object(env));
    write_settings(&raw, mtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_validation_rejects_bad_keys() {
        let bad = |k: &str, v: &str| {
            validate_env(&[EnvEntry { key: k.into(), value: v.into() }])
        };
        assert!(bad("", "x").is_some(), "空键名应拒绝");
        assert!(bad("1ABC", "x").is_some(), "数字开头应拒绝");
        assert!(bad("A-B", "x").is_some(), "含连字符应拒绝");
        assert!(bad("__proto__", "x").is_some(), "保留名应拒绝");
        assert!(bad("OK_KEY", "a\nb").is_some(), "换行应拒绝");
        assert!(bad("OK_KEY", "v").is_none(), "合法键应通过");
    }

    #[test]
    fn env_validation_rejects_duplicates() {
        let err = validate_env(&[
            EnvEntry { key: "A".into(), value: "1".into() },
            EnvEntry { key: "A".into(), value: "2".into() },
        ]);
        assert!(err.unwrap().contains("重复"));
    }

    #[test]
    fn model_keys_cover_all_six() {
        assert_eq!(MODEL_KEYS.len(), 6);
    }
}
