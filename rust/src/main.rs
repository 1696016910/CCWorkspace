//! Claude 工作台 · Rust 实现
//!
//! 与 Node 版读写同一个 SQLite 数据库、同一份 ~/.claude/settings.json，
//! 通过 spawn claude 子进程走 stdio JSON 协议对话。

mod claude;
mod db;
mod model;
mod service;

use eframe::egui;
use model::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

const VERSION: &str = "0.2.1";

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([900.0, 600.0])
            .with_title("Claude 工作台 · Rust 版"),
        ..Default::default()
    };
    eframe::run_native(
        "claude-workbench-rust",
        options,
        Box::new(|cc| {
            install_cjk_font(&cc.egui_ctx);
            Ok(Box::new(App::new()))
        }),
    )
}

/// egui 内置字体没有中文，从系统加载。
///
/// 只接受 .ttf：.ttc 是字体集合，egui 解析不了。
fn install_cjk_font(ctx: &egui::Context) {
    const CANDIDATES: &[&str] = &[
        r"C:\Windows\Fonts\Deng.ttf",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\msyh.ttf",
        "/System/Library/Fonts/PingFang.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttf",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttf",
    ];

    for path in CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else { continue };
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("cjk".to_owned(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "cjk".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("cjk".to_owned());
        ctx.set_fonts(fonts);
        return;
    }
    eprintln!("未找到中文字体，中文可能显示为方框");
}

/// 会话右键菜单选中的动作。
enum SessionAction {
    Rename(String, String),
    Pin(String, bool),
    Duplicate(String),
    Delete(String, String),
}

/// 后台线程回传的消息。
enum Msg {
    Delta(String),
    Done(String, bool),
    Error(String),
    /// CLI 会话 id，落库后下一轮才能 --resume 接上上下文
    ClaudeSession(String),
    /// 上下文用量快照
    Context { total: i64, max: i64, percent: f64 },
}

struct App {
    conn: Option<rusqlite::Connection>,
    db_error: Option<String>,

    workspaces: Vec<Workspace>,
    sessions: Vec<SessionSummary>,
    messages: Vec<ChatMessage>,

    current_ws: Option<String>,
    current_ses: Option<String>,

    input: String,
    streaming: bool,
    status: String,
    rx: Option<Receiver<Msg>>,

    // 弹窗状态
    show_new_ws: bool,
    new_ws_path: String,
    new_ws_name: String,
    pending_delete: Option<(String, String, bool)>, // (id, 名称, 是工作区)
    renaming: Option<(String, String, bool)>,       // (id, 当前名称, 是工作区)

    // 上下文面板 / 回收站 / 文件预览
    show_context: bool,
    show_trash: bool,
    trash: Vec<SessionSummary>,
    preview: Option<(String, String)>, // (标题, 内容)

    // 设置页
    show_settings: bool,
    settings_tab: usize,
    // 设置页的编辑缓冲
    env_rows: Vec<(String, String)>,
    perm_mode: String,
    perm_allow: String,
    perm_deny: String,
    model_fields: Vec<(String, String, String)>, // (key, label, value)
    mkt_name: String,
    mkt_repo: String,
}

impl App {
    fn new() -> Self {
        let mut app = App {
            conn: None,
            db_error: None,
            workspaces: vec![],
            sessions: vec![],
            messages: vec![],
            current_ws: None,
            current_ses: None,
            input: String::new(),
            streaming: false,
            status: "就绪".into(),
            rx: None,
            show_new_ws: false,
            new_ws_path: String::new(),
            new_ws_name: String::new(),
            pending_delete: None,
            renaming: None,
            show_context: false,
            show_trash: false,
            trash: vec![],
            preview: None,
            show_settings: false,
            settings_tab: 0,
            env_rows: vec![],
            perm_mode: "acceptEdits".into(),
            perm_allow: String::new(),
            perm_deny: String::new(),
            model_fields: vec![],
            mkt_name: String::new(),
            mkt_repo: String::new(),
        };
        match db::open() {
            Ok(c) => {
                app.conn = Some(c);
                app.reload();
            }
            Err(e) => app.db_error = Some(e),
        }
        app
    }

    fn reload(&mut self) {
        let Some(conn) = &self.conn else { return };
        self.workspaces = service::list_workspaces(conn);
        self.sessions = service::list_sessions(conn);
    }

    fn load_messages(&mut self) {
        let (Some(conn), Some(sid)) = (&self.conn, &self.current_ses) else {
            self.messages.clear();
            return;
        };
        self.messages = service::list_messages(conn, sid, service::PAGE_SIZE);
    }

    /// 打开设置页并载入当前值。
    fn open_settings(&mut self) {
        self.settings_tab = 0;
        self.show_settings = true;
        self.load_settings();
    }

    fn load_settings(&mut self) {
        self.env_rows = service::read_env().into_iter().map(|e| (e.key, e.value)).collect();
        let perm = service::read_permissions();
        self.perm_mode = perm.default_mode;
        self.perm_allow = perm.allow.join("
");
        self.perm_deny = perm.deny.join("
");
        let models = service::read_models();
        self.model_fields = vec![
            ("ANTHROPIC_MODEL".into(), "主模型".into(), models.get("ANTHROPIC_MODEL").cloned().unwrap_or_default()),
            ("ANTHROPIC_DEFAULT_OPUS_MODEL".into(), "Opus 映射".into(), models.get("ANTHROPIC_DEFAULT_OPUS_MODEL").cloned().unwrap_or_default()),
            ("ANTHROPIC_DEFAULT_SONNET_MODEL".into(), "Sonnet 映射".into(), models.get("ANTHROPIC_DEFAULT_SONNET_MODEL").cloned().unwrap_or_default()),
            ("ANTHROPIC_DEFAULT_HAIKU_MODEL".into(), "Haiku 映射".into(), models.get("ANTHROPIC_DEFAULT_HAIKU_MODEL").cloned().unwrap_or_default()),
            ("CLAUDE_CODE_EFFORT_LEVEL".into(), "推理强度".into(), models.get("CLAUDE_CODE_EFFORT_LEVEL").cloned().unwrap_or_default()),
            ("MAX_THINKING_TOKENS".into(), "思考预算".into(), models.get("MAX_THINKING_TOKENS").cloned().unwrap_or_default()),
        ];
    }

    fn send(&mut self, ctx: &egui::Context) {
        let Some(sid) = self.current_ses.clone() else {
            self.status = "请先选择一个会话".into();
            return;
        };
        let text = self.input.trim().to_string();
        if text.is_empty() || self.streaming {
            return;
        }

        // cwd 取会话所属工作区
        let ws_id = self
            .sessions
            .iter()
            .find(|s| s.id == sid)
            .map(|s| s.workspace_id.clone());
        let cwd = ws_id
            .and_then(|w| self.workspaces.iter().find(|x| x.id == w).map(|x| x.path.clone()));
        let Some(cwd) = cwd else {
            self.status = "会话所属工作区不存在".into();
            return;
        };

        let user_msg_result = match &self.conn {
            Some(conn) => service::append_message(conn, &sid, "user", &text, &[], false),
            None => Err("数据库未打开".into()),
        };
        if let Err(e) = user_msg_result {
            self.status = e;
            return;
        }
        self.input.clear();
        self.load_messages();
        self.streaming = true;
        self.status = "正在生成…".into();

        let resume = self
            .conn
            .as_ref()
            .and_then(|c| service::get_claude_session_id(c, &sid))
            .unwrap_or_default();
        let opts = claude::Options {
            cwd,
            text,
            model: service::read_models().get("ANTHROPIC_MODEL").cloned().unwrap_or_default(),
            resume,
            mode: "acceptEdits".into(),
            env: service::read_env().into_iter().map(|e| (e.key, e.value)).collect(),
        };

        let (tx, rx): (Sender<Msg>, Receiver<Msg>) = channel();
        self.rx = Some(rx);

        let ctx2 = ctx.clone();
        let sid2 = sid.clone();
        std::thread::spawn(move || {
            let mut acc = String::new();
            let mut tools: Vec<ToolCall> = vec![];
            let tx2 = tx.clone();
            let res = claude::run(opts, |e| match e {
                claude::Event::Text(d) => {
                    acc.push_str(&d);
                    let _ = tx2.send(Msg::Delta(d));
                    ctx2.request_repaint();
                }
                claude::Event::Tool(n, t) => tools.push(ToolCall { name: n, target: t }),
                claude::Event::ClaudeSession(cid) => {
                    let _ = tx2.send(Msg::ClaudeSession(cid));
                }
                claude::Event::Context { total, max, percent, .. } => {
                    let _ = tx2.send(Msg::Context { total, max, percent });
                }
                claude::Event::Error(m) => {
                    let _ = tx2.send(Msg::Error(m));
                }
            });
            match res {
                Ok(()) if acc.is_empty() && tools.is_empty() => {
                    let _ = tx.send(Msg::Error("模型没有返回任何内容".into()));
                }
                Ok(()) => {
                    let _ = tx.send(Msg::Done(acc, false));
                }
                Err(e) => {
                    let _ = tx.send(Msg::Error(e));
                }
            }
            let _ = sid2;
        });
    }

    fn poll(&mut self) {
        let Some(rx) = &self.rx else { return };
        let mut finished: Option<(String, bool)> = None;
        let mut err: Option<String> = None;
        let mut cli_sid: Option<String> = None;
        let mut ctx_usage: Option<(i64, i64, f64)> = None;
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::ClaudeSession(cid) => cli_sid = Some(cid),
                Msg::Context { total, max, percent } => ctx_usage = Some((total, max, percent)),
                Msg::Delta(d) => {
                    if let Some(last) = self.messages.last_mut() {
                        if last.role == "assistant" && last.id == "__stream" {
                            last.text.push_str(&d);
                            continue;
                        }
                    }
                    self.messages.push(ChatMessage {
                        id: "__stream".into(),
                        role: "assistant".into(),
                        text: d,
                        tools: vec![],
                        timestamp: None,
                        stopped: false,
                    });
                }
                Msg::Done(full, stopped) => finished = Some((full, stopped)),
                Msg::Error(e) => err = Some(e),
            }
        }

        if let (Some(cid), Some(conn), Some(sid)) = (cli_sid, &self.conn, &self.current_ses) {
            service::set_claude_session_id(conn, sid, &cid);
        }
        if let (Some((total, max, percent)), Some(conn), Some(sid)) = (ctx_usage, &self.conn, &self.current_ses) {
            service::set_context_usage(conn, sid, total, max, percent);
        }

        if let Some((full, stopped)) = finished {
            self.streaming = false;
            self.rx = None;
            if let (Some(conn), Some(sid)) = (&self.conn, &self.current_ses) {
                let tools: Vec<ToolCall> = self
                    .messages
                    .last()
                    .filter(|m| m.id == "__stream")
                    .map(|m| m.tools.clone())
                    .unwrap_or_default();
                let _ = service::append_message(conn, sid, "assistant", &full, &tools, stopped);
            }
            self.load_messages();
            self.status = if stopped { "已停止".into() } else { "就绪".into() };
        }
        if let Some(e) = err {
            self.streaming = false;
            self.rx = None;
            self.status = e;
        }
    }

    /// 上下文用量面板。
    ///
    /// 必须在 CentralPanel 之后调用：egui 按调用顺序绘制，
    /// 提前画的 Window 会被主区盖住，点开按钮什么也看不到。
    /// 只展示已落库的快照，不额外跑一次 /context，那会再花一轮 tokens。
    fn render_context_panel(&mut self, ctx: &egui::Context) {
        if !self.show_context {
            return;
        }
        let info = self
            .current_ses
            .as_ref()
            .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
            .map(|s| (s.context_percent, s.context_tokens));

        let mut open = true;
        egui::Window::new("上下文使用情况")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-24.0, -160.0))
            .default_width(300.0)
            .show(ctx, |ui| {
                match info {
                    Some((pct, tokens)) => {
                        ui.add(
                            // 不能用 INFINITY：浮动 Window 里会算出无效尺寸，
                            // 整块面板直接不渲染。
                            egui::ProgressBar::new((pct / 100.0) as f32)
                                .desired_width(240.0)
                                .text(format!("{pct:.1}%")),
                        );
                        ui.add_space(8.0);
                        ui.label(format!("已用 {}", human_tokens(tokens)));
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("每轮对话结束后由 CLI 的用量事件更新。")
                                .small()
                                .weak(),
                        );
                    }
                    None => {
                        ui.label("请先选择一个会话。");
                    }
                }
            });
        if !open {
            self.show_context = false;
        }
    }

    fn open_trash(&mut self) {
        self.trash = match &self.conn {
            Some(c) => service::list_deleted_sessions(c),
            None => vec![],
        };
        self.show_trash = true;
    }

    fn render_trash(&mut self, ctx: &egui::Context) {
        if !self.show_trash {
            return;
        }
        let mut open = true;
        let items = self.trash.clone();
        let mut restore: Option<String> = None;
        let mut purge: Option<(String, String)> = None;

        egui::Window::new("回收站")
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label("删除的会话会先放在这里，可以随时恢复。彻底删除后无法找回。");
                ui.separator();
                if items.is_empty() {
                    ui.label("回收站是空的");
                }
                egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                    for s in &items {
                        let title = if s.title.is_empty() { "(新会话)".to_string() } else { s.title.clone() };
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.strong(&title);
                                ui.label(
                                    egui::RichText::new(format!("{} · {} 条消息", s.updated_at, s.message_count))
                                        .small()
                                        .weak(),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("彻底删除").clicked() {
                                    purge = Some((s.id.clone(), title.clone()));
                                }
                                if ui.button("恢复").clicked() {
                                    restore = Some(s.id.clone());
                                }
                            });
                        });
                        ui.separator();
                    }
                });
            });

        if let Some(id) = restore {
            if let Some(c) = &self.conn {
                match service::restore_session(c, &id) {
                    Ok(()) => {
                        self.reload();
                        self.status = "已恢复会话".into();
                    }
                    Err(e) => self.status = e,
                }
            }
            self.open_trash();
        }
        if let Some((id, name)) = purge {
            if let Some(c) = &self.conn {
                match service::purge_session(c, &id) {
                    Ok(()) => {
                        self.reload();
                        self.status = format!("已彻底删除「{name}」");
                    }
                    Err(e) => self.status = e,
                }
            }
            self.open_trash();
        }
        if !open {
            self.show_trash = false;
        }
    }

    /// 预览工作区内的文本文件。
    fn open_file(&mut self, rel: String) {
        let Some(root) = self
            .current_ws
            .as_ref()
            .and_then(|id| self.workspaces.iter().find(|w| &w.id == id).map(|w| w.path.clone()))
        else {
            self.status = "请先选择一个工作区".into();
            return;
        };
        match service::read_workspace_file(&root, &rel) {
            Ok(content) => self.preview = Some((rel, content)),
            Err(e) => self.status = e,
        }
    }

    fn render_preview(&mut self, ctx: &egui::Context) {
        let Some((title, content)) = self.preview.clone() else { return };
        let mut open = true;
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .default_width(720.0)
            .default_height(520.0)
            .show(ctx, |ui| {
                egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                    let mut text = content.as_str();
                    ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
            });
        if !open {
            self.preview = None;
        }
    }

    /// 删除确认弹窗。主界面和设置页共用。
    fn render_delete_dialog(&mut self, ctx: &egui::Context) {
        let Some((id, name, is_ws)) = self.pending_delete.clone() else { return };
        let mut open = true;
        let (kind, hint) = if is_ws {
            ("工作区", "将同时删除它下面的所有会话与消息，且无法恢复。磁盘目录不会被删除。")
        } else {
            ("会话", "会移到回收站，可以随时恢复。")
        };
        egui::Window::new(format!("删除{kind}"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!("确定删除「{name}」？"));
                ui.label(egui::RichText::new(hint).small().weak());
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("确定删除").clicked() {
                        if let Some(conn) = &self.conn {
                            let r = if is_ws {
                                service::remove_workspace(conn, &id)
                            } else {
                                service::delete_session(conn, &id)
                            };
                            match r {
                                Ok(()) => {
                                    if is_ws && self.current_ws.as_deref() == Some(&id) {
                                        self.current_ws = None;
                                        self.current_ses = None;
                                        self.messages.clear();
                                    }
                                    if !is_ws && self.current_ses.as_deref() == Some(&id) {
                                        self.current_ses = None;
                                        self.messages.clear();
                                    }
                                    self.reload();
                                    self.status =
                                        if is_ws { "已删除工作区".into() } else { "已移到回收站".into() };
                                }
                                Err(e) => self.status = e,
                            }
                        }
                        self.pending_delete = None;
                    }
                    if ui.button("取消").clicked() {
                        self.pending_delete = None;
                    }
                });
            });
        if !open {
            self.pending_delete = None;
        }
    }

    /// 设置页：左侧 8 个分区导航 + 右侧内容。
    fn render_settings(&mut self, ctx: &egui::Context) {
        const TABS: &[&str] = &[
            "工作区", "环境变量", "权限", "模型", "Skill", "插件", "插件仓库", "更新",
        ];

        egui::TopBottomPanel::top("settings_head").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("← 返回").clicked() {
                    self.show_settings = false;
                }
                ui.heading("设置");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(&self.status).small().weak());
                });
            });
        });

        egui::SidePanel::left("settings_nav")
            .resizable(false)
            .default_width(160.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                for (i, t) in TABS.iter().enumerate() {
                    if ui.selectable_label(self.settings_tab == i, *t).clicked() {
                        self.settings_tab = i;
                        self.load_settings();
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| match self.settings_tab {
                    0 => self.settings_workspaces(ui),
                    1 => self.settings_env(ui),
                    2 => self.settings_permissions(ui),
                    3 => self.settings_models(ui),
                    4 => self.settings_skills(ui),
                    5 => self.settings_plugins(ui),
                    6 => self.settings_marketplaces(ui),
                    _ => self.settings_update(ui),
                });
        });

        self.render_delete_dialog(ctx);
        self.render_rename_dialog(ctx);
        self.render_trash(ctx);
        self.render_preview(ctx);
    }

    fn settings_workspaces(&mut self, ui: &mut egui::Ui) {
        ui.heading("工作区");
        ui.label("工作区是一个项目目录，每个工作区下可以建多个会话。");
        ui.separator();

        let list = self.workspaces.clone();
        if list.is_empty() {
            ui.label("还没有工作区");
        }
        for w in &list {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.strong(&w.name);
                    ui.label(egui::RichText::new(&w.path).small().weak());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("移除").clicked() {
                        self.pending_delete = Some((w.id.clone(), w.name.clone(), true));
                    }
                    if ui.button("重命名").clicked() {
                        self.renaming = Some((w.id.clone(), w.name.clone(), true));
                    }
                    let pin_label = if w.pinned_at.is_some() { "取消置顶" } else { "置顶" };
                    if ui.button(pin_label).clicked() {
                        if let Some(conn) = &self.conn {
                            let _ = service::pin_workspace(conn, &w.id, w.pinned_at.is_none());
                            self.reload();
                        }
                    }
                });
            });
            ui.separator();
        }

        ui.add_space(8.0);
        ui.label("添加工作区");
        ui.add(
            egui::TextEdit::singleline(&mut self.new_ws_path)
                .hint_text("项目目录绝对路径")
                .desired_width(360.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.new_ws_name)
                .hint_text("名称（留空用目录名）")
                .desired_width(360.0),
        );
        let can = !self.new_ws_path.trim().is_empty();
        if ui.add_enabled(can, egui::Button::new("添加")).clicked() {
            if let Some(conn) = &self.conn {
                match service::add_workspace(conn, &self.new_ws_path, &self.new_ws_name) {
                    Ok(()) => {
                        self.new_ws_path.clear();
                        self.new_ws_name.clear();
                        self.reload();
                        self.status = "工作区已添加".into();
                    }
                    Err(e) => self.status = e,
                }
            }
        }
    }

    fn render_rename_dialog(&mut self, ctx: &egui::Context) {
        let Some((id, name, is_ws)) = self.renaming.clone() else { return };
        let mut open = true;
        let mut edited = name.clone();
        egui::Window::new(if is_ws { "重命名工作区" } else { "重命名会话" })
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add(egui::TextEdit::singleline(&mut edited).desired_width(300.0));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let can = !edited.trim().is_empty();
                    if ui.add_enabled(can, egui::Button::new("保存")).clicked() {
                        if let Some(conn) = &self.conn {
                            let r = if is_ws {
                                service::rename_workspace(conn, &id, edited.trim())
                            } else {
                                service::rename_session(conn, &id, edited.trim())
                            };
                            match r {
                                Ok(()) => {
                                    self.reload();
                                    self.status = "已重命名".into();
                                }
                                Err(e) => self.status = e,
                            }
                        }
                        self.renaming = None;
                    }
                    if ui.button("取消").clicked() {
                        self.renaming = None;
                    }
                });
            });
        if !open {
            self.renaming = None;
        }
    }

    fn settings_env(&mut self, ui: &mut egui::Ui) {
        ui.heading("环境变量");
        ui.label("写入 ~/.claude/settings.json 的 env 字段，保存前会自动备份。");
        ui.separator();

        let mut remove_at: Option<usize> = None;
        for (i, (k, v)) in self.env_rows.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(k).hint_text("名称").desired_width(220.0));
                ui.add(egui::TextEdit::singleline(v).hint_text("值").desired_width(320.0));
                if ui.button("删除").clicked() {
                    remove_at = Some(i);
                }
            });
        }
        if let Some(i) = remove_at {
            self.env_rows.remove(i);
        }

        ui.add_space(6.0);
        if ui.button("＋ 添加一行").clicked() {
            self.env_rows.push((String::new(), String::new()));
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("这些值以明文保存在本地配置文件中，不会上传。").small().weak());
        ui.add_space(6.0);
        if ui.button("保存").clicked() {
            let list: Vec<EnvEntry> = self
                .env_rows
                .iter()
                .filter(|(k, _)| !k.trim().is_empty())
                .map(|(k, v)| EnvEntry { key: k.trim().to_string(), value: v.clone() })
                .collect();
            if let Some(problem) = service::validate_env(&list) {
                self.status = problem;
            } else {
                match service::save_env(&list) {
                    Ok(()) => self.status = "已保存".into(),
                    Err(e) => self.status = e,
                }
            }
        }
    }

    fn settings_permissions(&mut self, ui: &mut egui::Ui) {
        ui.heading("权限");
        ui.label("控制 Claude 执行工具时的确认行为。");
        ui.separator();

        let modes = ["default", "acceptEdits", "plan", "dontAsk", "bypassPermissions"];
        egui::ComboBox::from_label("默认模式")
            .selected_text(&self.perm_mode)
            .show_ui(ui, |ui| {
                for m in modes {
                    ui.selectable_value(&mut self.perm_mode, m.to_string(), m);
                }
            });

        ui.label("允许规则（每行一条）");
        ui.add(
            egui::TextEdit::multiline(&mut self.perm_allow)
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );
        ui.label("拒绝规则（每行一条）");
        ui.add(
            egui::TextEdit::multiline(&mut self.perm_deny)
                .desired_rows(4)
                .desired_width(f32::INFINITY),
        );

        ui.add_space(8.0);
        if ui.button("保存").clicked() {
            let p = service::Permissions {
                default_mode: self.perm_mode.clone(),
                allow: split_lines(&self.perm_allow),
                deny: split_lines(&self.perm_deny),
            };
            match service::save_permissions(&p) {
                Ok(()) => self.status = "已保存".into(),
                Err(e) => self.status = e,
            }
        }
    }

    fn settings_models(&mut self, ui: &mut egui::Ui) {
        ui.heading("模型");
        ui.label("模型只能在这里修改，会话内不提供切换。这些值存放在 settings.json 的 env 中。");
        ui.separator();

        for (key, label, value) in self.model_fields.iter_mut() {
            ui.horizontal(|ui| {
                ui.add_sized([120.0, 20.0], egui::Label::new(label.as_str()));
                ui.add(egui::TextEdit::singleline(value).desired_width(280.0));
                ui.label(egui::RichText::new(key.as_str()).small().weak());
            });
        }

        ui.add_space(8.0);
        if ui.button("保存").clicked() {
            let list: Vec<(String, String)> = self
                .model_fields
                .iter()
                .map(|(k, _, v)| (k.clone(), v.clone()))
                .collect();
            match service::save_models(&list) {
                Ok(()) => self.status = "已保存".into(),
                Err(e) => self.status = e,
            }
        }
    }

    fn settings_skills(&mut self, ui: &mut egui::Ui) {
        ui.heading("Skill");
        ui.label("扫描 ~/.claude/skills。开关只影响新会话。");
        ui.separator();

        let skills = match &self.conn {
            Some(c) => service::list_skills(c),
            None => vec![],
        };
        if skills.is_empty() {
            ui.label("未找到任何 Skill");
        }
        for s in skills {
            let mut on = s.enabled;
            if ui.checkbox(&mut on, &s.name).changed() {
                let r = match &self.conn {
                    Some(c) => service::toggle_skill(c, &s.name, on),
                    None => Err("数据库未打开".into()),
                };
                match r {
                    Ok(()) => self.status = "已更新".into(),
                    Err(e) => self.status = e,
                }
            }
            if !s.description.is_empty() {
                ui.label(egui::RichText::new(&s.description).small().weak());
            }
        }
    }

    fn settings_plugins(&mut self, ui: &mut egui::Ui) {
        ui.heading("插件");
        ui.label("已安装的插件。启用或禁用会影响下一次会话。");
        ui.separator();

        let plugins = service::list_plugins();
        if plugins.is_empty() {
            ui.label("没有已安装的插件，或 claude 命令不可用");
        }
        for p in plugins {
            let mut on = p.enabled;
            if ui.checkbox(&mut on, format!("{}  {}", p.name, p.version)).changed() {
                match service::toggle_plugin(&p.id, on) {
                    Ok(()) => self.status = "已更新".into(),
                    Err(e) => self.status = e,
                }
            }
        }
    }

    fn settings_marketplaces(&mut self, ui: &mut egui::Ui) {
        ui.heading("插件仓库");
        ui.label("添加第三方插件来源。");
        ui.separator();

        let mut remove: Option<String> = None;
        for m in service::list_marketplaces() {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.strong(&m.name);
                    ui.label(egui::RichText::new(&m.source).small().weak());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if m.builtin {
                        ui.label(egui::RichText::new("内置").small().weak());
                    } else if ui.button("移除").clicked() {
                        remove = Some(m.name.clone());
                    }
                });
            });
            ui.separator();
        }
        if let Some(name) = remove {
            match service::remove_marketplace(&name) {
                Ok(()) => self.status = "已移除仓库".into(),
                Err(e) => self.status = e,
            }
        }

        ui.add_space(8.0);
        ui.add(egui::TextEdit::singleline(&mut self.mkt_name).hint_text("仓库名称").desired_width(220.0));
        ui.add(
            egui::TextEdit::singleline(&mut self.mkt_repo)
                .hint_text("github.com/owner/repo")
                .desired_width(320.0),
        );
        if ui.button("添加仓库").clicked() {
            match service::add_marketplace(&self.mkt_name, &self.mkt_repo) {
                Ok(()) => {
                    self.mkt_name.clear();
                    self.mkt_repo.clear();
                    self.status = "已添加仓库".into();
                }
                Err(e) => self.status = e,
            }
        }
    }

    fn settings_update(&mut self, ui: &mut egui::Ui) {
        ui.heading("更新");
        ui.label("Rust 版是单文件程序，更新方式是下载新版本替换。");
        ui.separator();
        ui.label(format!("当前版本：v{VERSION}"));
        ui.label("发布地址：https://github.com/1696016910/CCWorkspace/releases");
        ui.add_space(8.0);
        ui.label("三个实现各有独立产物：");
        ui.label("  ClaudeWorkbench-Node-<版本>-x64.exe");
        ui.label("  ClaudeWorkbench-Go-<版本>-x64.exe");
        ui.label("  ClaudeWorkbench-Rust-<版本>-x64.exe");
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        if self.streaming {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }

        if self.show_settings {
            self.render_settings(ctx);
            return;
        }

        if let Some(err) = self.db_error.clone() {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("无法打开数据库");
                ui.label(err);
                if ui.button("重试").clicked() {
                    if let Ok(c) = db::open() {
                        self.conn = Some(c);
                        self.db_error = None;
                        self.reload();
                    }
                }
            });
            return;
        }

        egui::SidePanel::left("sidebar")
            .resizable(true)
            .default_width(250.0)
            .show(ctx, |ui| {
                // 工作区标题行：[工作区] ...... [＋]
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("工作区").small().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("＋").on_hover_text("新建工作区").clicked() {
                            self.show_new_ws = true;
                        }
                    });
                });

                let mut ws_action: Option<SessionAction> = None;
                let workspaces = self.workspaces.clone();
                egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                    for w in &workspaces {
                        let pin = if w.pinned_at.is_some() { "• " } else { "" };
                        let selected = self.current_ws.as_deref() == Some(&w.id);
                        let resp = ui
                            .selectable_label(selected, format!("{pin}{}", w.name))
                            .on_hover_text("右键：重命名 / 置顶 / 移除");
                        if resp.clicked() {
                            self.current_ws = Some(w.id.clone());
                            self.current_ses = None;
                            self.messages.clear();
                        }
                        resp.context_menu(|ui| {
                            if ui.button("重命名").clicked() {
                                ws_action = Some(SessionAction::Rename(w.id.clone(), w.name.clone()));
                                ui.close_menu();
                            }
                            let pin_label = if w.pinned_at.is_some() { "取消置顶" } else { "置顶" };
                            if ui.button(pin_label).clicked() {
                                ws_action = Some(SessionAction::Pin(w.id.clone(), w.pinned_at.is_none()));
                                ui.close_menu();
                            }
                            if ui.button("移除").clicked() {
                                ws_action = Some(SessionAction::Delete(w.id.clone(), w.name.clone()));
                                ui.close_menu();
                            }
                        });
                    }
                });
                // 工作区的重命名复用同一个对话框，靠 is_ws 区分
                match ws_action {
                    Some(SessionAction::Rename(id, name)) => self.renaming = Some((id, name, true)),
                    Some(SessionAction::Pin(id, pinned)) => {
                        if let Some(c) = &self.conn {
                            match service::pin_workspace(c, &id, pinned) {
                                Ok(()) => self.reload(),
                                Err(e) => self.status = e,
                            }
                        }
                    }
                    Some(SessionAction::Delete(id, name)) => {
                        if let Some(c) = &self.conn {
                            match service::remove_workspace(c, &id) {
                                Ok(()) => {
                                    self.current_ws = None;
                                    self.reload();
                                    self.status = format!("已移除「{name}」");
                                }
                                Err(e) => self.status = e,
                            }
                        }
                    }
                    _ => {}
                }

                ui.separator();
                // 会话标题行：[会话] ...... [＋]
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("会话").small().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("＋").on_hover_text("新建会话").clicked() {
                            if let (Some(conn), Some(ws)) = (&self.conn, self.current_ws.clone()) {
                                match service::create_session(conn, &ws) {
                                    Ok(_) => {
                                        self.reload();
                                        self.status = "已新建会话".into();
                                    }
                                    Err(e) => self.status = e,
                                }
                            } else {
                                self.status = "请先选择一个工作区".into();
                            }
                        }
                    });
                });

                let sessions: Vec<SessionSummary> = self
                    .sessions
                    .iter()
                    .filter(|s| {
                        self.current_ws.is_none() || self.current_ws.as_deref() == Some(&s.workspace_id)
                    })
                    .cloned()
                    .collect();

                let mut action: Option<SessionAction> = None;
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for s in &sessions {
                        let title = if s.title.is_empty() { "(新会话)" } else { &s.title };
                        let pin = if s.pinned_at.is_some() { "• " } else { "" };
                        let selected = self.current_ses.as_deref() == Some(&s.id);
                        let resp = ui
                            .selectable_label(selected, format!("{pin}{title}"))
                            .on_hover_text("右键：重命名 / 置顶 / 复制 / 删除");
                        if resp.clicked() {
                            self.current_ses = Some(s.id.clone());
                            self.load_messages();
                        }
                        resp.context_menu(|ui| {
                            if ui.button("重命名").clicked() {
                                action = Some(SessionAction::Rename(s.id.clone(), s.title.clone()));
                                ui.close_menu();
                            }
                            let pin_label = if s.pinned_at.is_some() { "取消置顶" } else { "置顶" };
                            if ui.button(pin_label).clicked() {
                                action = Some(SessionAction::Pin(s.id.clone(), s.pinned_at.is_none()));
                                ui.close_menu();
                            }
                            if ui.button("复制会话").clicked() {
                                action = Some(SessionAction::Duplicate(s.id.clone()));
                                ui.close_menu();
                            }
                            if ui.button("删除").clicked() {
                                action = Some(SessionAction::Delete(s.id.clone(), title.to_string()));
                                ui.close_menu();
                            }
                        });
                    }
                });
                match action {
                    Some(SessionAction::Rename(id, title)) => self.renaming = Some((id, title, false)),
                    Some(SessionAction::Pin(id, pinned)) => {
                        if let Some(c) = &self.conn {
                            match service::pin_session(c, &id, pinned) {
                                Ok(()) => {
                                    self.reload();
                                    self.status = if pinned { "已置顶".into() } else { "已取消置顶".into() };
                                }
                                Err(e) => self.status = e,
                            }
                        }
                    }
                    Some(SessionAction::Duplicate(id)) => {
                        if let Some(c) = &self.conn {
                            match service::duplicate_session(c, &id) {
                                Ok(s) => {
                                    self.reload();
                                    self.status = format!("已复制为「{}」", s.title);
                                }
                                Err(e) => self.status = e,
                            }
                        }
                    }
                    Some(SessionAction::Delete(id, title)) => {
                        self.pending_delete = Some((id, title, false))
                    }
                    None => {}
                }
            });

        egui::TopBottomPanel::bottom("composer")
            .exact_height(120.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.input)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("描述你想让它做的事"),
                );
                ui.horizontal(|ui| {
                    let send_enabled = !self.streaming && !self.input.trim().is_empty();
                    if ui.add_enabled(send_enabled, egui::Button::new("发送")).clicked() {
                        self.send(ctx);
                    }
                    if ui.add_enabled(self.streaming, egui::Button::new("停止")).clicked() {
                        self.streaming = false;
                        self.status = "已停止".into();
                    }
                    ui.separator();
                    if ui.selectable_label(self.show_context, "上下文").clicked() {
                        self.show_context = !self.show_context;
                    }
                    if ui.button("回收站").clicked() {
                        self.open_trash();
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(&self.status).small().weak());
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("设置").clicked() {
                    self.open_settings();
                }
                let title = self
                    .current_ses
                    .as_ref()
                    .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
                    .map(|s| if s.title.is_empty() { "(新会话)".to_string() } else { s.title.clone() })
                    .unwrap_or_else(|| "未选择会话".into());
                ui.heading(title);
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let mut preview: Option<String> = None;
                    for m in &self.messages {
                        let is_user = m.role == "user";
                        ui.horizontal_wrapped(|ui| {
                            if is_user {
                                ui.label(egui::RichText::new("你").small().weak());
                            } else {
                                ui.label(egui::RichText::new("C").small().weak().strong());
                            }
                            ui.label(&m.text);
                        });
                        if m.stopped {
                            ui.label(egui::RichText::new("■ 已停止生成").small().italics().weak());
                        }
                        if !m.tools.is_empty() {
                            let names: Vec<&str> = m.tools.iter().map(|t| t.name.as_str()).collect();
                            ui.label(
                                egui::RichText::new(format!("已处理 {} 步：{}", m.tools.len(), names.join(" · ")))
                                    .small()
                                    .weak(),
                            );
                            for t in &m.tools {
                                if looks_like_path(&t.target)
                                    && ui.link(&t.target).clicked()
                                {
                                    preview = Some(t.target.clone());
                                }
                            }
                        }
                        ui.add_space(6.0);
                    }
                    if let Some(p) = preview {
                        self.open_file(p);
                    }
                });
        });

        self.render_context_panel(ctx);
        self.render_delete_dialog(ctx);
        self.render_trash(ctx);
        self.render_preview(ctx);

        // ── 新建工作区弹窗 ──
        if self.show_new_ws {
            let mut open = true;
            egui::Window::new("新建工作区")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("项目目录");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_ws_path)
                            .desired_width(360.0)
                            .hint_text("绝对路径"),
                    );
                    ui.label("工作区名称（留空用目录名）");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_ws_name).desired_width(360.0),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let can = !self.new_ws_path.trim().is_empty();
                        if ui.add_enabled(can, egui::Button::new("创建")).clicked() {
                            if let Some(conn) = &self.conn {
                                match service::add_workspace(
                                    conn,
                                    &self.new_ws_path,
                                    &self.new_ws_name,
                                ) {
                                    Ok(()) => {
                                        self.reload();
                                        self.show_new_ws = false;
                                        self.new_ws_path.clear();
                                        self.new_ws_name.clear();
                                        self.status = "工作区已创建".into();
                                    }
                                    Err(e) => self.status = e,
                                }
                            }
                        }
                        if ui.button("取消").clicked() {
                            self.show_new_ws = false;
                        }
                    });
                });
            if !open {
                self.show_new_ws = false;
            }
        }
    }
}

/// 工具调用的目标是否像可预览的文件路径。
///
/// 命令、URL、搜索模式点开没有意义，只有带扩展名的路径才给入口。
fn looks_like_path(s: &str) -> bool {
    const BINARY: &[&str] = &[
        "png", "jpg", "jpeg", "gif", "webp", "ico", "bmp", "pdf", "zip", "gz", "7z", "rar", "exe",
        "dll", "so", "woff", "woff2", "ttf", "mp3", "mp4", "mov",
    ];
    if s.is_empty() || s.starts_with("http://") || s.starts_with("https://") || s.contains('\n') {
        return false;
    }
    if s.contains(' ') && !s.contains('/') && !s.contains('\\') {
        return false;
    }
    let ext = s.rsplit('.').next().unwrap_or("").to_lowercase();
    ext.len() >= 2 && ext.len() <= 6 && !BINARY.contains(&ext.as_str())
}

fn human_tokens(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

/// 把多行文本拆成去空的字符串列表。
fn split_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

#[allow(dead_code)]
fn unused(_: Arc<()>) {}
