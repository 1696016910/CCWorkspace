//! Skill 扫描与开关、插件与仓库管理。
//!
//! 对齐 Node 版 services/skills.ts、plugins.ts。

use crate::config;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub source: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub marketplace: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceInfo {
    pub name: String,
    pub source: String,
    pub builtin: bool,
}

/// 解析 SKILL.md 的 frontmatter，取 name / description。
fn parse_frontmatter(text: &str) -> (Option<String>, Option<String>) {
    let mut lines = text.lines();
    // 首行必须是 ---
    if lines.next().map(str::trim) != Some("---") {
        return (None, None);
    }
    let mut name = None;
    let mut desc = None;
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        let Some(idx) = line.find(':') else { continue };
        if idx == 0 {
            continue;
        }
        let key = line[..idx].trim();
        let raw = line[idx + 1..].trim();
        let value = raw.trim_matches(|c| c == '"' || c == '\'');
        // `description: >` / `|` 是多行写法，标识符本身不是内容
        if value.is_empty() || value == ">" || value == "|" {
            continue;
        }
        match key {
            "name" => name = Some(value.to_string()),
            "description" => desc = Some(value.to_string()),
            _ => {}
        }
    }
    (name, desc)
}

/// 禁用名单存在数据库 meta 表，与 Node 版同一位置——三版互认。
fn disabled_skills() -> std::collections::HashSet<String> {
    let raw = crate::db::with(|c| {
        let v: Option<String> = c
            .query_row(
                "SELECT value FROM meta WHERE key = 'disabled_skills'",
                [],
                |r| r.get(0),
            )
            .ok();
        Ok(v)
    })
    .ok()
    .flatten();
    raw.and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .map(|v| v.into_iter().collect())
        .unwrap_or_default()
}

/// 扫描 ~/.claude/skills，只列全局 Skill。
pub fn list_skills(project_dir: Option<&str>) -> Vec<SkillInfo> {
    let Some(dir) = config::claude_dir() else {
        return vec![];
    };
    let disabled = disabled_skills();
    let mut out = scan_dir(&dir.join("skills"), "user");
    if let Some(proj) = project_dir {
        out.extend(scan_dir(&PathBuf::from(proj).join(".claude").join("skills"), "project"));
    }
    for s in &mut out {
        s.enabled = !disabled.contains(&s.name);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn scan_dir(dir: &std::path::Path, source: &str) -> Vec<SkillInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut out = vec![];
    for e in entries.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let Ok(text) = fs::read_to_string(e.path().join("SKILL.md")) else {
            continue;
        };
        let (name, desc) = parse_frontmatter(&text);
        let fallback = e.file_name().to_string_lossy().to_string();
        out.push(SkillInfo {
            name: name.unwrap_or(fallback),
            description: desc.unwrap_or_default(),
            source: source.to_string(),
            enabled: true,
        });
    }
    out
}

pub fn toggle_skill(name: &str, enabled: bool) -> Result<(), String> {
    let mut set = disabled_skills();
    if enabled {
        set.remove(name);
    } else {
        set.insert(name.to_string());
    }
    let mut list: Vec<String> = set.into_iter().collect();
    list.sort();
    let body = serde_json::to_string(&list).map_err(|e| e.to_string())?;
    crate::db::with(|c| {
        c.execute(
            "INSERT INTO meta(key, value) VALUES('disabled_skills', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [body],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    })
}

/// 供会话启动时使用的已启用 Skill 名单。
#[allow(dead_code)]
pub fn enabled_skill_names() -> Vec<String> {
    list_skills(None)
        .into_iter()
        .filter(|s| s.enabled)
        .map(|s| s.name)
        .collect()
}

// ── 插件 ──

fn split_id(id: &str) -> (String, String) {
    match id.rfind('@') {
        Some(at) => (id[..at].to_string(), id[at + 1..].to_string()),
        None => (id.to_string(), "unknown".to_string()),
    }
}

/// 列出已安装插件，CLI 不可用时退化为读 settings.json。
pub fn list_plugins() -> Vec<PluginInfo> {
    if let Ok(stdout) = run_claude(&["plugin", "list", "--json"]) {
        if let Ok(arr) = serde_json::from_str::<Vec<Value>>(&stdout) {
            return arr
                .into_iter()
                .filter_map(|v| {
                    let id = v.get("id")?.as_str()?.to_string();
                    let (name, marketplace) = split_id(&id);
                    Some(PluginInfo {
                        version: v
                            .get("version")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        enabled: v.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true),
                        id,
                        name,
                        marketplace,
                    })
                })
                .collect();
        }
    }
    // 降级：从 enabledPlugins 推导
    let Ok(raw) = config::read_raw() else {
        return vec![];
    };
    let Some(map) = raw.get("enabledPlugins").and_then(|v| v.as_object()) else {
        return vec![];
    };
    map.iter()
        .map(|(id, val)| {
            let (name, marketplace) = split_id(id);
            PluginInfo {
                id: id.clone(),
                name,
                version: String::new(),
                marketplace,
                enabled: val.as_bool().unwrap_or(false),
            }
        })
        .collect()
}

pub fn toggle_plugin(id: &str, enabled: bool) -> Result<(), String> {
    let mtime = None;
    let mut raw = config::read_raw()?;
    let mut known = raw
        .get("enabledPlugins")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    known.insert(id.to_string(), Value::Bool(enabled));
    raw.insert("enabledPlugins".into(), Value::Object(known));
    config::write_settings(&raw, mtime)
}

pub fn install_plugin(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("插件 ID 不能为空".into());
    }
    run_claude(&["plugin", "install", id]).map(|_| ())
}

pub fn uninstall_plugin(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("插件 ID 不能为空".into());
    }
    run_claude(&["plugin", "uninstall", id]).map(|_| ())
}

// ── 插件仓库 ──

pub fn list_marketplaces() -> Vec<MarketplaceInfo> {
    let mut out = vec![MarketplaceInfo {
        name: "claude-plugins-official".into(),
        source: "github.com/anthropics/claude-plugins".into(),
        builtin: true,
    }];
    let Ok(raw) = config::read_raw() else {
        return out;
    };
    let Some(known) = raw.get("extraKnownMarketplaces").and_then(|v| v.as_object()) else {
        return out;
    };
    for (name, v) in known {
        let source = v
            .get("source")
            .and_then(|s| s.get("repo").or_else(|| s.get("url")))
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        out.push(MarketplaceInfo {
            name: name.clone(),
            source,
            builtin: false,
        });
    }
    out
}

pub fn add_marketplace(name: &str, repo: &str) -> Result<(), String> {
    let (name, repo) = (name.trim(), repo.trim());
    if name.is_empty() || repo.is_empty() {
        return Err("仓库名称与地址都不能为空".into());
    }
    let mut raw = config::read_raw()?;
    let mut known = raw
        .get("extraKnownMarketplaces")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    known.insert(
        name.to_string(),
        serde_json::json!({ "source": { "source": "github", "repo": repo } }),
    );
    raw.insert("extraKnownMarketplaces".into(), Value::Object(known));
    config::write_settings(&raw, None)
}

pub fn remove_marketplace(name: &str) -> Result<(), String> {
    if name == "claude-plugins-official" {
        return Err("内置仓库不可移除".into());
    }
    let mut raw = config::read_raw()?;
    let Some(mut known) = raw
        .get("extraKnownMarketplaces")
        .and_then(|v| v.as_object())
        .cloned()
    else {
        return Ok(());
    };
    known.remove(name);
    raw.insert("extraKnownMarketplaces".into(), Value::Object(known));
    config::write_settings(&raw, None)
}

fn run_claude(args: &[&str]) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd.exe");
        c.arg("/d").arg("/s").arg("/c").arg("claude");
        c.args(args);
        c
    };
    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = std::process::Command::new("claude");
        c.args(args);
        c
    };
    let out = cmd
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("调用 claude 命令失败：{e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontmatter_parsed() {
        let (n, d) = parse_frontmatter("---\nname: my-skill\ndescription: 做什么的\n---\n正文");
        assert_eq!(n.as_deref(), Some("my-skill"));
        assert_eq!(d.as_deref(), Some("做什么的"));
    }

    #[test]
    fn frontmatter_missing_falls_back() {
        let (n, d) = parse_frontmatter("# 普通 Markdown\n正文");
        assert!(n.is_none() && d.is_none());
    }

    #[test]
    fn frontmatter_strips_quotes() {
        let (n, _) = parse_frontmatter("---\nname: \"quoted-name\"\n---\n");
        assert_eq!(n.as_deref(), Some("quoted-name"));
    }

    #[test]
    fn multiline_description_not_taken_as_marker() {
        let (_, d) = parse_frontmatter("---\nname: x\ndescription: >\n  很长\n---\n");
        assert_ne!(d.as_deref(), Some(">"));
    }

    #[test]
    fn split_id_handles_at() {
        assert_eq!(split_id("name@market"), ("name".into(), "market".into()));
        assert_eq!(split_id("plain"), ("plain".into(), "unknown".into()));
    }

    #[test]
    fn builtin_marketplace_present() {
        let list = list_marketplaces();
        assert!(list.iter().any(|m| m.builtin));
    }
}
