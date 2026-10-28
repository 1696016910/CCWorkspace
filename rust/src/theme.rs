//! 设计令牌与通用绘制助手。
//!
//! 数值全部来自 prototype/20261027-界面原型.html 的 CSS 变量，
//! 改动前先对照那份稿子，别凭感觉调颜色和间距。

use egui::{Color32, FontId, Rounding, Stroke};

/// 与原型 `:root` 一一对应的调色板。
#[derive(Clone, Copy)]
pub struct Pal {
    pub bg: Color32,
    pub bg_bar: Color32,
    pub bg_hover: Color32,
    pub bg_soft: Color32,
    pub bg_select: Color32,
    pub surface: Color32,
    pub line: Color32,
    pub line2: Color32,
    pub line3: Color32,
    pub t1: Color32,
    pub t2: Color32,
    pub t3: Color32,
    pub t4: Color32,
    pub green: Color32,
    pub green_h: Color32,
    pub green_soft: Color32,
    pub green_t: Color32,
    pub red: Color32,
    pub danger_bg: Color32,
    pub danger_line: Color32,
    pub danger_t: Color32,
}

const fn c(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub const LIGHT: Pal = Pal {
    bg: c(0xffffff),
    bg_bar: c(0xfafafa),
    bg_hover: c(0xf0f0f1),
    bg_soft: c(0xf5f5f6),
    bg_select: c(0xe7e7e9),
    surface: c(0xffffff),
    line: c(0xe6e6e8),
    line2: c(0xdcdcde),
    line3: c(0xd0d0d4),
    t1: c(0x1f1f23),
    t2: c(0x5f5f66),
    t3: c(0x62626a),
    t4: c(0x6f6f78),
    green: c(0x47815c),
    green_h: c(0x3f7553),
    green_soft: c(0xedf4ef),
    green_t: c(0x2f6b49),
    red: c(0xc0392b),
    danger_bg: c(0xfdf5f5),
    danger_line: c(0xf2dedd),
    danger_t: c(0xa33c34),
};

pub const DARK: Pal = Pal {
    bg: c(0x1a1a1d),
    bg_bar: c(0x141417),
    bg_hover: c(0x26262b),
    bg_soft: c(0x212126),
    bg_select: c(0x2e2e35),
    surface: c(0x212126),
    line: c(0x2a2a30),
    line2: c(0x33333a),
    line3: c(0x3d3d45),
    t1: c(0xe8e8ea),
    t2: c(0xb4b4bb),
    t3: c(0x9c9ca4),
    t4: c(0x7c7c86),
    green: c(0x3f7d5c),
    green_h: c(0x487f61),
    green_soft: c(0x1e2b23),
    green_t: c(0x79bd93),
    red: c(0xe06c66),
    danger_bg: c(0x2a1e1e),
    danger_line: c(0x442e2e),
    danger_t: c(0xe8a9a4),
};

// 字号：原型用 px，egui 的 FontId 等价于 px
pub const F_NEW_WS: f32 = 13.0; // .btn-new-ws
pub const F_WS_NAME: f32 = 13.0; // .ws-name
pub const F_WS_COUNT: f32 = 11.0; // .ws-count
pub const F_CHAT: f32 = 12.5; // .chat-title
pub const F_CHAT_TIME: f32 = 10.5; // .chat-time
pub const F_FOOT: f32 = 12.5; // .foot-btn
pub const F_MH_TITLE: f32 = 14.0; // .mh-title
pub const F_MH_WS: f32 = 11.5; // .mh-ws
pub const F_BUBBLE: f32 = 13.5; // .msg-user .bubble
pub const F_AI: f32 = 13.5; // .ai-text
pub const F_ROLE: f32 = 11.5; // .ai-role
pub const F_STOPPED: f32 = 12.0; // .stopped-msg
pub const F_HINT: f32 = 11.0; // .cm-hint
pub const F_CODE: f32 = 12.0; // .ai-text code

// 间距
pub const SIDEBAR_W: f32 = 246.0; // grid-template-columns: 246px
pub const HEAD_H: f32 = 48.0; // .main-head
pub const THREAD_MAX_W: f32 = 720.0; // .thread-inner / .composer
pub const COMPOSER_BAR_H: f32 = 42.0; // .composer-bar

pub fn font(size: f32) -> FontId {
    FontId::proportional(size)
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn hairline(pal: &Pal) -> Stroke {
    Stroke::new(1.0_f32, pal.line)
}

/// 把调色板灌进 egui 全局样式，让默认控件也贴合设计稿。
pub fn apply(ctx: &egui::Context, pal: &Pal) {
    use egui::{Style, Visuals};
    let mut style = Style::default();
    let mut v = Visuals::light();
    v.override_text_color = Some(pal.t1);
    v.panel_fill = pal.bg;
    v.window_fill = pal.surface;
    v.extreme_bg_color = pal.bg_soft;
    v.faint_bg_color = pal.bg_soft;
    v.window_stroke = Stroke::new(1.0_f32, pal.line2);
    v.window_rounding = Rounding::same(9.0);
    v.selection.bg_fill = pal.green_soft;
    v.selection.stroke = Stroke::new(1.0_f32, pal.green);
    v.hyperlink_color = pal.green;
    v.widgets.noninteractive.bg_fill = pal.bg;
    v.widgets.noninteractive.weak_bg_fill = pal.bg;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.line);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, pal.t2);
    v.widgets.inactive.weak_bg_fill = pal.bg_soft;
    v.widgets.inactive.bg_fill = pal.bg_soft;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, pal.t2);
    v.widgets.hovered.weak_bg_fill = pal.bg_hover;
    v.widgets.hovered.bg_fill = pal.bg_hover;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, pal.t1);
    v.widgets.active.weak_bg_fill = pal.bg_select;
    v.widgets.active.bg_fill = pal.bg_select;
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, pal.t1);
    v.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 4.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(28),
    };
    v.popup_shadow = v.window_shadow;
    style.visuals = v;
    style.spacing.item_spacing = egui::vec2(0.0, 0.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    ctx.set_style(style);
}

/// 手绘一个填充 + 可选描边的圆角矩形。
pub fn round_rect(
    ui: &egui::Ui,
    rect: egui::Rect,
    radius: u8,
    fill: Color32,
    stroke: Option<Stroke>,
) {
    let p = ui.painter();
    if fill != Color32::TRANSPARENT {
        p.rect_filled(rect, Rounding::same(radius as f32), fill);
    }
    if let Some(s) = stroke {
        p.rect_stroke(rect, Rounding::same(radius as f32), s);
    }
}

/// 单行文本，超出宽度用省略号截断。
pub fn ellipsis(
    ui: &egui::Ui,
    text: &str,
    font: FontId,
    color: Color32,
    max_w: f32,
 ) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat { font_id: font, color, ..Default::default() },
    );
    job.wrap.max_width = max_w;
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    ui.fonts(|f| f.layout_job(job))
}

/// 水平画一条分隔线。
pub fn hline(ui: &mut egui::Ui, pal: &Pal) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, hairline(pal));
}
