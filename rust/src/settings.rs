//! 设置页：按 Node 版 SettingsPage.vue 的规范复刻。
//!
//! 关键尺寸：顶部栏 52px、左导航 232px、内容区 max-width 680px、
//! 导航项 34px 高 / 7px 圆角 / 选中 green-soft + green-t。

use crate::theme::{self, Pal};
use crate::{icons, service, App};
use egui::{Color32, Pos2, Rect, Stroke};

/// 导航分组：Node 版把 8 个分区按用途分了四组。
const GROUPS: &[(&str, &[(&str, &str, &str)])] = &[
    ("工作", &[("ws", "工作区", "folder")]),
    (
        "配置",
        &[
            ("env", "环境变量", "list"),
            ("perm", "权限", "shield"),
            ("model", "模型", "box"),
        ],
    ),
    (
        "扩展",
        &[
            ("skills", "Skill", "spark"),
            ("plugins", "插件", "grid"),
            ("market", "插件仓库", "globe"),
        ],
    ),
    ("关于", &[("update", "更新", "up")]),
];

impl App {
    pub fn render_settings(&mut self, ctx: &egui::Context) {
        let pal = self.pal();

        egui::TopBottomPanel::top("settings_bar")
            .exact_height(52.0)
            .frame(egui::Frame::none().fill(pal.bg_bar))
            .show(ctx, |ui| {
                let bar = egui::Rect::from_min_size(
                    ui.min_rect().min,
                    egui::vec2(ui.available_width(), 52.0),
                );
                ui.painter().hline(
                    bar.x_range(),
                    bar.bottom(),
                    Stroke::new(1.0_f32, pal.line),
                );

                // 返回按钮：32px 高、8px 圆角、1px 边框
                let back_rect = egui::Rect::from_min_size(
                    egui::pos2(bar.left() + 16.0, bar.top() + 10.0),
                    egui::vec2(72.0, 32.0),
                );
                let back = ui.interact(back_rect, ui.id().with("st-back"), egui::Sense::click());
                if back.hovered() {
                    theme::round_rect(ui, back_rect, 8, pal.bg_hover, None);
                }
                theme::round_rect(
                    ui,
                    back_rect,
                    8,
                    Color32::TRANSPARENT,
                    Some(Stroke::new(1.0_f32, if back.hovered() { pal.line3 } else { pal.line2 })),
                );
                icons::back(
                    ui.painter(),
                    egui::Rect::from_center_size(
                        egui::pos2(back_rect.left() + 16.0, back_rect.center().y),
                        egui::vec2(15.0, 15.0),
                    ),
                    pal.t1,
                );
                ui.painter().text(
                    egui::pos2(back_rect.left() + 28.0, back_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "返回",
                    theme::font(13.0),
                    pal.t1,
                );
                if back.clicked() {
                    self.show_settings = false;
                }

                // 标题 14px / 600
                ui.painter().text(
                    egui::pos2(back_rect.right() + 14.0, bar.center().y),
                    egui::Align2::LEFT_CENTER,
                    "设置",
                    theme::font_bold(14.0),
                    pal.t1,
                );

                // 右上关闭 ✕
                let close_rect = egui::Rect::from_center_size(
                    egui::pos2(bar.right() - 32.0, bar.center().y),
                    egui::vec2(32.0, 32.0),
                );
                let close = ui.interact(close_rect, ui.id().with("st-close"), egui::Sense::click());
                if close.hovered() {
                    theme::round_rect(ui, close_rect, 8, pal.bg_hover, None);
                }
                icons::close(
                    ui.painter(),
                    egui::Rect::from_center_size(close_rect.center(), egui::vec2(15.0, 15.0)),
                    if close.hovered() { pal.t1 } else { pal.t3 },
                );
                if close.clicked() {
                    self.show_settings = false;
                }
            });

        // 左导航 232px
        egui::SidePanel::left("settings_nav")
            .resizable(false)
            .exact_width(232.0)
            .frame(egui::Frame::none().fill(pal.bg_bar))
            .show(ctx, |ui| {
                let nav_rect = egui::Rect::from_min_size(
                    ui.min_rect().min,
                    egui::vec2(232.0, ui.available_height()),
                );
                ui.painter().vline(
                    nav_rect.right(),
                    nav_rect.y_range(),
                    Stroke::new(1.0_f32, pal.line),
                );

                let mut y = nav_rect.top() + 14.0;
                let mut picked: Option<&str> = None;
                for (gi, (group, items)) in GROUPS.iter().enumerate() {
                    // .st-grp 11px t4，首组 padding-top 2px
                    let grp_y = if gi == 0 { y } else { y + 12.0 };
                    ui.painter().text(
                        Pos2::new(nav_rect.left() + 20.0, grp_y + 6.0),
                        egui::Align2::LEFT_CENTER,
                        *group,
                        theme::font(11.0),
                        pal.t4,
                    );
                    let mut iy = grp_y + 22.0;

                    for (id, label, icon) in items.iter() {
                        // .st-item 34px 高 / 7px 圆角 / padding 0 10px
                        let item = egui::Rect::from_min_size(
                            Pos2::new(nav_rect.left() + 10.0, iy),
                            egui::vec2(212.0, 34.0),
                        );
                        let on = self.settings_tab_id() == *id;
                        let r = ui.interact(
                            item,
                            ui.id().with(("st-item", *id)),
                            egui::Sense::click(),
                        );
                        if on {
                            theme::round_rect(ui, item, 7, pal.green_soft, None);
                        } else if r.hovered() {
                            theme::round_rect(ui, item, 7, pal.bg_hover, None);
                        }
                        let col = if on {
                            pal.green_t
                        } else if r.hovered() {
                            pal.t1
                        } else {
                            pal.t2
                        };
                        icons::nav(
                            ui.painter(),
                            egui::Rect::from_center_size(
                                Pos2::new(item.left() + 18.0, item.center().y),
                                egui::vec2(15.0, 15.0),
                            ),
                            col,
                            icon,
                        );
                        ui.painter().text(
                            Pos2::new(item.left() + 34.0, item.center().y),
                            egui::Align2::LEFT_CENTER,
                            *label,
                            if on {
                                theme::font_bold(13.0)
                            } else {
                                theme::font(13.0)
                            },
                            col,
                        );
                        if r.clicked() {
                            picked = Some(id);
                        }
                        iy += 34.0;
                    }
                    y = iy;
                }
                if let Some(id) = picked {
                    self.set_settings_tab(id);
                    self.load_settings();
                }
            });

        // 内容区：max-width 680px，padding 28px 32px 40px
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg))
            .show(ctx, |ui| {
                let avail = ui.available_width();
                // 卡片型分区（Skill/插件/仓库）用 Node 版的 wide 布局：占满宽度，
                // 否则 auto-fill 只能排两列
                let wide = matches!(self.settings_tab_id(), "skills" | "plugins" | "market");
                let inner_w = if wide {
                    (avail - 64.0).max(320.0)
                } else {
                    680.0_f32.min(avail - 64.0)
                };
                let left = ui.min_rect().left() + (avail - inner_w) / 2.0;

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(28.0);
                        let top = ui.cursor().top();
                        ui.scope_builder(
                            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                                Pos2::new(left, top),
                                egui::vec2(inner_w, ui.available_height()),
                            )),
                            |ui| match self.settings_tab_id() {
                                "ws" => self.settings_workspaces(ui),
                                "env" => self.settings_env(ui),
                                "perm" => self.settings_permissions(ui),
                                "model" => self.settings_models(ui),
                                "skills" => self.settings_skills(ui),
                                "plugins" => self.settings_plugins(ui),
                                "market" => self.settings_marketplaces(ui),
                                _ => self.settings_update(ui),
                            },
                        );
                        ui.add_space(40.0);
                    });
            });

        self.render_row_menu(ctx);
        self.render_confirm(ctx);
        self.render_rename_dialog(ctx);
        self.render_trash(ctx);
        self.render_preview(ctx);
    }

    pub fn settings_tab_id(&self) -> &'static str {
        const IDS: &[&str] = &["ws", "env", "perm", "model", "skills", "plugins", "market", "update"];
        IDS.get(self.settings_tab).copied().unwrap_or("ws")
    }

    pub fn set_settings_tab(&mut self, id: &str) {
        const IDS: &[&str] = &["ws", "env", "perm", "model", "skills", "plugins", "market", "update"];
        self.settings_tab = IDS.iter().position(|x| *x == id).unwrap_or(0);
    }
}

// ── 共享控件 ──

/// .st-h1x：19px / 600 / margin-bottom 6px
pub(super) fn settings_h1(ui: &mut egui::Ui, pal: &Pal, text: &str) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.0), egui::Sense::hover());
    ui.painter().text(
        egui::pos2(rect.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        theme::font_bold(19.0),
        pal.t1,
    );
}

/// .st-subx：12.5px t3 / line-height 1.65 / margin-bottom 24px
pub(super) fn settings_sub(ui: &mut egui::Ui, pal: &Pal, text: &str) {
    let w = ui.available_width();
    let galley = ui.fonts(|f| {
        f.layout(
            text.to_owned(),
            theme::font(12.5),
            pal.t3,
            w,
        )
    });
    let h = galley.size().y;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    ui.painter()
        .galley(egui::pos2(rect.left(), rect.top()), galley, pal.t3);
    ui.add_space(24.0);
}

/// .st-cardx：1px line-2 边框 + 10px 圆角
pub(super) fn card_frame(pal: &Pal) -> egui::Frame {
    egui::Frame::none()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0_f32, pal.line2))
        .rounding(egui::Rounding::same(10.0))
        .inner_margin(egui::Margin::ZERO)
}

/// 同 card_row，但可指定行高。
pub(super) fn card_row_h(
    ui: &mut egui::Ui,
    pal: &Pal,
    last: bool,
    h: f32,
    draw: impl FnOnce(&mut egui::Ui, Rect),
) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    if resp.hovered() {
        ui.painter().rect_filled(rect, egui::Rounding::ZERO, pal.surface2);
    }
    if !last {
        ui.painter()
            .hline(rect.x_range(), rect.bottom(), Stroke::new(1.0_f32, pal.line));
    }
    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.top() + 12.0),
        egui::pos2(rect.right() - 16.0, rect.bottom() - 12.0),
    );
    draw(ui, inner);
}

/// .st-rowx：padding 14px 16px，底部 1px 分隔
pub(super) fn card_row(
    ui: &mut egui::Ui,
    pal: &Pal,
    last: bool,
    draw: impl FnOnce(&mut egui::Ui, Rect),
) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, 64.0), egui::Sense::hover());
    if resp.hovered() {
        ui.painter().rect_filled(rect, egui::Rounding::ZERO, pal.surface2);
    }
    if !last {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            Stroke::new(1.0_f32, pal.line),
        );
    }
    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 16.0, rect.top() + 12.0),
        egui::pos2(rect.right() - 16.0, rect.bottom() - 12.0),
    );
    // 不能用 scope_builder：它内部会再分配一次空间，把父游标额外推进，
    // 后面的行就被挤出可视区（表现为只剩第一行）。
    // 拆成「先定位、只绘制、不推进游标」三步。
    draw(ui, inner);
    // 游标已经由 allocate_exact_size 推进过，这里不再补推
    let _ = inner;
}

/// .st-btn：32px 高 / 14px 横内边距 / 6px 圆角
pub(super) fn settings_btn(
    ui: &mut egui::Ui,
    pal: &Pal,
    label: &str,
    primary: bool,
) -> bool {
    let w = label.chars().count() as f32 * 13.0 + 28.0;
    let (rect, r) = ui.allocate_exact_size(egui::vec2(w, 32.0), egui::Sense::click());
    let (bg, border) = if primary {
        (if r.hovered() { pal.green_h } else { pal.green }, pal.green)
    } else {
        (
            if r.hovered() { pal.bg_hover } else { pal.surface2 },
            pal.line3,
        )
    };
    theme::round_rect(ui, rect, 6, bg, Some(Stroke::new(1.0_f32, border)));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        theme::font(12.0),
        if primary { Color32::WHITE } else { pal.t1 },
    );
    r.clicked()
}

// ── 各分区内容 ──

impl App {
    // ── 工作区 ──
    fn settings_workspaces(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal();
        settings_h1(ui, &pal, "工作区");
        settings_sub(
            ui,
            &pal,
            "工作区是一个项目目录，每个工作区下可以建多个会话。移除工作区会同时删除它下面的会话与消息。",
        );

        let list = self.workspaces.clone();
        let mut remove: Option<(String, String)> = None;
        let mut rename: Option<(String, String)> = None;
        let mut pin: Option<(String, bool)> = None;

        let n = list.len();
        card_frame(&pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if list.is_empty() {
                let (r, _) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::hover());
                ui.painter().text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "还没有工作区",
                    theme::font(12.0),
                    pal.t4,
                );
            }
            for (i, w) in list.iter().enumerate() {
                let last = i + 1 == n;
                card_row(ui, &pal, last, |ui, rect| {
                    ui.painter().text(
                        egui::pos2(rect.left(), rect.top() + 8.0),
                        egui::Align2::LEFT_CENTER,
                        &w.name,
                        theme::font(13.0),
                        pal.t1,
                    );
                    ui.painter().text(
                        egui::pos2(rect.left(), rect.top() + 26.0),
                        egui::Align2::LEFT_CENTER,
                        &w.path,
                        theme::font(11.5),
                        pal.t4,
                    );
                    let mut x = rect.right();
                    for (label, action) in [
                        ("移除", 2u8),
                        ("重命名", 1u8),
                        (if w.pinned_at.is_some() { "取消置顶" } else { "置顶" }, 0u8),
                    ] {
                        let bw = label.chars().count() as f32 * 13.0 + 24.0;
                        let br = egui::Rect::from_min_size(
                            egui::pos2(x - bw, rect.center().y - 16.0),
                            egui::vec2(bw, 32.0),
                        );
                        let r = ui.interact(
                            br,
                            ui.id().with(("ws-act", w.id.as_str(), action)),
                            egui::Sense::click(),
                        );
                        let bg = if r.hovered() { pal.bg_hover } else { pal.surface2 };
                        theme::round_rect(ui, br, 6, bg, Some(Stroke::new(1.0_f32, pal.line3)));
                        ui.painter().text(
                            br.center(),
                            egui::Align2::CENTER_CENTER,
                            label,
                            theme::font(12.0),
                            if action == 2 { pal.red } else { pal.t1 },
                        );
                        if r.clicked() {
                            match action {
                                0 => pin = Some((w.id.clone(), w.pinned_at.is_none())),
                                1 => rename = Some((w.id.clone(), w.name.clone())),
                                _ => remove = Some((w.id.clone(), w.name.clone())),
                            }
                        }
                        x -= bw + 8.0;
                    }
                });
            }
        });

        ui.add_space(8.0);
        self.add_row_button(ui, &pal, "＋ 添加工作区");

        if let Some((id, name)) = remove {
            self.pending_confirm = Some(crate::Confirm {
                title: format!("删除工作区「{name}」？"),
                message: "该工作区下的所有会话与消息都会被删除。".into(),
                warning: Some("磁盘上的项目目录不会被删除。".into()),
                confirm_text: "删除工作区".into(),
                tone: crate::Tone::Danger,
                kind: crate::ConfirmKind::RemoveWorkspace(id),
            });
        }
        if let Some((id, name)) = rename {
            self.renaming = Some((id, name, true));
        }
        if let Some((id, pinned)) = pin {
            if let Some(c) = &self.conn {
                let _ = service::pin_workspace(c, &id, pinned);
                self.reload();
            }
        }
    }

    // ── 环境变量 ──
    fn settings_env(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal();
        settings_h1(ui, &pal, "环境变量");
        settings_sub(
            ui,
            &pal,
            "写入 ~/.claude/settings.json 的 env 字段，保存前自动备份到 ~/.claude/backups/。",
        );

        let mut remove_at: Option<usize> = None;
        let rows = self.env_rows.len();
        let key_w = 300.0_f32.min((ui.available_width() - 24.0) * 0.42);

        card_frame(&pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            // 表头
            let (hr, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
            ui.painter().rect_filled(hr, egui::Rounding::ZERO, pal.bg_bar);
            ui.painter()
                .hline(hr.x_range(), hr.bottom(), Stroke::new(1.0_f32, pal.line2));
            ui.painter().text(
                egui::pos2(hr.left() + 12.0, hr.center().y),
                egui::Align2::LEFT_CENTER,
                "键",
                theme::font(11.5),
                pal.t3,
            );
            ui.painter().text(
                egui::pos2(hr.left() + key_w + 12.0, hr.center().y),
                egui::Align2::LEFT_CENTER,
                "值",
                theme::font(11.5),
                pal.t3,
            );

            if rows == 0 {
                let (er, _) =
                    ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::hover());
                ui.painter().text(
                    er.center(),
                    egui::Align2::CENTER_CENTER,
                    "还没有环境变量",
                    theme::font(12.0),
                    pal.t4,
                );
            }
            for (i, (k, v)) in self.env_rows.iter_mut().enumerate() {
                let (rr, resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 44.0),
                    egui::Sense::hover(),
                );
                if resp.hovered() {
                    ui.painter().rect_filled(rr, egui::Rounding::ZERO, pal.surface2);
                }
                if i + 1 < rows {
                    ui.painter()
                        .hline(rr.x_range(), rr.bottom(), Stroke::new(1.0_f32, pal.line));
                }
                let key_rect = egui::Rect::from_min_size(
                    egui::pos2(rr.left() + 4.0, rr.top() + 6.0),
                    egui::vec2(key_w - 8.0, 32.0),
                );
                let val_rect = egui::Rect::from_min_size(
                    egui::pos2(rr.left() + key_w + 4.0, rr.top() + 6.0),
                    egui::vec2(rr.width() - key_w - 44.0, 32.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(key_rect), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(k)
                            .frame(false)
                            .font(theme::mono(12.0))
                            .text_color(pal.blue)
                            .hint_text("KEY_NAME")
                            .desired_width(key_rect.width()),
                    );
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(val_rect), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(v)
                            .frame(false)
                            .font(theme::mono(12.0))
                            .hint_text("value")
                            .desired_width(val_rect.width()),
                    );
                });
                let xr = egui::Rect::from_center_size(
                    egui::pos2(rr.right() - 22.0, rr.center().y),
                    egui::vec2(21.0, 21.0),
                );
                let xresp = ui.interact(xr, ui.id().with(("env-del", i)), egui::Sense::click());
                if xresp.hovered() {
                    theme::round_rect(ui, xr, 4, pal.bg_hover, None);
                }
                icons::close(
                    ui.painter(),
                    egui::Rect::from_center_size(xr.center(), egui::vec2(9.0, 9.0)),
                    if xresp.hovered() { pal.red } else { pal.t4 },
                );
                if xresp.clicked() {
                    remove_at = Some(i);
                }
            }
        });

        if let Some(i) = remove_at {
            self.env_rows.remove(i);
        }

        ui.add_space(8.0);
        if self.add_row_button(ui, &pal, "＋ 添加变量") {
            self.env_rows.push((String::new(), String::new()));
        }

        self.notice(ui, "已在运行的会话不会读取新变量，请关闭后重新打开。", false);

        ui.add_space(4.0);
        let (lr, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter()
            .hline(lr.x_range(), lr.center().y, Stroke::new(1.0_f32, pal.line));
        ui.add_space(14.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("保存前备份到 ~/.claude/backups/")
                    .font(theme::font(11.5))
                    .color(pal.t4),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if settings_btn(ui, &pal, "保存", true) {
                    let list: Vec<crate::model::EnvEntry> = self
                        .env_rows
                        .iter()
                        .filter(|(k, _)| !k.trim().is_empty())
                        .map(|(k, v)| crate::model::EnvEntry {
                            key: k.trim().to_string(),
                            value: v.clone(),
                        })
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
                ui.add_space(8.0);
                if settings_btn(ui, &pal, "连接体检", false) {
                    self.status = "连接体检需在会话内验证".into();
                }
            });
        });
    }

    // ── 权限 ──
    fn settings_permissions(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal();
        settings_h1(ui, &pal, "权限");
        settings_sub(
            ui,
            &pal,
            "控制 Claude 执行工具时的确认行为。默认模式决定新会话的起点。",
        );

        card_frame(&pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            card_row(ui, &pal, true, |ui, rect| {
                ui.painter().text(
                    egui::pos2(rect.left(), rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "默认模式",
                    theme::font(13.0),
                    pal.t1,
                );
                let bw = 200.0;
                let br = egui::Rect::from_min_size(
                    egui::pos2(rect.right() - bw, rect.center().y - 15.0),
                    egui::vec2(bw, 30.0),
                );
                ui.scope_builder(egui::UiBuilder::new().max_rect(br), |ui| {
                    egui::ComboBox::from_id_salt("perm-mode")
                        .selected_text(self.perm_mode.clone())
                        .width(bw)
                        .show_ui(ui, |ui| {
                            for m in
                                ["default", "acceptEdits", "plan", "dontAsk", "bypassPermissions"]
                            {
                                ui.selectable_value(&mut self.perm_mode, m.to_string(), m);
                            }
                        });
                });
            });
        });

        ui.add_space(18.0);
        for is_allow in [true, false] {
            let title = if is_allow {
                "允许规则（每行一条）"
            } else {
                "拒绝规则（每行一条）"
            };
            ui.label(
                egui::RichText::new(title)
                    .font(theme::font_bold(12.5))
                    .color(pal.t2),
            );
            ui.add_space(6.0);
            let text = if is_allow {
                &mut self.perm_allow
            } else {
                &mut self.perm_deny
            };
            // 不用 add/add_sized：在自定义 max_rect + ScrollArea 下，
            // TextEdit 的可用高度会被算成 0，表现为只有一行。
            // 手工分配矩形再放进绝对定位的 ui。
            let w = ui.available_width();
            // 高度随内容行数增长（每行 18px），最少 4 行
            let lines = text.lines().count().max(4) as f32;
            let box_h = lines * 18.0 + 16.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(w, box_h), egui::Sense::hover());
            theme::round_rect(
                ui,
                rect,
                6,
                pal.surface,
                Some(Stroke::new(1.0_f32, pal.line2)),
            );
            let inner = egui::Rect::from_min_max(
                egui::pos2(rect.left() + 8.0, rect.top() + 6.0),
                egui::pos2(rect.right() - 8.0, rect.bottom() - 6.0),
            );
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(inner)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            child.add(
                egui::TextEdit::multiline(text)
                    .frame(false)
                    .desired_width(inner.width())
                    .desired_rows(lines as usize)
                    .font(theme::mono(12.0)),
            );
            ui.add_space(16.0);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if settings_btn(ui, &pal, "保存", true) {
                let p = service::Permissions {
                    default_mode: self.perm_mode.clone(),
                    allow: crate::split_lines(&self.perm_allow),
                    deny: crate::split_lines(&self.perm_deny),
                };
                match service::save_permissions(&p) {
                    Ok(()) => self.status = "已保存".into(),
                    Err(e) => self.status = e,
                }
            }
        });
    }

    // ── 模型 ──
    fn settings_models(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal();
        settings_h1(ui, &pal, "模型");
        settings_sub(
            ui,
            &pal,
            "模型只能在这里修改，会话内不提供切换。这些值存放在 settings.json 的 env 中。",
        );

        let n = self.model_fields.len();
        card_frame(&pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for i in 0..n {
                let last = i + 1 == n;
                let (key, label, _) = self.model_fields[i].clone();
                card_row_h(ui, &pal, last, 76.0, |ui, rect| {
                    ui.painter().text(
                        egui::pos2(rect.left(), rect.top() + 8.0),
                        egui::Align2::LEFT_CENTER,
                        &label,
                        theme::font(13.0),
                        pal.t1,
                    );
                    ui.painter().text(
                        egui::pos2(rect.left(), rect.top() + 26.0),
                        egui::Align2::LEFT_CENTER,
                        &key,
                        theme::mono(11.0),
                        pal.t4,
                    );
                    let bw = 260.0_f32.min(rect.width() * 0.5);
                    let br = egui::Rect::from_min_size(
                        egui::pos2(rect.right() - bw, rect.center().y - 15.0),
                        egui::vec2(bw, 30.0),
                    );
                    let mut buf = self.model_fields[i].2.clone();
                    theme::round_rect(
                        ui,
                        br,
                        6,
                        pal.surface,
                        Some(Stroke::new(1.0_f32, pal.line2)),
                    );
                    let mut child = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(egui::Rect::from_min_max(
                                egui::pos2(br.left() + 8.0, br.top() + 5.0),
                                egui::pos2(br.right() - 8.0, br.bottom() - 5.0),
                            ))
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    if child
                        .add(
                            egui::TextEdit::singleline(&mut buf)
                                .frame(false)
                                .font(theme::mono(12.0))
                                .desired_width(bw - 16.0),
                        )
                        .changed()
                    {
                        self.model_fields[i].2 = buf;
                    }
                });
            }
        });

        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if settings_btn(ui, &pal, "保存", true) {
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
        });
    }

    // ── Skill ──
    fn settings_skills(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal();
        settings_h1(ui, &pal, "Skill");
        settings_sub(ui, &pal, "扫描 ~/.claude/skills。开关只影响新会话。");

        let skills = match &self.conn {
            Some(c) => service::list_skills(c),
            None => vec![],
        };
        if skills.is_empty() {
            ui.label(
                egui::RichText::new("未找到任何 Skill")
                    .font(theme::font(12.0))
                    .color(pal.t4),
            );
            return;
        }

        let mut toggle: Option<(String, bool)> = None;
        let avail = ui.available_width();
        let cols = (((avail + 12.0) / 272.0).floor() as usize).max(1);
        let card_w = (avail - 12.0 * (cols as f32 - 1.0)) / cols as f32;

        for chunk in skills.chunks(cols) {
            ui.horizontal(|ui| {
                for s in chunk {
                    let (rect, r) =
                        ui.allocate_exact_size(egui::vec2(card_w, 78.0), egui::Sense::click());
                    theme::round_rect(
                        ui,
                        rect,
                        10,
                        pal.surface,
                        Some(Stroke::new(
                            1.0_f32,
                            if r.hovered() { pal.line3 } else { pal.line2 },
                        )),
                    );
                    ui.painter().text(
                        egui::pos2(rect.left() + 12.0, rect.top() + 18.0),
                        egui::Align2::LEFT_CENTER,
                        &s.name,
                        theme::font_bold(12.5),
                        pal.t1,
                    );
                    let desc = theme::ellipsis(
                        ui,
                        &s.description,
                        theme::font(11.5),
                        pal.t4,
                        rect.width() - 24.0,
                    );
                    ui.painter()
                        .galley(egui::pos2(rect.left() + 12.0, rect.top() + 32.0), desc, pal.t4);
                    // 开关
                    let sw = egui::Rect::from_center_size(
                        egui::pos2(rect.right() - 26.0, rect.bottom() - 18.0),
                        egui::vec2(30.0, 16.0),
                    );
                    theme::round_rect(
                        ui,
                        sw,
                        8,
                        if s.enabled { pal.green } else { pal.line3 },
                        None,
                    );
                    let kx = if s.enabled { sw.right() - 8.0 } else { sw.left() + 8.0 };
                    ui.painter()
                        .circle_filled(egui::pos2(kx, sw.center().y), 6.0, Color32::WHITE);
                    if r.clicked() {
                        toggle = Some((s.name.clone(), !s.enabled));
                    }
                    ui.add_space(12.0);
                }
            });
            ui.add_space(12.0);
        }

        if let Some((name, on)) = toggle {
            let r = match &self.conn {
                Some(c) => service::toggle_skill(c, &name, on),
                None => Err("数据库未打开".into()),
            };
            match r {
                Ok(()) => self.status = "已更新".into(),
                Err(e) => self.status = e,
            }
        }
    }

    // ── 插件 ──
    fn settings_plugins(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal();
        settings_h1(ui, &pal, "插件");
        settings_sub(
            ui,
            &pal,
            "开关写入 settings.json 的 enabledPlugins。安装与卸载通过 claude plugin 执行。",
        );

        // 安装行：输入 + 按钮
        let mut do_install = false;
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 90.0).max(180.0);
            ui.add_sized(
                [w, 30.0],
                egui::TextEdit::singleline(&mut self.plugin_input)
                    .font(theme::mono(11.5))
                    .hint_text("插件 ID，如 xxx@marketplace"),
            );
            ui.add_space(8.0);
            // 只接受 name@marketplace 形式，避免无谓的 CLI 调用
            let can = {
                let v = self.plugin_input.trim();
                v.contains('@')
                    && v.split('@').count() == 2
                    && v.chars().all(|c| c.is_alphanumeric() || matches!(c, '@' | '.' | '_' | '-'))
            };
            let bw = 72.0;
            let (r, resp) = ui.allocate_exact_size(egui::vec2(bw, 30.0), egui::Sense::click());
            let c = if !can {
                pal.green.gamma_multiply(0.4)
            } else if resp.hovered() {
                pal.green_h
            } else {
                pal.green
            };
            theme::round_rect(ui, r, 6, c, None);
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                "安装",
                theme::font_bold(12.0),
                Color32::WHITE,
            );
            if resp.clicked() && can {
                do_install = true;
            }
        });
        ui.add_space(14.0);

        let plugins = service::list_plugins();
        if plugins.is_empty() {
            // .empty-state padding 56px 20px
            let (r, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 120.0), egui::Sense::hover());
            ui.painter().text(
                egui::pos2(r.center().x, r.top() + 48.0),
                egui::Align2::CENTER_CENTER,
                "还没有安装插件",
                theme::font(13.0),
                pal.t2,
            );
            ui.painter().text(
                egui::pos2(r.center().x, r.top() + 72.0),
                egui::Align2::CENTER_CENTER,
                "在「插件仓库」中添加仓库后即可安装",
                theme::font(12.0),
                pal.t4,
            );
        } else {
            // 自适应列数：minmax(clamp(220px, 24vw, 300px), 1fr)
            let avail = ui.available_width();
            let min_w = (avail * 0.24).clamp(220.0, 300.0);
            let cols = ((avail + 10.0) / (min_w + 10.0)).floor().max(1.0) as usize;
            let card_w = (avail - 10.0 * (cols as f32 - 1.0)) / cols as f32;

            let mut toggle: Option<(String, bool)> = None;
            let mut uninstall: Option<String> = None;

            for chunk in plugins.chunks(cols) {
                ui.horizontal(|ui| {
                    for pl in chunk {
                        let card_h = 96.0;
                        let (rect, r) =
                            ui.allocate_exact_size(egui::vec2(card_w, card_h), egui::Sense::hover());
                        theme::round_rect(
                            ui,
                            rect,
                            8,
                            pal.surface,
                            Some(Stroke::new(
                                1.0_f32,
                                if r.hovered() { pal.line3 } else { pal.line2 },
                            )),
                        );
                        // 图标 30x30
                        let ico = egui::Rect::from_min_size(
                            egui::pos2(rect.left() + 13.0, rect.top() + 13.0),
                            egui::vec2(30.0, 30.0),
                        );
                        theme::round_rect(
                            ui,
                            ico,
                            6,
                            pal.bg_soft,
                            Some(Stroke::new(1.0_f32, pal.line)),
                        );
                        let initials: String = pl.name.chars().take(2).collect::<String>().to_uppercase();
                        ui.painter().text(
                            ico.center(),
                            egui::Align2::CENTER_CENTER,
                            initials,
                            theme::mono(9.5),
                            pal.t2,
                        );
                        // 名称 / 版本 · 仓库
                        let bx = ico.right() + 11.0;
                        let bg = crate::theme::ellipsis_ctx(
                            ui.ctx(),
                            &pl.name,
                            theme::font_bold(12.5),
                            pal.t1,
                            rect.right() - bx - 13.0,
                        );
                        ui.painter().galley(egui::pos2(bx, rect.top() + 14.0), bg, pal.t1);
                        let sub = if pl.version.is_empty() {
                            pl.marketplace.clone()
                        } else {
                            format!("{} · {}", pl.version, pl.marketplace)
                        };
                        let sg = crate::theme::ellipsis_ctx(
                            ui.ctx(),
                            &sub,
                            theme::font(11.0),
                            pal.t4,
                            rect.right() - bx - 13.0,
                        );
                        ui.painter().galley(egui::pos2(bx, rect.top() + 32.0), sg, pal.t4);

                        // 分隔线 + 状态行
                        let fy = rect.bottom() - 34.0;
                        ui.painter().hline(
                            (rect.left() + 13.0)..=(rect.right() - 13.0),
                            fy,
                            Stroke::new(1.0_f32, pal.line),
                        );
                        let dot_c = if pl.enabled { pal.green } else { Color32::from_rgb(0xc6, 0xc6, 0xcb) };
                        ui.painter()
                            .circle_filled(egui::pos2(rect.left() + 15.5, fy + 14.0), 2.5, dot_c);
                        ui.painter().text(
                            egui::pos2(rect.left() + 24.0, fy + 14.0),
                            egui::Align2::LEFT_CENTER,
                            if pl.enabled { "已启用" } else { "已禁用" },
                            theme::font(11.0),
                            if pl.enabled { pal.green_t } else { pal.t4 },
                        );

                        // 右侧两个小按钮：开关 + 卸载
                        let del_rect = egui::Rect::from_center_size(
                            egui::pos2(rect.right() - 21.0, fy + 14.0),
                            egui::vec2(24.0, 20.0),
                        );
                        let tog_rect = egui::Rect::from_center_size(
                            egui::pos2(del_rect.left() - 6.0 - 12.0, fy + 14.0),
                            egui::vec2(24.0, 20.0),
                        );
                        let del = ui.interact(
                            del_rect,
                            ui.id().with(("pl-del", pl.id.as_str())),
                            egui::Sense::click(),
                        );
                        let tog = ui.interact(
                            tog_rect,
                            ui.id().with(("pl-tog", pl.id.as_str())),
                            egui::Sense::click(),
                        );
                        if del.hovered() {
                            theme::round_rect(ui, del_rect, 4, pal.danger_bg, None);
                        }
                        let confirming = self.plugin_confirm.as_deref() == Some(pl.id.as_str());
                        ui.painter().text(
                            del_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            if confirming { "确认" } else { "✕" },
                            theme::font(if confirming { 11.0 } else { 10.0 }),
                            if confirming || del.hovered() { pal.red } else { pal.t4 },
                        );
                        // 开关小图标
                        let sw = egui::Rect::from_center_size(tog_rect.center(), egui::vec2(20.0, 12.0));
                        theme::round_rect(
                            ui,
                            sw,
                            6,
                            if pl.enabled { pal.green } else { pal.line3 },
                            None,
                        );
                        let kx = if pl.enabled { sw.right() - 5.0 } else { sw.left() + 5.0 };
                        ui.painter()
                            .circle_filled(egui::pos2(kx, sw.center().y), 4.0, Color32::WHITE);

                        if tog.clicked() {
                            toggle = Some((pl.id.clone(), !pl.enabled));
                        }
                        if del.clicked() {
                            uninstall = Some(pl.id.clone());
                        }
                        ui.add_space(10.0);
                    }
                });
                ui.add_space(10.0);
            }

            if let Some((id, on)) = toggle {
                match service::toggle_plugin(&id, on) {
                    Ok(()) => self.status = "已更新".into(),
                    Err(e) => self.status = e,
                }
            }
            if let Some(id) = uninstall {
                // 第一次点击进入确认态，4 秒后自动复位
                if self.plugin_confirm.as_deref() == Some(id.as_str()) {
                    self.plugin_confirm = None;
                    match service::uninstall_plugin(&id) {
                        Ok(()) => self.status = "已卸载".into(),
                        Err(e) => self.status = e,
                    }
                } else {
                    self.plugin_confirm = Some(id);
                    self.plugin_confirm_at = ui.input(|i| i.time);
                }
            }
        }

        if do_install {
            let id = self.plugin_input.trim().to_string();
            match service::install_plugin(&id) {
                Ok(()) => {
                    self.plugin_input.clear();
                    self.status = "已安装".into();
                }
                Err(e) => self.status = e,
            }
        }

        // 确认态 4 秒超时复位
        if let (Some(_), at) = (&self.plugin_confirm, self.plugin_confirm_at) {
            if ui.input(|i| i.time) - at > 4.0 {
                self.plugin_confirm = None;
            }
        }
    }

    // ── 插件仓库 ──
    fn settings_marketplaces(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal();
        settings_h1(ui, &pal, "插件仓库");
        settings_sub(ui, &pal, "添加第三方插件来源。移除后已安装的插件不会被卸载。");

        let list = service::list_marketplaces();
        let mut remove: Option<String> = None;
        let n = list.len();
        card_frame(&pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (i, m) in list.iter().enumerate() {
                card_row(ui, &pal, i + 1 == n, |ui, rect| {
                    ui.painter().text(
                        egui::pos2(rect.left(), rect.top() + 8.0),
                        egui::Align2::LEFT_CENTER,
                        &m.name,
                        theme::font(13.0),
                        pal.t1,
                    );
                    ui.painter().text(
                        egui::pos2(rect.left(), rect.top() + 26.0),
                        egui::Align2::LEFT_CENTER,
                        &m.source,
                        theme::font(11.5),
                        pal.t4,
                    );
                    if m.builtin {
                        ui.painter().text(
                            egui::pos2(rect.right(), rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            "内置",
                            theme::font(11.5),
                            pal.t4,
                        );
                    } else {
                        let br = egui::Rect::from_min_size(
                            egui::pos2(rect.right() - 56.0, rect.center().y - 16.0),
                            egui::vec2(56.0, 32.0),
                        );
                        let r = ui.interact(
                            br,
                            ui.id().with(("mkt-del", m.name.as_str())),
                            egui::Sense::click(),
                        );
                        let bg = if r.hovered() { pal.bg_hover } else { pal.surface2 };
                        theme::round_rect(ui, br, 6, bg, Some(Stroke::new(1.0_f32, pal.line3)));
                        ui.painter().text(
                            br.center(),
                            egui::Align2::CENTER_CENTER,
                            "移除",
                            theme::font(12.0),
                            pal.red,
                        );
                        if r.clicked() {
                            remove = Some(m.name.clone());
                        }
                    }
                });
            }
        });

        if let Some(name) = remove {
            match service::remove_marketplace(&name) {
                Ok(()) => self.status = "已移除仓库".into(),
                Err(e) => self.status = e,
            }
        }

        ui.add_space(18.0);
        ui.label(
            egui::RichText::new("添加仓库")
                .font(theme::font_bold(12.5))
                .color(pal.t2),
        );
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.mkt_name)
                    .hint_text("仓库名称")
                    .desired_width(180.0),
            );
            ui.add_space(8.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.mkt_repo)
                    .hint_text("github.com/owner/repo")
                    .desired_width(320.0),
            );
            ui.add_space(8.0);
            if settings_btn(ui, &pal, "添加", true) {
                match service::add_marketplace(&self.mkt_name, &self.mkt_repo) {
                    Ok(()) => {
                        self.mkt_name.clear();
                        self.mkt_repo.clear();
                        self.status = "已添加仓库".into();
                    }
                    Err(e) => self.status = e,
                }
            }
        });
    }

    // ── 更新 ──
    fn settings_update(&mut self, ui: &mut egui::Ui) {
        let pal = self.pal();
        settings_h1(ui, &pal, "更新");
        settings_sub(
            ui,
            &pal,
            "Rust 版是单文件程序，更新方式是下载新版本替换。三个实现各有独立产物。",
        );

        card_frame(&pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            card_row(ui, &pal, false, |ui, rect| {
                ui.painter().text(
                    egui::pos2(rect.left(), rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "当前版本",
                    theme::font(13.0),
                    pal.t1,
                );
                ui.painter().text(
                    egui::pos2(rect.right(), rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    format!("v{}", crate::VERSION),
                    theme::mono(12.0),
                    pal.t2,
                );
            });
            card_row(ui, &pal, true, |ui, rect| {
                ui.painter().text(
                    egui::pos2(rect.left(), rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "发布地址",
                    theme::font(13.0),
                    pal.t1,
                );
                ui.painter().text(
                    egui::pos2(rect.right(), rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    "github.com/1696016910/CCWorkspace/releases",
                    theme::mono(11.5),
                    pal.blue,
                );
            });
        });

        ui.add_space(14.0);
        self.notice(
            ui,
            "三个实现的产物：ClaudeWorkbench-Node / Go / Rust-<版本>-x64.exe",
            false,
        );
    }

    /// .st-add-row：虚线边框的整行按钮
    fn add_row_button(&mut self, ui: &mut egui::Ui, pal: &Pal, label: &str) -> bool {
        let (ar, r) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::click());
        theme::round_rect(
            ui,
            ar,
            8,
            if r.hovered() { pal.bg_soft } else { pal.surface2 },
            Some(Stroke::new(1.0_f32, pal.line3)),
        );
        ui.painter().text(
            ar.center(),
            egui::Align2::CENTER_CENTER,
            label,
            theme::font(12.0),
            if r.hovered() { pal.t2 } else { pal.t3 },
        );
        if label.starts_with("＋ 添加工作区") && r.clicked() {
            self.show_new_ws = true;
        }
        r.clicked()
    }

    /// .st-notice：浅底 + 1px 边框 + 8px 圆角
    fn notice(&self, ui: &mut egui::Ui, text: &str, warn: bool) {
        let pal = self.pal();
        let w = ui.available_width();
        let galley = ui.fonts(|f| f.layout(text.to_owned(), theme::font(12.0), pal.t3, w - 24.0));
        let h = galley.size().y + 20.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        let (bg, bd, fg) = if warn {
            (pal.danger_bg, pal.danger_line, pal.danger_t)
        } else {
            (pal.surface2, pal.line2, pal.t3)
        };
        theme::round_rect(ui, rect, 8, bg, Some(Stroke::new(1.0_f32, bd)));
        ui.painter()
            .galley(egui::pos2(rect.left() + 12.0, rect.top() + 10.0), galley, fg);
        ui.add_space(14.0);
    }
}
