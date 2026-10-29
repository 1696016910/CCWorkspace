//! Tauri 后端入口：数据库初始化 + 全部 IPC 命令。
//!
//! 命令名用下划线（tauri 约定），前端 api/ 模块封装成和 Node 版同名的函数。

mod claude;
mod config;
mod db;
mod file;
mod pool;
mod service;
mod session;
mod skills;

use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};

/// 一轮对话的中断标志 + runId，供 chat_interrupt 精确停下对应那轮。
struct ActiveRun {
    stop: Arc<AtomicBool>,
}

/// 同时生成的会话上限。每轮都是一个 claude 子进程，放开会吃掉内存与 API 额度。
const MAX_CONCURRENT_RUNS: usize = 10;

struct AppState {
    run_seq: AtomicU64,
    /// 按会话分桶。单槽位时新开一轮会覆盖旧那轮的停止句柄，导致旧进程无法中断。
    active: Mutex<std::collections::HashMap<String, ActiveRun>>,
    /// CLI 在 init 事件里报的能力清单。首轮对话之后才有内容。
    meta: Mutex<Option<claude::Meta>>,
    /// 会话进程池：常驻 LRU 最多 3 个，省掉每轮 4.4s 的启动开销。
    pool: Arc<pool::Pool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            run_seq: AtomicU64::new(0),
            active: Mutex::new(std::collections::HashMap::new()),
            meta: Mutex::new(None),
            pool: pool::Pool::new(),
        }
    }
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

/// 用 `/context` 校准某个会话的上下文用量。
///
/// `/context` 是 CLI 的本地命令：`num_turns=0`、`duration_api_ms=0`、
/// usage 全 0，实测 179ms——不调模型，几乎免费。
/// 它给的是 CLI 自己算的权威值，比我们从 usage 推算的可靠。
///
/// 在后台线程里跑，不阻塞调用方。
/// 查询某会话的上下文用量。用户打开上下文弹窗时调用。
///
/// `/context` 不调模型（`duration_api_ms=0`），但要新起一个 CLI 进程，
/// 实测约 4.3s。所以只在用户主动查看时查，不再每轮自动跑。
#[tauri::command]
fn context_probe(app: tauri::AppHandle, session_id: String) -> Result<(), String> {
    let sid = session_id;
    let app2 = app.clone();
    std::thread::spawn(move || {
        let Ok((cwd, resume)) = db::with(|c| {
            let s = service::list_sessions(c)?
                .into_iter()
                .find(|s| s.id == sid)
                .ok_or("会话不存在")?;
            let ws = service::list_workspaces(c)?
                .into_iter()
                .find(|w| w.id == s.workspace_id)
                .ok_or("工作区不存在")?;
            Ok((
                ws.path,
                service::get_claude_session_id(c, &sid).unwrap_or_default(),
            ))
        }) else {
            return;
        };
        if resume.is_empty() {
            return;
        }

        let Ok(models) = config::read_models() else { return };
        let env: Vec<(String, String)> = config::read_env()
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect();

        let opts = claude::Options {
            cwd,
            text: "/context".into(),
            model: models.get("ANTHROPIC_MODEL").cloned().unwrap_or_default(),
            resume,
            mode: "acceptEdits".into(),
            env,
            images: vec![],
        };

        let mut probed: Option<(i64, i64, f64)> = None;
        let _ = claude::run(opts, Arc::new(AtomicBool::new(false)), |ev| {
            if let claude::Event::Context { total, max, percent } = ev {
                probed = Some((total, max, percent));
            }
        });

        if let Some((t, m, p)) = probed {
            let _ = db::with(|c| {
                service::set_context_usage(c, &sid, t, m, p);
                Ok(())
            });
            // 推给前端，界面上的百分比立刻刷新
            emit(
                &app2,
                ChatEvent {
                    kind: "context".into(),
                    session_id: sid.clone(),
                    run_id: 0,
                    delta: None,
                    tool: None,
                    message: None,
                    usage: Some(serde_json::json!({
                        "tokens": t, "max": m, "percent": p
                    })),
                    interrupted: None,
                    error: None,
                },
            );
        }
    });

    // 探测在后台跑，结果经 chat:event 的 context 消息回传
    Ok(())
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
async fn skill_read(
    name: String,
    source: String,
    project_dir: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        skills::read_skill(&name, &source, project_dir.as_deref())
    })
    .await
    .map_err(|e| format!("读取 Skill 失败: {e}"))?
}

#[tauri::command]
async fn skill_write(
    name: String,
    source: String,
    project_dir: Option<String>,
    content: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        skills::write_skill(&name, &source, project_dir.as_deref(), &content)
    })
    .await
    .map_err(|e| format!("写入 Skill 失败: {e}"))?
}

#[tauri::command]
async fn skill_delete(
    name: String,
    source: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        skills::delete_skill(&name, &source, project_dir.as_deref())
    })
    .await
    .map_err(|e| format!("删除 Skill 失败: {e}"))?
}

// ── 规则 ──

#[tauri::command]
fn rule_list(project_dir: Option<String>) -> Vec<skills::RuleInfo> {
    skills::list_rules(project_dir.as_deref())
}

#[tauri::command]
async fn rule_read(
    name: String,
    source: String,
    project_dir: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        skills::read_rule(&name, &source, project_dir.as_deref())
    })
    .await
    .map_err(|e| format!("读取规则失败: {e}"))?
}

#[tauri::command]
async fn rule_write(
    name: String,
    source: String,
    project_dir: Option<String>,
    content: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        skills::write_rule(&name, &source, project_dir.as_deref(), &content)
    })
    .await
    .map_err(|e| format!("写入规则失败: {e}"))?
}

#[tauri::command]
async fn rule_delete(
    name: String,
    source: String,
    project_dir: Option<String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        skills::delete_rule(&name, &source, project_dir.as_deref())
    })
    .await
    .map_err(|e| format!("删除规则失败: {e}"))?
}

#[tauri::command]
fn command_list(project_dir: Option<String>) -> Vec<skills::CommandInfo> {
    skills::list_commands(project_dir.as_deref())
}

/// 返回 CLI 上报的可用能力清单。首轮对话之前为空。
#[tauri::command]
fn cli_meta(state: State<'_, AppState>) -> Option<claude::Meta> {
    state.meta.lock().ok().and_then(|m| m.clone())
}

/// 这些命令内部要起 `claude plugin ...` 子进程，慢的几秒。
/// 同步执行会占住 IPC 主线程，窗口表现为未响应，所以统一挪到阻塞线程池。
#[tauri::command]
async fn plugin_list() -> Result<Vec<skills::PluginInfo>, String> {
    tauri::async_runtime::spawn_blocking(skills::list_plugins)
        .await
        .map_err(|e| format!("读取插件列表失败: {e}"))
}

#[tauri::command]
fn plugin_toggle(id: String, enabled: bool) -> Result<Vec<skills::PluginInfo>, String> {
    skills::toggle_plugin(&id, enabled)?;
    Ok(skills::list_plugins())
}

#[tauri::command]
async fn plugin_install(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || skills::install_plugin(&id))
        .await
        .map_err(|e| format!("安装插件失败: {e}"))?
}

#[tauri::command]
async fn plugin_uninstall(id: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || skills::uninstall_plugin(&id))
        .await
        .map_err(|e| format!("卸载插件失败: {e}"))?
}

#[tauri::command]
async fn marketplace_list() -> Vec<skills::MarketplaceInfo> {
    skills::list_marketplaces()
}

#[tauri::command]
async fn marketplace_add(name: String, repo: String) -> Result<Vec<skills::MarketplaceInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        skills::add_marketplace(&name, &repo)?;
        Ok(skills::list_marketplaces())
    })
    .await
    .map_err(|e| format!("添加插件仓库失败: {e}"))?
}

#[tauri::command]
async fn marketplace_remove(name: String) -> Result<Vec<skills::MarketplaceInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        skills::remove_marketplace(&name)?;
        Ok(skills::list_marketplaces())
    })
    .await
    .map_err(|e| format!("移除插件仓库失败: {e}"))?
}

// ── 对话 ──

/// 发一轮对话。立即返回 runId，内容通过 chat:event 推送。
#[tauri::command]
fn chat_send(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    text: String,
    images: Option<Vec<claude::ImageAttachment>>,
    attachments: Option<Vec<service::Attachment>>,
) -> Result<u64, String> {
    let trimmed = text.trim();
    let images = images.unwrap_or_default();
    let user_attachments = attachments.unwrap_or_default();
    if trimmed.is_empty() && images.is_empty() && user_attachments.is_empty() {
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

    let run_id = state.run_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().map_err(|_| "状态锁中毒")?;
        if active.contains_key(&session_id) {
            return Err("该会话正在生成回复，请先停止或等待完成".into());
        }
        if active.len() >= MAX_CONCURRENT_RUNS {
            return Err(format!(
                "同时最多 {MAX_CONCURRENT_RUNS} 个会话在生成，请等待其中一个完成"
            ));
        }
        active.insert(
            session_id.clone(),
            ActiveRun { stop: stop.clone() },
        );
    }

    // 用户消息先落库。放在占位成功之后，避免并发被拒时留下一条没有回复的用户消息
    db::with(|c| service::append_message(c, &session_id, "user", trimmed, &[], false, &user_attachments))?;

    let opts = claude::Options {
        cwd,
        text: trimmed.to_string(),
        model,
        resume,
        mode: config::read_permissions()
            .map(|p| p.default_mode)
            .unwrap_or_else(|_| "acceptEdits".into()),
        env,
        images,
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
        // AppHandle 是 clone + 'static，State 借用它，所以必须在闭包内取
        let state_active = app2.state::<AppState>();
        let acc = Arc::new(Mutex::new(String::new()));
        let tools: Arc<Mutex<Vec<service::ToolCall>>> = Arc::new(Mutex::new(vec![]));
        let acc2 = acc.clone();
        let tools2 = tools.clone();
        // 本轮统计：CLI 在 result 里给耗时，done 事件带给前端
        let stats: Arc<Mutex<Option<(i64, i64, i64)>>> = Arc::new(Mutex::new(None));
        let stats2 = stats.clone();

        // 文本增量走 channel：有数据立刻推，没数据阻塞。
        // 原来是 sleep(80ms) 轮询——空转 12 次/秒，首字还多等 0-80ms。
        let (text_tx, text_rx) = std::sync::mpsc::channel::<String>();
        let flusher = {
            let app = app2.clone();
            let sid = sid.clone();
            std::thread::spawn(move || {
                while let Ok(chunk) = text_rx.recv() {
                    emit(
                        &app,
                        ChatEvent {
                            kind: "text".into(),
                            session_id: sid.clone(),
                            run_id,
                            delta: Some(chunk),
                            tool: None,
                            message: None,
                            usage: None,
                            interrupted: None,
                            error: None,
                        },
                    );
                }
            })
        };

        // 从池里取常驻进程；没有就新建一个，用完放回池。
        // 每轮新建进程要付 4.4s 启动开销，实测一轮 8.5s 里占一半。
        let pool = state_active.pool.clone();
        let t0 = std::time::Instant::now();
        let mut reused = true;
        let mut proc = match pool.take_resident(&sid) {
            Some(p) => p,
            None => {
                reused = false;
                match session::SessionProcess::spawn(&opts) {
                    Ok(p) => p,
                Err(e) => {
                    if let Ok(mut active) = state_active.active.lock() {
                        active.remove(&sid);
                    }
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
                            error: Some(e),
                        },
                    );
                        return;
                    }
                }
            }
        };
        eprintln!(
            "[wb] 会话 {} {}（{}ms）",
            &sid[..8.min(sid.len())],
            if reused { "复用常驻进程" } else { "新建进程" },
            t0.elapsed().as_millis()
        );

        let res = proc.run_turn(&opts.text, &opts.images, stop2, |ev| match ev {
            claude::Event::Text(d) => {
                if let Ok(mut a) = acc2.lock() {
                    a.push_str(&d);
                }
                let _ = text_tx.send(d);
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
                match db::with(|c| {
                    service::set_claude_session_id(c, &sid, &cid);
                    Ok(())
                }) {
                    Ok(()) => eprintln!("[wb] claude 会话 id 已记录: {cid}"),
                    Err(e) => eprintln!("[wb] 记录 claude 会话 id 失败: {e}"),
                }
            }
            claude::Event::Meta(m) => {
                if let Ok(mut slot) = state_active.meta.lock() {
                    *slot = Some(m.clone());
                }
                // 推给前端，补全面板靠它拿到全量清单
                emit(
                    &app2,
                    ChatEvent {
                        kind: "meta".into(),
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
            }
            claude::Event::Context { total, max, percent } => {
                let _ = db::with(|c| {
                    // 直接覆盖：上下文会被 CLI 清理，回落是真实的。
                    // 曾经为了「不跳变」做成只增不减，结果读过大文件之后
                    // 永远虚高（某轮 142k，之后实际只剩 21k）。
                    service::set_context_usage(c, &sid, total, max, percent);
                    Ok(())
                });
            }
            claude::Event::Done {
                duration_ms,
                ttft_ms,
                num_turns,
            } => {
                if let Ok(mut slot) = stats2.lock() {
                    *slot = Some((duration_ms, ttft_ms, num_turns));
                }
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

        // 关掉发送端，flusher 读完就退出
        drop(text_tx);
        let _ = flusher.join();

        // 决定这个进程是留着还是丢掉：
        // - 正常结束 —— 放回池，下次直接复用
        // - 中断/超时 —— 丢掉，下次重建（带 --resume 接回上下文）
        let reusable =
            matches!(&res, Ok(o) if !o.interrupted && !o.timed_out && !o.process_died)
                && proc.alive();
        if reusable {
            pool.put_resident(sid.clone(), proc);
        } else {
            drop(proc);
        }

        let acc_text = acc.lock().map(|a| a.clone()).unwrap_or_default();
        let tool_list = tools.lock().map(|t| t.clone()).unwrap_or_default();

        // 无论成败都要摘掉占位，否则该会话之后再也不能发消息
        if let Ok(mut active) = state_active.active.lock() {
            active.remove(&sid);
        }

        let outcome = res.as_ref().copied().unwrap_or_default();
        match res {
            Ok(o) if !o.interrupted => {
                // 空回复也要落一条，否则界面看着像没反应
                let body = if acc_text.is_empty() {
                    "（模型没有返回任何内容）".to_string()
                } else {
                    acc_text
                };
                let saved = db::with(|c| {
                    service::append_message(c, &sid, "assistant", &body, &tool_list, false, &[])
                });
                let elapsed = stats.lock().ok().and_then(|s| *s);
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
                            usage: elapsed.map(|(d, ttft, turns)| {
                                serde_json::json!({
                                    "durationMs": d,
                                    "ttftMs": ttft,
                                    "numTurns": turns,
                                })
                            }),
                            interrupted: None,
                            error: None,
                        },
                    );
                    // 不再每轮探测上下文：`/context` 虽不调模型，
                    // 但要新起一个 CLI 进程，实测 4.3s——只为一个用户未必在看的数字。
                    // 改成用户打开弹窗时按需查询（context_probe 命令）。
                }
            }
            Ok(_) => {
                // 被中断：已收到的内容仍要落库，并标记 stopped
                let body = if acc_text.is_empty() {
                    "（已停止生成）".to_string()
                } else {
                    format!("{acc_text}\n\n（已停止生成）")
                };
                let saved = db::with(|c| {
                    service::append_message(c, &sid, "assistant", &body, &tool_list, true, &[])
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
                            interrupted: Some(true),
                            error: None,
                        },
                    );
                }
            }
            Err(e) => {
                let _ = outcome;
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

/// 压缩上下文：跑一轮 `/compact`，只把摘要落库，不写用户消息。
///
/// 必须异步：CLI 一轮要几十秒，同步执行会把 IPC 主线程占死，窗口表现为「未响应」。
#[tauri::command]
fn context_compact(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    // 有对话在跑时压缩会开第二个 claude 操作同一个会话，两边都会乱
    {
        let active = state.active.lock().map_err(|_| "状态锁中毒")?;
        if active.contains_key(&session_id) {
            return Err("当前有对话正在进行，请先停止或等待完成".into());
        }
        if active.len() >= MAX_CONCURRENT_RUNS {
            return Err(format!(
                "同时最多 {MAX_CONCURRENT_RUNS} 个会话在生成，请等待其中一个完成"
            ));
        }
    }

    let (cwd, resume) = db::with(|c| {
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
        Ok((ws.path, resume))
    })?;

    let models = config::read_models()?;
    let env: Vec<(String, String)> = config::read_env()?
        .into_iter()
        .map(|e| (e.key, e.value))
        .collect();

    let model = models.get("ANTHROPIC_MODEL").cloned().unwrap_or_default();
    let opts = claude::Options {
        cwd: cwd.clone(),
        text: "/compact".into(),
        model: model.clone(),
        resume: resume.clone(),
        mode: "acceptEdits".into(),
        env: env.clone(),
        images: vec![],
    };

    // 占位时用同一个 active 表，防止压缩期间又开一轮对话
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().map_err(|_| "状态锁中毒")?;
        active.insert(
            session_id.clone(),
            ActiveRun { stop: stop.clone() },
        );
    }

    let app2 = app.clone();
    std::thread::spawn(move || {
        let state_active = app2.state::<AppState>();
        let pool = state_active.pool.clone();

        // 复用常驻进程：压缩和随后的探测可以共用同一个进程，
        // 省掉两次 4.4s 的启动开销。
        let mut proc = match pool.take_resident(&session_id) {
            Some(p) => p,
            None => match session::SessionProcess::spawn(&opts) {
                Ok(p) => p,
                Err(e) => {
                    if let Ok(mut active) = state_active.active.lock() {
                        active.remove(&session_id);
                    }
                    let _ = app2.emit(
                        "chat:compact",
                        serde_json::json!({
                            "sessionId": session_id,
                            "ok": false,
                            "error": e,
                        }),
                    );
                    return;
                }
            },
        };

        let mut summary = String::new();
        let res = proc.run_turn("/compact", &[], stop, |ev| {
            if let claude::Event::Text(d) = ev {
                summary.push_str(&d);
            }
        });

        if let Ok(mut active) = state_active.active.lock() {
            active.remove(&session_id);
        }

        let text = summary.trim().to_string();
        // `/compact` 是 CLI 的本地命令：实测 is_error=false、subtype=success，
        // 但 result 是**空字符串**（它只做压缩，不产摘要文本）。
        // 原来把空文本当失败，导致每次压缩都报「压缩没有返回摘要」。
        let body = if text.is_empty() {
            "（上下文已压缩）".to_string()
        } else {
            format!("（上下文已压缩）\n\n{text}")
        };
        let result = match res {
            Err(e) => Err(e),
            Ok(o) if o.timed_out => Err("压缩超时，请稍后重试".to_string()),
            Ok(o) if o.interrupted => Err("压缩已中断".to_string()),
            Ok(_) => {
                // 压缩后立刻问一次 `/context`。关键是**复用同一个进程**——
                // 新起一个要多付 4.4s，而这里进程已经在手上。
                let mut probed: Option<(i64, i64, f64)> = None;
                if proc.alive() {
                    let _ = proc.run_turn(
                        "/context",
                        &[],
                        Arc::new(AtomicBool::new(false)),
                        |ev| {
                            if let claude::Event::Context { total, max, percent } = ev {
                                probed = Some((total, max, percent));
                            }
                        },
                    );
                }

                db::with(|c| {
                    service::append_message(c, &session_id, "assistant", &body, &[], false, &[])?;
                    match probed {
                        Some((t, m, p)) => service::set_context_usage(c, &session_id, t, m, p),
                        // 探测失败才归零：总比留在压缩前的旧值强
                        None => service::set_context_usage(c, &session_id, 0, 0, 0.0),
                    }
                    Ok(body)
                })
            }
        };

        // 进程放回池，下次对话直接复用
        if proc.alive() {
            pool.put_resident(session_id.clone(), proc);
        } else {
            drop(proc);
        }

        let _ = app2.emit(
            "chat:compact",
            serde_json::json!({
                "sessionId": session_id,
                "ok": result.is_ok(),
                "error": result.as_ref().err().cloned(),
            }),
        );
    });

    Ok(())
}

/// 中断某一轮对话。
///
/// 必须带目标：并发下无参版本会把所有会话的生成一起停掉。
#[tauri::command]
fn chat_interrupt(state: State<'_, AppState>, session_id: Option<String>) -> Result<(), String> {
    let active = state.active.lock().map_err(|_| "状态锁中毒")?;
    match session_id {
        Some(sid) => {
            let run = active
                .get(&sid)
                .ok_or("该会话没有正在进行的生成")?;
            run.stop.store(true, Ordering::SeqCst);
            // 中断后进程状态不可信，直接丢掉，下轮重建（带 --resume 接回上下文）
            state.pool.evict(&sid);
        }
        None => {
            // 兼容旧调用：没有目标时按「全部停止」处理
            for run in active.values() {
                run.stop.store(true, Ordering::SeqCst);
            }
        }
    }
    Ok(())
}

/// 读取工作区内的文本文件用于预览。
#[tauri::command]
fn file_read(path: String) -> Result<file::FileContent, String> {
    let roots = db::with(|c| Ok(service::list_workspaces(c)?.into_iter().map(|w| w.path).collect::<Vec<_>>()))?;
    file::read(&roots, &path)
}

/// 把本地图片读成 base64，供附件预览与发送。
///
/// 走这里而不是前端 FileReader：Tauri 的 `<input type="file">` 拿不到真实路径
/// （`.path` 是 Electron 的扩展），只有 dialog 插件返回的路径才是可信的绝对路径。
#[tauri::command]
fn file_image(path: String) -> Result<file::ImageData, String> {
    file::read_image(&path)
}

/// 批量判定哪些路径可预览。一次算完，避免每条消息各发起一次 IPC。
#[tauri::command]
fn file_check(paths: Vec<String>) -> Result<Vec<bool>, String> {
    let roots = db::with(|c| Ok(service::list_workspaces(c)?.into_iter().map(|w| w.path).collect::<Vec<_>>()))?;
    Ok(file::check(&roots, &paths))
}

/// 列一层目录，供文件树按需展开。
///
/// `rel` 是相对工作区根的子路径；空字符串表示根。只列一层，
/// 展开子目录时前端再调一次——大项目一次递归会卡住界面。
#[tauri::command]
fn workspace_list_dir(
    workspace_id: String,
    rel: String,
    show_ignored: bool,
) -> Result<Vec<file::TreeNode>, String> {
    let root = db::with(|c| {
        let ws = service::list_workspaces(c)?
            .into_iter()
            .find(|w| w.id == workspace_id)
            .ok_or("工作区不存在")?;
        Ok(ws.path)
    })?;
    file::list_dir(&root, &rel, show_ignored)
}

/// 在系统文件管理器中打开某个工作区。
///
/// 先查存在性再启动：目录没了直接返回可读错误，别让 explorer 弹系统对话框。
/// explorer 传目录时退出码经常是 1 但实际成功，所以不检查返回值。
#[tauri::command]
fn workspace_reveal(id: String) -> Result<(), String> {
    let path = db::with(|c| {
        let ws = service::list_workspaces(c)?
            .into_iter()
            .find(|w| w.id == id)
            .ok_or("工作区不存在")?;
        Ok(ws.path)
    })?;

    if !std::path::Path::new(&path).exists() {
        return Err(format!("工作区目录不存在: {path}"));
    }

    #[cfg(target_os = "windows")]
    let mut cmd = std::process::Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = std::process::Command::new("xdg-open");

    cmd.arg(&path)
        .spawn()
        .map_err(|e| format!("打开文件管理器失败: {e}"))?;
    Ok(())
}

// ── 应用信息 ──

/// 诊断用：当前常驻会话进程数。也方便排查「进程是不是泄漏了」。
#[tauri::command]
fn pool_status(state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({
        "resident": state.pool.resident_count(),
        "maxResident": 3,
    })
}

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

            // 每 60 秒扫一次空闲进程，超过 5 分钟没用就回收
            let pool = app.state::<AppState>().pool.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
                let n = pool.sweep_idle();
                if n > 0 {
                    eprintln!("[wb] 回收 {n} 个空闲会话进程");
                }
            });

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
            skill_read,
            skill_write,
            skill_delete,
            rule_list,
            rule_read,
            rule_write,
            rule_delete,
            command_list,
            cli_meta,
            plugin_list,
            plugin_toggle,
            plugin_install,
            plugin_uninstall,
            marketplace_list,
            marketplace_add,
            marketplace_remove,
            file_read,
            file_image,
            file_check,
            workspace_reveal,
            workspace_list_dir,
            chat_send,
            chat_interrupt,
            context_probe,
            context_compact,
            app_version,
            pool_status,
        ])
        .build(tauri::generate_context!())
        .expect("Tauri 启动失败")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // 退出时杀掉所有常驻进程，避免留下僵尸
                app.state::<AppState>().pool.shutdown();
            }
        });
}
