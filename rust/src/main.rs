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

/// 后台线程回传的消息。
enum Msg {
    Delta(String),
    Done(String, bool),
    Error(String),
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
            skills: service::enabled_skill_names(),
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
                claude::Event::ClaudeSession(_cid) => {}
                claude::Event::Context { total, max, percent, .. } => {
                    // 上下文写入由主线程在 Done 之后处理
                    let _ = (total, max, percent);
                }
                claude::Event::Commands(_) => {}
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
        while let Ok(m) = rx.try_recv() {
            match m {
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
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        if self.streaming {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
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
                if ui.button("＋ 新建工作区").clicked() {
                    self.show_new_ws = true;
                }
                ui.separator();
                ui.label(egui::RichText::new("工作区").small().weak());

                let workspaces = self.workspaces.clone();
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for w in &workspaces {
                        let pin = if w.pinned_at.is_some() { "• " } else { "" };
                        let selected = self.current_ws.as_deref() == Some(&w.id);
                        if ui
                            .selectable_label(selected, format!("{pin}{}", w.name))
                            .clicked()
                        {
                            self.current_ws = Some(w.id.clone());
                            self.current_ses = None;
                            self.messages.clear();
                        }
                    }
                });

                if ui.button("删除工作区").clicked() {
                    if let Some(id) = self.current_ws.clone() {
                        let name = self
                            .workspaces
                            .iter()
                            .find(|w| w.id == id)
                            .map(|w| w.name.clone())
                            .unwrap_or_default();
                        self.pending_delete = Some((id, name, true));
                    } else {
                        self.status = "请先选择一个工作区".into();
                    }
                }

                ui.separator();
                if ui.button("＋ 新建会话").clicked() {
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
                if ui.button("删除会话").clicked() {
                    if let Some(id) = self.current_ses.clone() {
                        let title = self
                            .sessions
                            .iter()
                            .find(|s| s.id == id)
                            .map(|s| if s.title.is_empty() { "(新会话)".to_string() } else { s.title.clone() })
                            .unwrap_or_default();
                        self.pending_delete = Some((id, title, false));
                    } else {
                        self.status = "请先选择一个会话".into();
                    }
                }
                ui.label(egui::RichText::new("会话").small().weak());

                let sessions: Vec<SessionSummary> = self
                    .sessions
                    .iter()
                    .filter(|s| {
                        self.current_ws.is_none() || self.current_ws.as_deref() == Some(&s.workspace_id)
                    })
                    .cloned()
                    .collect();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for s in &sessions {
                        let title = if s.title.is_empty() { "(新会话)" } else { &s.title };
                        let pin = if s.pinned_at.is_some() { "• " } else { "" };
                        let selected = self.current_ses.as_deref() == Some(&s.id);
                        if ui
                            .selectable_label(selected, format!("{pin}{title}"))
                            .clicked()
                        {
                            self.current_ses = Some(s.id.clone());
                            self.load_messages();
                        }
                    }
                });
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
                    ui.label(egui::RichText::new(&self.status).small().weak());
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
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
                        }
                        ui.add_space(6.0);
                    }
                });
        });

        // ── 删除确认弹窗 ──
        if let Some((id, name, is_ws)) = self.pending_delete.clone() {
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
                                        self.status = if is_ws { "已删除工作区".into() } else { "已移到回收站".into() };
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

#[allow(dead_code)]
fn unused(_: Arc<()>) {}
