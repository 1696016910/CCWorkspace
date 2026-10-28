//! Tauri 后端入口：数据库初始化 + 全部 IPC 命令。
//!
//! 命令名用下划线（tauri 约定），前端 api/ 模块封装成和 Node 版同名的函数。

mod claude;
mod config;
mod db;
mod service;
mod skills;

use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};

/// 一轮对话的中断标志 + runId，供 chat_interrupt 精确停下对应那轮。
struct ActiveRun {
    run_id: u64,
    stop: Arc<AtomicBool>,
}

#[derive(Default)]
struct AppState {
    run_seq: AtomicU64,
    active: Mutex<Option<ActiveRun>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChatEvent {
    #[serde(rename = "type")]
    kind: String,
    session_id: String,
    run_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    delta: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool: Option<service::ToolCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<service::ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interrupted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn emit(app: &tauri::AppHandle, ev: ChatEvent) {
    let _ = app.emit("chat:event", ev);
}

// ── 工作区 ──

#[tauri::command]
fn workspace_list() -> Result<Vec<service::Workspace>, String> {
    db::with(|c| service::list_workspaces(c))
}

#[tauri::command]
fn workspace_add(dir: String, name: String) -> Result<service::Workspace, String> {
    db::with(|c| service::add_workspace(c, &dir, &name))
}

#[tauri::command]
fn workspace_remove(id: String) -> Result<(), String> {
    db::with(|c| service::remove_workspace(c, &id))
}

#[tauri::command]
fn workspace_rename(id: String, name: String) -> Result<(), String> {
    db::with(|c| service::rename_workspace(c, &id, &name))
}

#[tauri::command]
fn workspace_pin(id: String, pinned: bool) -> Result<(), String> {
    db::with(|c| service::pin_workspace(c, &id, pinned))
}

// ── 会话 ──

#[tauri::command]
fn session_list() -> Result<Vec<service::SessionSummary>, String> {
    db::with(|c| service::list_sessions(c))
}

#[tauri::command]
fn session_list_deleted() -> Result<Vec<service::SessionSummary>, String> {
    db::with(|c| service::list_deleted_sessions(c))
}

#[tauri::command]
fn session_create(workspace_id: String) -> Result<service::SessionSummary, String> {
    db::with(|c| service::create_session(c, &workspace_id))
}

#[tauri::command]
fn session_rename(id: String, title: String) -> Result<(), String> {
    db::with(|c| service::rename_session(c, &id, &title))
}

#[tauri::command]
fn session_remove(id: String) -> Result<(), String> {
    db::with(|c| service::delete_session(c, &id))
}

#[tauri::command]
fn session_restore(id: String) -> Result<(), String> {
    db::with(|c| service::restore_session(c, &id))
}

#[tauri::command]
fn session_purge(id: String) -> Result<(), String> {
    db::with(|c| service::purge_session(c, &id))
}

#[tauri::command]
fn session_pin(id: String, pinned: bool) -> Result<(), String> {
    db::with(|c| service::pin_session(c, &id, pinned))
}

#[tauri::command]
fn session_duplicate(id: String) -> Result<service::SessionSummary, String> {
    db::with(|c| service::duplicate_session(c, &id))
}

#[tauri::command]
fn session_messages(id: String, limit: Option<i64>) -> Result<Vec<service::ChatMessage>, String> {
    db::with(|c| service::list_messages(c, &id, limit.unwrap_or(service::PAGE_SIZE)))
}

#[tauri::command]
fn session_message_count(id: String) -> Result<i64, String> {
    db::with(|c| service::count_messages(c, &id))
}

// ── 配置 ──

#[tauri::command]
fn config_read_env() -> Result<Vec<config::EnvEntry>, String> {
    config::read_env()
}

#[tauri::command]
fn config_save_env(entries: Vec<config::EnvEntry>) -> Result<(), String> {
    config::save_env(&entries)
}

#[tauri::command]
fn config_read_permissions() -> Result<config::Permissions, String> {
    config::read_permissions()
}

#[tauri::command]
fn config_save_permissions(p: config::Permissions) -> Result<(), String> {
    config::save_permissions(&p)
}

#[tauri::command]
fn config_read_models() -> Result<std::collections::HashMap<String, String>, String> {
    config::read_models()
}

#[tauri::command]
fn config_save_models(m: Vec<(String, String)>) -> Result<(), String> {
    config::save_models(&m)
}

#[tauri::command]
fn config_health() -> Option<String> {
    config::last_read_error()
}

// ── Skill / 插件 / 仓库 ──

#[tauri::command]
fn skill_list(project_dir: Option<String>) -> Vec<skills::SkillInfo> {
    skills::list_skills(project_dir.as_deref())
}

#[tauri::command]
fn skill_toggle(name: String, enabled: bool) -> Result<Vec<skills::SkillInfo>, String> {
    skills::toggle_skill(&name, enabled)?;
    Ok(skills::list_skills(None))
}

#[tauri::command]
fn plugin_list() -> Vec<skills::PluginInfo> {
    skills::list_plugins()
}

#[tauri::command]
fn plugin_toggle(id: String, enabled: bool) -> Result<Vec<skills::PluginInfo>, String> {
    skills::toggle_plugin(&id, enabled)?;
    Ok(skills::list_plugins())
}

#[tauri::command]
fn plugin_install(id: String) -> Result<(), String> {
    skills::install_plugin(&id)
}

#[tauri::command]
fn plugin_uninstall(id: String) -> Result<(), String> {
    skills::uninstall_plugin(&id)
}

#[tauri::command]
fn marketplace_list() -> Vec<skills::MarketplaceInfo> {
    skills::list_marketplaces()
}

#[tauri::command]
fn marketplace_add(name: String, repo: String) -> Result<Vec<skills::MarketplaceInfo>, String> {
    skills::add_marketplace(&name, &repo)?;
    Ok(skills::list_marketplaces())
}

#[tauri::command]
fn marketplace_remove(name: String) -> Result<Vec<skills::MarketplaceInfo>, String> {
    skills::remove_marketplace(&name)?;
    Ok(skills::list_marketplaces())
}

// ── 对话 ──

/// 发一轮对话。立即返回 runId，内容通过 chat:event 推送。
#[tauri::command]
fn chat_send(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    text: String,
) -> Result<u64, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("消息不能为空".into());
    }

    // 找到会话所属工作区作为 cwd
    let (cwd, resume, _model, _env) = db::with(|c| {
        let sessions = service::list_sessions(c)?;
        let s = sessions
            .iter()
            .find(|s| s.id == session_id)
            .ok_or("会话不存在")?;
        let ws = service::list_workspaces(c)?
            .into_iter()
            .find(|w| w.id == s.workspace_id)
            .ok_or("会话所属工作区已被移除")?;
        let resume = service::get_claude_session_id(c, &session_id).unwrap_or_default();
        Ok((ws.path, resume, String::new(), Vec::<(String, String)>::new()))
    })?;

    // 模型与 env 从 settings.json 取
    let models = config::read_models()?;
    let model = models.get("ANTHROPIC_MODEL").cloned().unwrap_or_default();
    let env: Vec<(String, String)> = config::read_env()?
        .into_iter()
        .map(|e| (e.key, e.value))
        .collect();

    // 用户消息先落库
    db::with(|c| service::append_message(c, &session_id, "user", trimmed, &[], false))?;

    let run_id = state.run_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().map_err(|_| "状态锁中毒")?;
        *active = Some(ActiveRun {
            run_id,
            stop: stop.clone(),
        });
    }

    let opts = claude::Options {
        cwd,
        text: trimmed.to_string(),
        model,
        resume,
        mode: config::read_permissions()
            .map(|p| p.default_mode)
            .unwrap_or_else(|_| "acceptEdits".into()),
        env,
    };

    let sid = session_id.clone();
    let app2 = app.clone();
    let stop2 = stop.clone();

    emit(
        &app,
        ChatEvent {
            kind: "start".into(),
            session_id: sid.clone(),
            run_id,
            delta: None,
            tool: None,
            message: None,
            usage: None,
            interrupted: None,
            error: None,
        },
    );

    std::thread::spawn(move || {
        let acc = Arc::new(Mutex::new(String::new()));
        let tools: Arc<Mutex<Vec<service::ToolCall>>> = Arc::new(Mutex::new(vec![]));
        let acc2 = acc.clone();
        let tools2 = tools.clone();

        let res = claude::run(opts, stop2, |ev| match ev {
            claude::Event::Text(d) => {
                if let Ok(mut a) = acc2.lock() {
                    a.push_str(&d);
                }
                emit(
                    &app2,
                    ChatEvent {
                        kind: "text".into(),
                        session_id: sid.clone(),
                        run_id,
                        delta: Some(d),
                        tool: None,
                        message: None,
                        usage: None,
                        interrupted: None,
                        error: None,
                    },
                );
            }
            claude::Event::Tool(n, t) => {
                let tc = service::ToolCall { name: n, target: t };
                if let Ok(mut v) = tools2.lock() {
                    v.push(tc.clone());
                }
                emit(
                    &app2,
                    ChatEvent {
                        kind: "tool".into(),
                        session_id: sid.clone(),
                        run_id,
                        delta: None,
                        tool: Some(tc),
                        message: None,
                        usage: None,
                        interrupted: None,
                        error: None,
                    },
                );
            }
            claude::Event::ClaudeSession(cid) => {
                let _ = db::with(|c| {
                    service::set_claude_session_id(c, &sid, &cid);
                    Ok(())
                });
            }
            claude::Event::Context { total, max, percent } => {
                let _ = db::with(|c| {
                    service::set_context_usage(c, &sid, total, max, percent);
                    Ok(())
                });
            }
            claude::Event::Error(m) => {
                emit(
                    &app2,
                    ChatEvent {
                        kind: "error".into(),
                        session_id: sid.clone(),
                        run_id,
                        delta: None,
                        tool: None,
                        message: None,
                        usage: None,
                        interrupted: None,
                        error: Some(m),
                    },
                );
            }
        });

        let acc_text = acc.lock().map(|a| a.clone()).unwrap_or_default();
        let tool_list = tools.lock().map(|t| t.clone()).unwrap_or_default();

        match res {
            Ok(()) => {
                // 空回复也要落一条，否则界面看着像没反应
                let body = if acc_text.is_empty() {
                    "（模型没有返回任何内容）".to_string()
                } else {
                    acc_text
                };
                let saved = db::with(|c| {
                    service::append_message(c, &sid, "assistant", &body, &tool_list, false)
                });
                if let Ok(msg) = saved {
                    emit(
                        &app2,
                        ChatEvent {
                            kind: "done".into(),
                            session_id: sid.clone(),
                            run_id,
                            delta: None,
                            tool: None,
                            message: Some(msg),
                            usage: None,
                            interrupted: None,
                            error: None,
                        },
                    );
                }
            }
            Err(e) => {
                emit(
                    &app2,
                    ChatEvent {
                        kind: "error".into(),
                        session_id: sid,
                        run_id,
                        delta: None,
                        tool: None,
                        message: None,
                        usage: None,
                        interrupted: None,
                        error: Some(e),
                    },
                );
            }
        }
    });

    Ok(run_id)
}

#[tauri::command]
fn chat_interrupt(state: State<'_, AppState>) -> Result<(), String> {
    let active = state.active.lock().map_err(|_| "状态锁中毒")?;
    if let Some(run) = active.as_ref() {
        run.stop.store(true, Ordering::SeqCst);
        let _ = run.run_id;
    }
    Ok(())
}

// ── 应用信息 ──

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn main() {
    if let Err(e) = db::open() {
        eprintln!("数据库初始化失败: {e}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_title("Claude 工作台 · Tauri 版");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            workspace_list,
            workspace_add,
            workspace_remove,
            workspace_rename,
            workspace_pin,
            session_list,
            session_list_deleted,
            session_create,
            session_rename,
            session_remove,
            session_restore,
            session_purge,
            session_pin,
            session_duplicate,
            session_messages,
            session_message_count,
            config_read_env,
            config_save_env,
            config_read_permissions,
            config_save_permissions,
            config_read_models,
            config_save_models,
            config_health,
            skill_list,
            skill_toggle,
            plugin_list,
            plugin_toggle,
            plugin_install,
            plugin_uninstall,
            marketplace_list,
            marketplace_add,
            marketplace_remove,
            chat_send,
            chat_interrupt,
            app_version,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 启动失败");
}
