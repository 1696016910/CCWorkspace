//! Skill 扫描与开关、插件与仓库管理。
//!
//! 对齐 Node 版 services/skills.ts、plugins.ts。

use crate::config;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillInfo {
    pub name: String,
    pub description: String,
    pub source: String,
    /// 只在 Skill 目录下的附属文件数（不含 SKILL.md）。删除时要用它提示连带影响。
    pub extra_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInfo {
    pub name: String,
    pub description: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
    pub source: String,
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

/// 扫描 ~/.claude/skills 与项目的 .claude/skills。
///
/// 开关已移除：原来的 toggle 只写应用自己的数据库，claude 子进程从不读取，
/// 开或关对实际行为毫无影响。保留它等于给用户一个假的开关。
pub fn list_skills(project_dir: Option<&str>) -> Vec<SkillInfo> {
    let Some(dir) = config::claude_dir() else {
        return vec![];
    };
    let mut out = scan_skill_dir(&dir.join("skills"), "user");
    if let Some(proj) = project_dir {
        out.extend(scan_skill_dir(
            &PathBuf::from(proj).join(".claude").join("skills"),
            "project",
        ));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn scan_skill_dir(dir: &std::path::Path, source: &str) -> Vec<SkillInfo> {
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
            extra_files: count_extra_files(&e.path()),
        });
    }
    out
}

/// 目录里除 SKILL.md 外的条目数。删除前要告诉用户会连带删掉多少东西。
fn count_extra_files(dir: &std::path::Path) -> usize {
    fs::read_dir(dir)
        .map(|rd| rd.flatten().filter(|e| e.file_name() != "SKILL.md").count())
        .unwrap_or(0)
}

/// 名称合法性：不允许路径分隔符，避免 `../` 逃出目标目录。
fn safe_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
}

/// Skill 的落地目录。用户级走 ~/.claude/skills，项目级走 <工作区>/.claude/skills。
fn skill_dir(name: &str, source: &str, project_dir: Option<&str>) -> Result<PathBuf, String> {
    if !safe_name(name) {
        return Err("Skill 名称不合法".into());
    }
    match source {
        "project" => {
            let proj = project_dir.ok_or("缺少工作区目录")?;
            Ok(PathBuf::from(proj).join(".claude").join("skills").join(name))
        }
        _ => {
            let dir = config::claude_dir().ok_or("找不到 ~/.claude 目录")?;
            Ok(dir.join("skills").join(name))
        }
    }
}

pub fn read_skill(name: &str, source: &str, project_dir: Option<&str>) -> Result<String, String> {
    let dir = skill_dir(name, source, project_dir)?;
    fs::read_to_string(dir.join("SKILL.md")).map_err(|_| format!("读不到 Skill「{name}」的内容"))
}

/// 写入 Skill 正文。目录不存在时一并创建。
pub fn write_skill(
    name: &str,
    source: &str,
    project_dir: Option<&str>,
    content: &str,
) -> Result<(), String> {
    let dir = skill_dir(name, source, project_dir)?;
    fs::create_dir_all(&dir).map_err(|e| format!("创建 Skill 目录失败: {e}"))?;
    fs::write(dir.join("SKILL.md"), content).map_err(|e| format!("写入 Skill 失败: {e}"))
}

pub fn delete_skill(name: &str, source: &str, project_dir: Option<&str>) -> Result<(), String> {
    let dir = skill_dir(name, source, project_dir)?;
    if !dir.exists() {
        return Err(format!("Skill「{name}」不存在"));
    }
    fs::remove_dir_all(&dir).map_err(|e| format!("删除 Skill 失败: {e}"))
}

// ── 规则 ──

/// 规则是散装 `.md`：`~/.claude/rules/<name>.md`，与项目的 `.claude/rules/`。
///
/// 与 Skill 不同：没有 `name` 字段（名字就是文件名），而且有些文件压根没有 frontmatter。
pub fn list_rules(project_dir: Option<&str>) -> Vec<RuleInfo> {
    let mut out = vec![];
    if let Some(dir) = config::claude_dir() {
        out.extend(scan_rule_dir(&dir.join("rules"), "user"));
    }
    if let Some(proj) = project_dir {
        out.extend(scan_rule_dir(
            &PathBuf::from(proj).join(".claude").join("rules"),
            "project",
        ));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn scan_rule_dir(dir: &std::path::Path, source: &str) -> Vec<RuleInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut out = vec![];
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&p) else {
            continue;
        };
        out.push(RuleInfo {
            name: p
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            description: doc_description(&text),
            source: source.to_string(),
        });
    }
    out
}

/// 描述：优先 frontmatter 的 description，退回首行标题，再退回首段文字。
///
/// 规则与命令都没 `name` 字段，共用一个提取逻辑。
fn doc_description(text: &str) -> String {
    let (_, desc) = parse_frontmatter(text);
    if let Some(d) = desc.filter(|d| !d.is_empty()) {
        return d;
    }
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t == "---" {
            continue;
        }
        let t = t.trim_start_matches('#').trim();
        if !t.is_empty() {
            return t.chars().take(120).collect();
        }
    }
    String::new()
}

fn rule_path(name: &str, source: &str, project_dir: Option<&str>) -> Result<PathBuf, String> {
    if !safe_name(name) {
        return Err("规则名称不合法".into());
    }
    let base = match source {
        "project" => {
            let proj = project_dir.ok_or("缺少工作区目录")?;
            PathBuf::from(proj).join(".claude").join("rules")
        }
        _ => config::claude_dir()
            .ok_or("找不到 ~/.claude 目录")?
            .join("rules"),
    };
    Ok(base.join(format!("{name}.md")))
}

pub fn read_rule(name: &str, source: &str, project_dir: Option<&str>) -> Result<String, String> {
    let p = rule_path(name, source, project_dir)?;
    fs::read_to_string(&p).map_err(|_| format!("读不到规则「{name}」的内容"))
}

pub fn write_rule(
    name: &str,
    source: &str,
    project_dir: Option<&str>,
    content: &str,
) -> Result<(), String> {
    let p = rule_path(name, source, project_dir)?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建规则目录失败: {e}"))?;
    }
    fs::write(&p, content).map_err(|e| format!("写入规则失败: {e}"))
}

pub fn delete_rule(name: &str, source: &str, project_dir: Option<&str>) -> Result<(), String> {
    let p = rule_path(name, source, project_dir)?;
    if !p.exists() {
        return Err(format!("规则「{name}」不存在"));
    }
    fs::remove_file(&p).map_err(|e| format!("删除规则失败: {e}"))
}

// ── 命令 ──

/// 用户自定义命令：`~/.claude/commands/**/*.md` 与项目的 `.claude/commands/`。
///
/// 子目录里的命令以 `dir:name` 形式被调用，名字里要保留这个前缀。
pub fn list_commands(project_dir: Option<&str>) -> Vec<CommandInfo> {
    let mut out = vec![];
    if let Some(dir) = config::claude_dir() {
        out.extend(scan_command_dir(&dir.join("commands"), "", "user"));
    }
    if let Some(proj) = project_dir {
        let base = PathBuf::from(proj).join(".claude").join("commands");
        out.extend(scan_command_dir(&base, "", "project"));
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn scan_command_dir(dir: &std::path::Path, prefix: &str, source: &str) -> Vec<CommandInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut out = vec![];
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            let sub = e.file_name().to_string_lossy().to_string();
            let nested = if prefix.is_empty() {
                sub
            } else {
                format!("{prefix}:{sub}")
            };
            out.extend(scan_command_dir(&p, &nested, source));
            continue;
        }
        if p.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&p) else {
            continue;
        };
        let stem = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let name = if prefix.is_empty() {
            stem
        } else {
            format!("{prefix}:{stem}")
        };
        out.push(CommandInfo {
            name,
            description: doc_description(&text),
            source: source.to_string(),
        });
    }
    out
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
