//! Claude 工作台 · Rust 实现
//!
//! 与 Node 版读写同一个 SQLite 数据库、同一份 ~/.claude/settings.json，
//! 通过 spawn claude 子进程走 stdio JSON 协议对话。

mod claude;
mod db;
mod model;
mod icons;
mod markdown;
mod service;
mod theme;

use eframe::egui;
use egui::Color32;
use model::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

// 从 Cargo.toml 取版本：硬编码常量会和 Cargo.toml 各升各的，迟早对不上
const VERSION: &str = env!("CARGO_PKG_VERSION");

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
            theme::apply(&cc.egui_ctx, &theme::LIGHT);
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

    // 粗体候选：egui 的 FontId 不带字重，只能靠单独注册一个字体族来表达 **加粗**
    const BOLD_CANDIDATES: &[&str] = &[
        r"C:\Windows\Fonts\Dengb.ttf",
        r"C:\Windows\Fonts\msyhbd.ttf",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttf",
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

        // 注册粗体族；找不到粗体文件时退回常规字体，界面不会因此报错
        let bold_bytes = BOLD_CANDIDATES
            .iter()
            .find_map(|p| std::fs::read(p).ok());
        if let Some(b) = bold_bytes {
            fonts
                .font_data
                .insert("cjk-bold".to_owned(), egui::FontData::from_owned(b).into());
            fonts.families.insert(
                egui::FontFamily::Name("bold".into()),
                vec!["cjk-bold".to_owned(), "cjk".to_owned()],
            );
            theme::set_bold_family(true);
        }

        ctx.set_fonts(fonts);
        return;
    }
    eprintln!("未找到中文字体，中文可能显示为方框");
}

/// 会话右键菜单选中的动作。
enum SessionAction {
    /// 打开「更多」菜单：(id, 名称, 是否置顶, 是否工作区)
    Menu(String, String, bool, bool),
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

    // 右键/更多菜单的目标
    menu_target: Option<(String, String, bool, bool)>,

    // 外观与侧栏角标
    dark: bool,
    trash_count: usize,

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
            menu_target: None,
            dark: false,
            trash_count: 0,
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
                // 默认落在最近更新的会话上，启动就能看到内容而不是空白
                app.current_ses = app.sessions.first().map(|s| s.id.clone());
                app.current_ws = app
                    .current_ses
                    .as_ref()
                    .and_then(|id| app.sessions.iter().find(|s| &s.id == id))
                    .map(|s| s.workspace_id.clone())
                    .or_else(|| app.workspaces.first().map(|w| w.id.clone()));
                app.load_messages();
            }
            Err(e) => app.db_error = Some(e),
        }
        app
    }

    /// 当前调色板。所有绘制都从这里取，切暗色才能整体生效。
    fn pal(&self) -> theme::Pal {
        if self.dark {
            theme::DARK
        } else {
            theme::LIGHT
        }
    }

    fn reload(&mut self) {
        let Some(conn) = &self.conn else { return };
        self.workspaces = service::list_workspaces(conn);
        self.sessions = service::list_sessions(conn);
        self.trash_count = service::list_deleted_sessions(conn).len();
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
        self.refresh_trash();
        self.show_trash = true;
    }

    fn refresh_trash(&mut self) {
        self.trash = match &self.conn {
            Some(c) => service::list_deleted_sessions(c),
            None => vec![],
        };
        self.trash_count = self.trash.len();
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

    /// 「更多」菜单：工作区与会话共用。设计稿 .menu-pop 190px、9px 圆角。
    fn render_row_menu(&mut self, ctx: &egui::Context) {
        let Some((id, name, pinned, is_ws)) = self.menu_target.clone() else { return };
        let pal = self.pal();
        let mut open = true;
        let mut picked: Option<&'static str> = None;

        egui::Window::new("menu_row")
            .title_bar(false)
            .resizable(false)
            .fixed_size(egui::vec2(190.0, 0.0))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(
                egui::Frame::popup(&ctx.style())
                    .fill(pal.surface)
                    .stroke(egui::Stroke::new(1.0_f32, pal.line2))
                    .rounding(egui::Rounding::same(9.0))
                    .inner_margin(egui::Margin::same(5.0)),
            )
            .show(ctx, |ui| {
                let item = |ui: &mut egui::Ui, label: &str, danger: bool, icon: Option<FootIcon>| -> bool {
                    let h = 30.0;
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
                    if resp.hovered() {
                        let bg = if danger { pal.danger_bg } else { pal.bg_hover };
                        let stroke = if danger {
                            Some(egui::Stroke::new(1.0_f32, pal.danger_line))
                        } else {
                            None
                        };
                        theme::round_rect(ui, rect, 6, bg, stroke);
                    }
                    let col = if danger {
                        pal.danger_t
                    } else if resp.hovered() {
                        pal.t1
                    } else {
                        pal.t2
                    };
                    if let Some(ic) = icon {
                        let ir = egui::Rect::from_center_size(
                            egui::pos2(rect.left() + 18.0, rect.center().y),
                            egui::vec2(14.0, 14.0),
                        );
                        ic.draw(ui.painter(), ir, col);
                    }
                    ui.painter().text(
                        egui::pos2(rect.left() + if icon.is_some() { 28.0 } else { 10.0 }, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        theme::font(12.5),
                        col,
                    );
                    resp.clicked()
                };

                if item(ui, "重命名", false, None) {
                    picked = Some("rename");
                }
                let pin_label = if pinned { "取消置顶" } else { "置顶" };
                if item(ui, pin_label, false, Some(FootIcon::Pin)) {
                    picked = Some("pin");
                }
                if !is_ws && item(ui, "复制会话", false, Some(FootIcon::Copy)) {
                    picked = Some("dup");
                }
                if !is_ws {
                    ui.add_space(4.0);
                    let r = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 1.0),
                        egui::Sense::hover(),
                    );
                    ui.painter()
                        .hline(r.0.x_range(), r.0.center().y, egui::Stroke::new(1.0_f32, pal.line));
                    ui.add_space(4.0);
                }
                if item(ui, if is_ws { "移除工作区" } else { "删除会话" }, true, Some(FootIcon::Trash)) {
                    picked = Some("del");
                }
            });

        // 点窗口外就关
        if ctx.input(|i| i.pointer.any_click()) && !ctx.is_pointer_over_area() {
            open = false;
        }

        if let Some(action) = picked {
            self.menu_target = None;
            match (action, is_ws) {
                ("rename", _) => self.renaming = Some((id.clone(), name.clone(), is_ws)),
                ("pin", true) => {
                    if let Some(c) = &self.conn {
                        match service::pin_workspace(c, &id, !pinned) {
                            Ok(()) => self.reload(),
                            Err(e) => self.status = e,
                        }
                    }
                }
                ("pin", false) => {
                    if let Some(c) = &self.conn {
                        match service::pin_session(c, &id, !pinned) {
                            Ok(()) => self.reload(),
                            Err(e) => self.status = e,
                        }
                    }
                }
                ("dup", _) => {
                    if let Some(c) = &self.conn {
                        match service::duplicate_session(c, &id) {
                            Ok(_) => self.reload(),
                            Err(e) => self.status = e,
                        }
                    }
                }
                ("del", true) => {
                    self.pending_delete = Some((id.clone(), name.clone(), true))
                }
                ("del", false) => {
                    self.pending_delete = Some((id.clone(), name.clone(), false))
                }
                _ => {}
            }
        }
        if !open {
            self.menu_target = None;
        }
    }

    /// 删除确认弹窗。主界面和设置页共用。
    // ════════════════ 设计稿界面 ════════════════
    // 尺寸/颜色/间距逐项对照 prototype/20261027-界面原型.html

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(theme::SIDEBAR_W)
            .frame(egui::Frame::none().fill(pal.bg_bar))
            .show(ctx, |ui| {
                // .side-top padding:12px / .btn-new-ws height:36px radius:8px
                ui.add_space(12.0);
                let btn_rect = egui::Rect::from_min_size(
                    egui::pos2(ui.min_rect().left() + 12.0, ui.cursor().top()),
                    egui::vec2(theme::SIDEBAR_W - 24.0, 36.0),
                );
                let btn_resp = ui.allocate_rect(btn_rect, egui::Sense::click());
                let fill = if btn_resp.hovered() { pal.green_h } else { pal.green };
                theme::round_rect(ui, btn_rect, 8, fill, None);
                icons::plus(
                    ui.painter(),
                    egui::Rect::from_center_size(
                        egui::pos2(btn_rect.center().x - 40.0, btn_rect.center().y),
                        egui::vec2(15.0, 15.0),
                    ),
                    Color32::WHITE,
                );
                ui.painter().text(
                    egui::pos2(btn_rect.center().x + 6.0, btn_rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    "新建工作区",
                    theme::font(theme::F_NEW_WS),
                    Color32::WHITE,
                );
                if btn_resp.clicked() {
                    self.show_new_ws = true;
                }
                ui.add_space(12.0);

                let mut ws_action: Option<SessionAction> = None;
                let mut ses_action: Option<SessionAction> = None;
                let workspaces = self.workspaces.clone();
                let sessions = self.sessions.clone();
                let cur_ws = self.current_ws.clone();
                let cur_ses = self.current_ses.clone();

                // 底部固定栏：先算高度，把滚动区限制在剩余空间内，
                // 否则 ScrollArea 会吃掉整个侧栏，三个按钮被挤到窗口外
                let foot_h = 34.0 * 3.0 + 16.0 + 8.0;
                let list_h = (ui.available_height() - foot_h).max(80.0);
                egui::ScrollArea::vertical()
                    .max_height(list_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for w in &workspaces {
                            let expanded = cur_ws.as_deref() == Some(w.id.as_str());
                            let count =
                                sessions.iter().filter(|s| s.workspace_id == w.id).count();
                            // .ws-head padding:7px 8px; margin:0 8px; radius:7px
                            let row = egui::Rect::from_min_size(
                                egui::pos2(ui.min_rect().left() + 8.0, ui.cursor().top()),
                                egui::vec2(theme::SIDEBAR_W - 16.0, 30.0),
                            );
                            let resp = ui.allocate_rect(row, egui::Sense::click());
                            let hot = resp.hovered();
                            if hot {
                                theme::round_rect(ui, row, 7, pal.bg_hover, None);
                            }

                            // .ws-tri 14x14 —— 展开用下箭头，收起用右箭头
                            let tri = egui::Rect::from_center_size(
                                egui::pos2(row.left() + 13.0, row.center().y),
                                egui::vec2(14.0, 14.0),
                            );
                            if expanded {
                                icons::chevron_down(ui.painter(), tri, pal.t4);
                            } else {
                                icons::chevron_right(ui.painter(), tri, pal.t4);
                            }
                            // .ws-ico 15x15 文件夹
                            icons::folder(
                                ui.painter(),
                                egui::Rect::from_center_size(
                                    egui::pos2(row.left() + 31.0, row.center().y),
                                    egui::vec2(15.0, 15.0),
                                ),
                                pal.t3,
                            );

                            // 右侧按钮区先算好位置，名称的可用宽度要避开它
                            let add_rect = egui::Rect::from_center_size(
                                egui::pos2(row.right() - 13.0, row.center().y),
                                egui::vec2(24.0, 24.0),
                            );
                            let more_rect = egui::Rect::from_center_size(
                                egui::pos2(row.right() - 38.0, row.center().y),
                                egui::vec2(24.0, 24.0),
                            );
                            // 有会话时显示数量气泡，否则让位给名称
                            let cnt_rect = egui::Rect::from_center_size(
                                egui::pos2(row.right() - 38.0, row.center().y),
                                egui::vec2(22.0, 16.0),
                            );

                            // .ws-name 13px 500
                            let name_x = row.left() + 44.0;
                            let name_right = if hot { more_rect.left() } else { cnt_rect.left() };
                            let name_galley = theme::ellipsis(
                                ui,
                                &w.name,
                                theme::font(theme::F_WS_NAME),
                                pal.t1,
                                (name_right - name_x - 6.0).max(20.0),
                            );
                            ui.painter().galley(
                                egui::pos2(
                                    name_x,
                                    row.center().y - name_galley.size().y / 2.0,
                                ),
                                name_galley,
                                pal.t1,
                            );

                            let add_resp = ui.interact(
                                add_rect,
                                ui.id().with(("ws-add", w.id.as_str())),
                                egui::Sense::click(),
                            );
                            let more_resp = ui.interact(
                                more_rect,
                                ui.id().with(("ws-more", w.id.as_str())),
                                egui::Sense::click(),
                            );

                            if hot {
                                // .row-btn.add 与 .row-btn.more 只在 hover 时出现
                                icons::plus(ui.painter(), add_rect, pal.green);
                                icons::more(ui.painter(), more_rect, pal.t3);
                            } else if count > 0 {
                                // .ws-count 11px bg-soft radius:8px
                                let cr = egui::Rect::from_center_size(
                                    cnt_rect.center(),
                                    egui::vec2(
                                        (count.to_string().len() as f32 * 6.0 + 10.0).max(20.0),
                                        16.0,
                                    ),
                                );
                                theme::round_rect(ui, cr, 8, pal.bg_soft, None);
                                ui.painter().text(
                                    cr.center(),
                                    egui::Align2::CENTER_CENTER,
                                    count.to_string(),
                                    theme::font(theme::F_WS_COUNT),
                                    pal.t4,
                                );
                            }

                            if more_resp.clicked() {
                                ws_action = Some(SessionAction::Menu(
                                    w.id.clone(),
                                    w.name.clone(),
                                    w.pinned_at.is_some(),
                                    true,
                                ));
                            }
                            if add_resp.clicked() {
                                self.current_ws = Some(w.id.clone());
                                if let Some(c) = &self.conn {
                                    match service::create_session(c, &w.id) {
                                        Ok(_) => {
                                            self.reload();
                                            self.status = "已新建会话".into();
                                        }
                                        Err(e) => self.status = e,
                                    }
                                }
                            }
                            if resp.clicked() && !add_resp.clicked() && !more_resp.clicked() {
                                if expanded {
                                    self.current_ws = None;
                                    self.current_ses = None;
                                    self.messages.clear();
                                } else {
                                    self.current_ws = Some(w.id.clone());
                                    self.current_ses = None;
                                    self.messages.clear();
                                }
                            }

                            // .chat-item margin:1px 8px 1px 26px
                            if expanded {
                                let mine: Vec<SessionSummary> = sessions
                                    .iter()
                                    .filter(|s| s.workspace_id == w.id)
                                    .cloned()
                                    .collect();
                                if mine.is_empty() {
                                    // .ws-empty font-size:12px padding:10px 26px
                                    let r = egui::Rect::from_min_size(
                                        egui::pos2(ui.min_rect().left() + 26.0, ui.cursor().top()),
                                        egui::vec2(theme::SIDEBAR_W - 52.0, 20.0),
                                    );
                                    ui.painter().text(
                                        egui::pos2(r.left(), r.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        "暂无会话，点上方 ＋ 新建",
                                        theme::font(12.0),
                                        pal.t4,
                                    );
                                    ui.allocate_rect(r, egui::Sense::hover());
                                    ui.add_space(10.0);
                                }
                                for s in &mine {
                                    let item = egui::Rect::from_min_size(
                                        egui::pos2(ui.min_rect().left() + 26.0, ui.cursor().top()),
                                        egui::vec2(theme::SIDEBAR_W - 34.0, 30.0),
                                    );
                                    let on = cur_ses.as_deref() == Some(s.id.as_str());
                                    let r = ui.allocate_rect(item, egui::Sense::click());
                                    // .chat-item.on：bg green-soft，文字 green-500
                                    if on {
                                        theme::round_rect(ui, item, 7, pal.green_soft, None);
                                    } else if r.hovered() {
                                        theme::round_rect(ui, item, 7, pal.bg_hover, None);
                                    }

                                    let title = if s.title.is_empty() {
                                        "(新会话)"
                                    } else {
                                        s.title.as_str()
                                    };
                                    let col = if on { pal.green_t } else { pal.t1 };

                                    // 右侧：hover 显示更多，否则显示相对时间 + 状态点
                                    let dot_c = if s.running { pal.green } else { pal.line3 };
                                    let dot_x = item.right() - 10.0;
                                    let time_x = dot_x - 8.0;
                                    let more_ses = egui::Rect::from_center_size(
                                        egui::pos2(item.right() - 13.0, item.center().y),
                                        egui::vec2(24.0, 24.0),
                                    );
                                    let ses_more = ui.interact(
                                        more_ses,
                                        ui.id().with(("ses-more", s.id.as_str())),
                                        egui::Sense::click(),
                                    );

                                    let text_right = if (r.hovered() || ses_more.hovered())
                                        || on
                                    {
                                        more_ses.left()
                                    } else {
                                        time_x - 14.0
                                    };
                                    let g = theme::ellipsis(
                                        ui,
                                        title,
                                        theme::font(theme::F_CHAT),
                                        col,
                                        (text_right - item.left() - 8.0).max(20.0),
                                    );
                                    ui.painter().galley(
                                        egui::pos2(
                                            item.left() + 8.0,
                                            item.center().y - g.size().y / 2.0,
                                        ),
                                        g,
                                        col,
                                    );

                                    if r.hovered() || ses_more.hovered() || on {
                                        icons::more(ui.painter(), more_ses, pal.t3);
                                    } else {
                                        // .chat-time 10.5px
                                        ui.painter().text(
                                            egui::pos2(time_x, item.center().y),
                                            egui::Align2::RIGHT_CENTER,
                                            relative_time(&s.updated_at),
                                            theme::font(theme::F_CHAT_TIME),
                                            pal.t4,
                                        );
                                        ui.painter().circle_filled(
                                            egui::pos2(dot_x, item.center().y),
                                            3.0,
                                            dot_c,
                                        );
                                    }

                                    if ses_more.clicked() {
                                        ses_action = Some(SessionAction::Menu(
                                            s.id.clone(),
                                            title.to_string(),
                                            s.pinned_at.is_some(),
                                            false,
                                        ));
                                    } else if r.clicked() {
                                        self.current_ses = Some(s.id.clone());
                                        self.load_messages();
                                    }
                                    ui.add_space(2.0);
                                }
                            }
                            ui.add_space(4.0);
                        }
                        ui.add_space(8.0);
                    });

                // 工作区/会话的「更多」菜单
                if let Some(SessionAction::Menu(id, name, pinned, is_ws)) = ws_action.take() {
                    self.menu_target = Some((id, name, pinned, is_ws));
                }
                if let Some(SessionAction::Menu(id, name, pinned, is_ws)) = ses_action.take() {
                    self.menu_target = Some((id, name, pinned, is_ws));
                }

                // .side-foot padding:8px border-top
                theme::hline(ui, &pal);
                ui.add_space(8.0);
                let deleted_n = self.trash_count;
                if self.foot_btn(ui, &pal, FootIcon::Moon, "暗色模式", None) {
                    self.dark = !self.dark;
                    theme::apply(ctx, &self.pal());
                }
                if self.foot_btn(ui, &pal, FootIcon::Recycle, "回收站", Some(deleted_n)) {
                    self.open_trash();
                }
                if self.foot_btn(ui, &pal, FootIcon::Gear, "设置", None) {
                    self.open_settings();
                }
                ui.add_space(8.0);
            });
    }

    /// .foot-btn：height:34px padding:0 9px radius:7px font-size:12.5px
    fn foot_btn(
        &mut self,
        ui: &mut egui::Ui,
        pal: &theme::Pal,
        icon: FootIcon,
        label: &str,
        badge: Option<usize>,
    ) -> bool {
        let row = egui::Rect::from_min_size(
            egui::pos2(ui.min_rect().left() + 8.0, ui.cursor().top()),
            egui::vec2(theme::SIDEBAR_W - 16.0, 34.0),
        );
        let r = ui.allocate_rect(row, egui::Sense::click());
        if r.hovered() {
            theme::round_rect(ui, row, 7, pal.bg_hover, None);
        }
        let ic = egui::Rect::from_center_size(
            egui::pos2(row.left() + 17.0, row.center().y),
            egui::vec2(15.0, 15.0),
        );
        icon.draw(ui.painter(), ic, if r.hovered() { pal.t1 } else { pal.t3 });
        ui.painter().text(
            egui::pos2(row.left() + 33.0, row.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            theme::font(theme::F_FOOT),
            if r.hovered() { pal.t1 } else { pal.t2 },
        );
        if let Some(n) = badge {
            if n > 0 {
                let br = egui::Rect::from_center_size(
                    egui::pos2(row.right() - 18.0, row.center().y),
                    egui::vec2(22.0, 16.0),
                );
                theme::round_rect(ui, br, 8, pal.bg_soft, None);
                ui.painter().text(
                    br.center(),
                    egui::Align2::CENTER_CENTER,
                    n.to_string(),
                    theme::font(10.5),
                    pal.t4,
                );
            }
        }
        r.clicked()
    }

    /// .composer-wrap padding:14px 28px 16px + .composer max-width:720px radius:14px
    fn render_composer(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        egui::TopBottomPanel::bottom("composer")
            .frame(egui::Frame::none().fill(pal.bg))
            .show(ctx, |ui| {
                ui.add_space(14.0);
                let avail = ui.available_width();
                let box_w = theme::THREAD_MAX_W.min(avail - 56.0);
                let left = ui.min_rect().left() + (avail - box_w) / 2.0;
                let top = ui.cursor().top();

                let text_h = 52.0;
                let total_h = text_h + theme::COMPOSER_BAR_H;
                let rect =
                    egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(box_w, total_h));
                theme::round_rect(
                    ui,
                    rect,
                    14,
                    pal.surface,
                    Some(egui::Stroke::new(1.0_f32, pal.line2)),
                );

                let mut send_now = false;
                let text_rect = egui::Rect::from_min_size(
                    egui::pos2(left + 16.0, top + 12.0),
                    egui::vec2(box_w - 32.0, text_h - 20.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect), |ui| {
                    let te = egui::TextEdit::multiline(&mut self.input)
                        .frame(false)
                        .desired_width(f32::INFINITY)
                        .font(theme::font(theme::F_BUBBLE))
                        .hint_text(
                            egui::RichText::new("描述你想让它做的事，或输入 / 调用命令与 Skill...")
                                .font(theme::font(theme::F_BUBBLE))
                                .color(pal.t4),
                        );
                    let r = ui.add_sized([text_rect.width(), text_rect.height()], te);
                    if r.has_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift)
                    {
                        send_now = true;
                    }
                });

                let bar_y = top + text_h;
                let plus_rect = egui::Rect::from_min_size(
                    egui::pos2(left + 10.0, bar_y + 7.0),
                    egui::vec2(28.0, 28.0),
                );
                icons::plus(ui.painter(), plus_rect, pal.t3);

                let send_rect = egui::Rect::from_min_size(
                    egui::pos2(left + box_w - 40.0, bar_y + 6.0),
                    egui::vec2(30.0, 30.0),
                );
                let can = !self.streaming && !self.input.trim().is_empty();
                let hovering = ui.rect_contains_pointer(send_rect);
                let send_fill = if self.streaming {
                    pal.red
                } else if can {
                    if hovering {
                        pal.green_h
                    } else {
                        pal.green
                    }
                } else {
                    pal.green.gamma_multiply(0.4)
                };
                theme::round_rect(ui, send_rect, 8, send_fill, None);
                let send_ic = egui::Rect::from_center_size(send_rect.center(), egui::vec2(15.0, 15.0));
                if self.streaming {
                    icons::stop(ui.painter(), send_ic, Color32::WHITE);
                } else {
                    icons::arrow_up(ui.painter(), send_ic, Color32::WHITE);
                }
                let send_resp = ui.interact(send_rect, ui.id().with("send"), egui::Sense::click());
                if send_resp.clicked() {
                    if self.streaming {
                        self.streaming = false;
                        self.status = "已停止".into();
                    } else if can {
                        send_now = true;
                    }
                }

                // .ctx-bar 64x4 + 百分比
                let pct = self
                    .current_ses
                    .as_ref()
                    .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
                    .map(|s| s.context_percent)
                    .unwrap_or(0.0);
                let bar_rect = egui::Rect::from_min_size(
                    egui::pos2(send_rect.left() - 110.0, bar_y + 19.0),
                    egui::vec2(64.0, 4.0),
                );
                let w_pct = bar_rect.width() * (pct as f32 / 100.0);
                theme::round_rect(ui, bar_rect, 2, pal.bg_soft, None);
                if w_pct > 1.0 {
                    let w = w_pct;
                    theme::round_rect(
                        ui,
                        egui::Rect::from_min_size(bar_rect.min, egui::vec2(w, 4.0)),
                        2,
                        pal.green,
                        None,
                    );
                }
                ui.painter().text(
                    egui::pos2(bar_rect.left() - 8.0, bar_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    format!("{}%", pct.round() as i64),
                    theme::font(11.5),
                    pal.t3,
                );

                // .cm-hint 11px 居中
                ui.allocate_rect(
                    egui::Rect::from_min_size(egui::pos2(left, top), egui::vec2(box_w, total_h)),
                    egui::Sense::hover(),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space((avail - 200.0) / 2.0);
                    ui.label(
                        egui::RichText::new("Enter 发送 · Shift + Enter 换行")
                            .font(theme::font(theme::F_HINT))
                            .color(pal.t4),
                    );
                });
                ui.add_space(16.0);

                if send_now {
                    self.send(ctx);
                }
            });
    }

    /// .main-head 48px + .thread max-width:720px
    fn render_thread(&mut self, ctx: &egui::Context) {
        let pal = self.pal();
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg))
            .show(ctx, |ui| {
                let head = egui::Rect::from_min_size(
                    ui.min_rect().min,
                    egui::vec2(ui.available_width(), theme::HEAD_H),
                );
                ui.painter()
                    .hline(head.x_range(), head.bottom(), egui::Stroke::new(1.0_f32, pal.line));

                let ses = self
                    .current_ses
                    .as_ref()
                    .and_then(|id| self.sessions.iter().find(|s| &s.id == id))
                    .cloned();
                let title = ses
                    .as_ref()
                    .map(|s| {
                        if s.title.is_empty() {
                            "(新会话)".to_string()
                        } else {
                            s.title.clone()
                        }
                    })
                    .unwrap_or_else(|| "未选择会话".into());
                let ws_name = ses
                    .as_ref()
                    .and_then(|s| self.workspaces.iter().find(|w| w.id == s.workspace_id))
                    .map(|w| w.name.clone())
                    .unwrap_or_default();

                // .mh-title 14px 500，从 left+24 开始
                let title_x = head.left() + 24.0;
                let title_galley = theme::ellipsis(
                    ui,
                    &title,
                    theme::font(theme::F_MH_TITLE),
                    pal.t1,
                    head.width() - 200.0,
                );
                let title_w = title_galley.size().x;
                ui.painter().galley(
                    egui::pos2(title_x, head.center().y - title_galley.size().y / 2.0),
                    title_galley,
                    pal.t1,
                );

                // .mh-ws：文件夹图标 + 工作区名，11.5px，紧跟在标题后
                if !ws_name.is_empty() {
                    let wx = title_x + title_w + 10.0;
                    let ic = egui::Rect::from_center_size(
                        egui::pos2(wx + 5.5, head.center().y),
                        egui::vec2(11.0, 11.0),
                    );
                    icons::folder(ui.painter(), ic, pal.t4);
                    ui.painter().text(
                        egui::pos2(wx + 16.0, head.center().y),
                        egui::Align2::LEFT_CENTER,
                        &ws_name,
                        theme::font(theme::F_MH_WS),
                        pal.t4,
                    );
                }

                // 右侧三个图标按钮：从右往左 复制 / 回收站 / 更多
                let mut x = head.right() - 24.0;
                let mut icon_btns: Vec<(&str, egui::Rect)> = vec![];
                for id in ["copy", "del"] {
                    let br = egui::Rect::from_center_size(
                        egui::pos2(x - 14.0, head.center().y),
                        egui::vec2(28.0, 28.0),
                    );
                    icon_btns.push((id, br));
                    x -= 32.0;
                }
                for (id, br) in &icon_btns {
                    let r = ui.interact(*br, ui.id().with(("mh", *id)), egui::Sense::click());
                    if r.hovered() {
                        theme::round_rect(ui, *br, 6, pal.bg_hover, None);
                    }
                    let col = if r.hovered() { pal.t1 } else { pal.t3 };
                    let ic = egui::Rect::from_center_size(br.center(), egui::vec2(15.0, 15.0));
                    match *id {
                        "copy" => icons::copy(ui.painter(), ic, col),
                        _ => icons::trash(ui.painter(), ic, col),
                    }
                    if r.clicked() {
                        match *id {
                            "copy" => match &ses {
                                Some(s) => {
                                    if let Some(c) = &self.conn {
                                        match service::duplicate_session(c, &s.id) {
                                            Ok(_) => self.reload(),
                                            Err(e) => self.status = e,
                                        }
                                    }
                                }
                                None => self.status = "请先选择一个会话".into(),
                            },
                            _ => match &ses {
                                Some(s) => {
                                    let t = if s.title.is_empty() {
                                        "(新会话)".to_string()
                                    } else {
                                        s.title.clone()
                                    };
                                    self.pending_delete = Some((s.id.clone(), t, false));
                                }
                                None => self.status = "请先选择一个会话".into(),
                            },
                        }
                    }
                }

                let body_top = head.bottom();
                let avail = ui.available_width();
                let inner_w = theme::THREAD_MAX_W.min(avail - 56.0);
                let left = ui.min_rect().left() + (avail - inner_w) / 2.0;
                let list_rect = egui::Rect::from_min_max(
                    egui::pos2(ui.min_rect().left(), body_top),
                    egui::pos2(ui.max_rect().right(), ui.max_rect().bottom()),
                );
                let mut preview: Option<String> = None;
                ui.scope_builder(egui::UiBuilder::new().max_rect(list_rect), |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.add_space(26.0);
                            let msgs = self.messages.clone();
                            for m in &msgs {
                                if m.role == "user" {
                                    draw_user_bubble(ui, &pal, m, left, inner_w);
                                } else {
                                    draw_ai_message(ui, &pal, m, left, inner_w, &mut preview);
                                }
                            }
                            ui.add_space(32.0);
                        });
                });
                ui.allocate_rect(list_rect, egui::Sense::hover());
                if let Some(p) = preview {
                    self.open_file(p);
                }
            });
    }

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

        self.render_row_menu(ctx);
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

        self.render_sidebar(ctx);
        self.render_composer(ctx);
        self.render_thread(ctx);

        self.render_row_menu(ctx);
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

/// 侧栏底部三个按钮的图标种类。
#[derive(Clone, Copy)]
enum FootIcon {
    Moon,
    Recycle,
    Gear,
    Pin,
    Copy,
    Trash,
}

impl FootIcon {
    fn draw(self, p: &egui::Painter, r: egui::Rect, c: Color32) {
        match self {
            FootIcon::Moon => icons::moon(p, r, c),
            FootIcon::Recycle => icons::recycle(p, r, c),
            FootIcon::Gear => icons::gear(p, r, c),
            FootIcon::Pin => icons::pin(p, r, c),
            FootIcon::Copy => icons::copy(p, r, c),
            FootIcon::Trash => icons::trash(p, r, c),
        }
    }
}

/// 把 ISO 时间转成设计稿里的相对时间：“5分钟前”“2小时前”“昨天”。
///
/// 解析失败时返回空串，不把原始 ISO 串怼到界面上。
fn relative_time(iso: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let Ok(t) = parse_iso(iso) else { return String::new() };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = now - t;
    match diff {
        d if d < 60 => "刚刚".into(),
        d if d < 3600 => format!("{}分钟前", d / 60),
        d if d < 86400 => format!("{}小时前", d / 3600),
        d if d < 172800 => "昨天".into(),
        d if d < 2592000 => format!("{}天前", d / 86400),
        d => format!("{}个月前", d / 2592000),
    }
}

/// 解析 `2026-10-27T08:34:43.404Z` 这种 ISO 8601 → unix 秒。
fn parse_iso(s: &str) -> Result<i64, ()> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return Err(());
    }
    let num = |a: usize, n: usize| -> i64 {
        s.get(a..a + n).and_then(|v| v.parse::<i64>().ok()).unwrap_or(0)
    };
    let (y, mo, d) = (num(0, 4), num(5, 2), num(8, 2));
    let (h, mi, sec) = (num(11, 2), num(14, 2), num(17, 2));

    // 公历转 days since epoch
    let y0 = if mo <= 2 { y - 1 } else { y };
    let era = if y0 >= 0 { y0 } else { y0 - 399 } / 400;
    let yoe = y0 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;

    Ok(days * 86400 + h * 3600 + mi * 60 + sec)
}

/// 用户气泡：.msg-user .bubble radius:14px 14px 4px 14px bg-soft max-width:78%
fn draw_user_bubble(
    ui: &mut egui::Ui,
    pal: &theme::Pal,
    m: &ChatMessage,
    left: f32,
    inner_w: f32,
) {
    let max_w = inner_w * 0.78;
    let galley = ui.fonts(|f| {
        f.layout(
            m.text.clone(),
            theme::font(theme::F_BUBBLE),
            pal.t1,
            max_w - 30.0,
        )
    });
    let pad = egui::vec2(15.0, 10.0);
    let size = egui::vec2(
        (galley.size().x + pad.x * 2.0).min(max_w),
        galley.size().y + pad.y * 2.0,
    );
    let rect = egui::Rect::from_min_size(
        egui::pos2(left + inner_w - size.x, ui.cursor().top()),
        size,
    );
    theme::round_rect(ui, rect, 14, pal.bg_soft, None);
    // 右下角收成 4px：盖一个方角
    ui.painter().rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(rect.right() - 8.0, rect.bottom() - 8.0),
            rect.right_bottom(),
        ),
        0.0,
        pal.bg_soft,
    );
    ui.painter().galley(rect.min + pad, galley, pal.t1);
    ui.allocate_rect(rect, egui::Sense::hover());
    ui.add_space(26.0);
}

/// 助手消息：.a-avatar 26px radius:8px + .ai-role + .ai-text
fn draw_ai_message(
    ui: &mut egui::Ui,
    pal: &theme::Pal,
    m: &ChatMessage,
    left: f32,
    inner_w: f32,
    preview: &mut Option<String>,
) {
    let top = ui.cursor().top();
    let av = egui::Rect::from_min_size(egui::pos2(left, top + 1.0), egui::vec2(26.0, 26.0));
    theme::round_rect(ui, av, 8, pal.green_soft, None);
    ui.painter().text(
        av.center(),
        egui::Align2::CENTER_CENTER,
        "C",
        theme::font(12.0),
        pal.green,
    );

    let x = left + 38.0;
    let w = inner_w - 38.0;

    ui.painter().text(
        egui::pos2(x, top + 9.0),
        egui::Align2::LEFT_CENTER,
        "Claude",
        theme::font(theme::F_ROLE),
        pal.t2,
    );
    let mut y = top + 24.0;

    for t in &m.tools {
        let g = theme::ellipsis(
            ui,
            &format!("{}  {}", t.name, t.target),
            theme::font(11.5),
            pal.t3,
            w,
        );
        ui.painter().galley(egui::pos2(x, y), g, pal.t3);
        let row = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, 18.0));
        if looks_like_path(&t.target) {
            let r = ui.interact(row, ui.id().with(("tool", t.target.as_str())), egui::Sense::click());
            if r.hovered() {
                ui.painter().text(
                    egui::pos2(x, y),
                    egui::Align2::LEFT_TOP,
                    &t.target,
                    theme::font(11.5),
                    pal.green_t,
                );
            }
            if r.clicked() {
                *preview = Some(t.target.clone());
            }
        }
        y += 18.0;
    }
    if !m.tools.is_empty() {
        y += 6.0;
    }

    // 正文按 Markdown 排版：段落/行内代码/围栏代码块/列表，
    // 纯文本会让设计稿里的代码块和列表完全丢失
    y += markdown::draw(ui, pal, &m.text, x, y, w);

    if m.stopped {
        y += 10.0;
        let g = ui.fonts(|f| {
            f.layout_no_wrap(
                "■ 已停止生成".to_string(),
                theme::font(theme::F_STOPPED),
                pal.t3,
            )
        });
        let r = egui::Rect::from_min_size(
            egui::pos2(x, y),
            egui::vec2(g.size().x + 22.0, g.size().y + 14.0),
        );
        theme::round_rect(ui, r, 7, pal.bg_soft, None);
        ui.painter()
            .galley(egui::pos2(r.left() + 11.0, r.top() + 7.0), g, pal.t3);
        y = r.bottom();
    }

    let row = egui::Rect::from_min_size(
        egui::pos2(left, top),
        egui::vec2(inner_w, (y - top).max(26.0)),
    );
    ui.allocate_rect(row, egui::Sense::hover());
    ui.add_space(26.0);
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
