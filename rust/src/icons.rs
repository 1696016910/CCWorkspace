//! 线性图标，全部用 egui 的 painter 手绘。
//!
//! egui 不自带图标字体，塞字体文件会让二进制大好几 MB，
//! 而设计稿里的图标都是简单线条，直接画更可控。
//! 所有函数在 `r` 这个 15x15 的方框内绘制，与原型 svg 的 viewBox 比例一致。

use egui::{Color32, Pos2, Rect, Rounding, Shape, Stroke, Vec2};

/// 描边宽度与设计稿一致：1.6 / 1.7 / 2 三档。
const W: f32 = 1.6;

fn stroke(color: Color32, w: f32) -> Stroke {
    Stroke::new(w, color)
}

/// 折线。
fn poly(p: &egui::Painter, pts: &[Pos2], st: Stroke) {
    p.add(Shape::line(pts.to_vec(), st));
}

/// 圆弧（角度用弧度，0 指向右侧，顺时针为正）。
fn arc(p: &egui::Painter, c: Pos2, r: f32, from: f32, to: f32, st: Stroke) {
    const STEPS: usize = 24;
    let mut pts = Vec::with_capacity(STEPS + 1);
    for i in 0..=STEPS {
        let t = from + (to - from) * (i as f32 / STEPS as f32);
        pts.push(Pos2::new(c.x + r * t.cos(), c.y + r * t.sin()));
    }
    poly(p, &pts, st);
}

/// 右上角带折角的文件夹。
pub fn folder(p: &egui::Painter, r: Rect, color: Color32) {
    let st = stroke(color, W);
    let body = Rect::from_min_max(
        Pos2::new(r.left() + 1.0, r.top() + 4.5),
        Pos2::new(r.right() - 1.0, r.bottom() - 1.5),
    );
    p.rect_stroke(body, Rounding::same(2.0), st);
    // 顶部标签
    poly(
        p,
        &[
            Pos2::new(r.left() + 1.0, r.top() + 4.5),
            Pos2::new(r.left() + 1.0, r.top() + 2.0),
            Pos2::new(r.left() + 6.0, r.top() + 2.0),
            Pos2::new(r.left() + 7.5, r.top() + 4.5),
        ],
        st,
    );
}

/// 向下的尖角（展开状态）。
pub fn chevron_down(p: &egui::Painter, r: Rect, color: Color32) {
    let c = r.center();
    poly(
        p,
        &[
            Pos2::new(c.x - 3.2, c.y - 1.6),
            Pos2::new(c.x, c.y + 1.6),
            Pos2::new(c.x + 3.2, c.y - 1.6),
        ],
        stroke(color, 2.0),
    );
}

/// 向右的尖角（收起状态）。
pub fn chevron_right(p: &egui::Painter, r: Rect, color: Color32) {
    let c = r.center();
    poly(
        p,
        &[
            Pos2::new(c.x - 1.6, c.y - 3.2),
            Pos2::new(c.x + 1.6, c.y),
            Pos2::new(c.x - 1.6, c.y + 3.2),
        ],
        stroke(color, 2.0),
    );
}

/// 三个横点（更多）。
pub fn more(p: &egui::Painter, r: Rect, color: Color32) {
    let cy = r.center().y;
    for dx in [-4.5, 0.0, 4.5] {
        p.circle_filled(Pos2::new(r.center().x + dx, cy), 1.35, color);
    }
}

/// ＋
pub fn plus(p: &egui::Painter, r: Rect, color: Color32) {
    let c = r.center();
    let st = stroke(color, 2.0);
    p.line_segment([Pos2::new(c.x - 4.0, c.y), Pos2::new(c.x + 4.0, c.y)], st);
    p.line_segment([Pos2::new(c.x, c.y - 4.0), Pos2::new(c.x, c.y + 4.0)], st);
}

/// 图钉。
pub fn pin(p: &egui::Painter, r: Rect, color: Color32) {
    let st = stroke(color, W);
    let c = r.center();
    p.circle_stroke(Pos2::new(c.x, c.y - 2.5), 2.8, st);
    p.line_segment([Pos2::new(c.x, c.y + 0.3), Pos2::new(c.x, c.y + 5.0)], st);
}

/// 垃圾桶。
pub fn trash(p: &egui::Painter, r: Rect, color: Color32) {
    let st = stroke(color, W);
    let c = r.center();
    p.line_segment(
        [Pos2::new(c.x - 5.0, c.y - 3.5), Pos2::new(c.x + 5.0, c.y - 3.5)],
        st,
    );
    // 桶盖把手
    p.line_segment(
        [Pos2::new(c.x - 1.8, c.y - 3.5), Pos2::new(c.x - 1.8, c.y - 5.0)],
        st,
    );
    p.line_segment(
        [Pos2::new(c.x + 1.8, c.y - 3.5), Pos2::new(c.x + 1.8, c.y - 5.0)],
        st,
    );
    p.line_segment(
        [Pos2::new(c.x - 1.8, c.y - 5.0), Pos2::new(c.x + 1.8, c.y - 5.0)],
        st,
    );
    // 桶身
    poly(
        p,
        &[
            Pos2::new(c.x - 3.6, c.y - 3.5),
            Pos2::new(c.x - 3.0, c.y + 5.5),
            Pos2::new(c.x + 3.0, c.y + 5.5),
            Pos2::new(c.x + 3.6, c.y - 3.5),
        ],
        st,
    );
}

/// 齿轮。
pub fn gear(p: &egui::Painter, r: Rect, color: Color32) {
    let st = stroke(color, W);
    let c = r.center();
    p.circle_stroke(c, 2.6, st);
    p.circle_stroke(c, 5.2, st);
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::TAU / 8.0;
        let (s, co) = a.sin_cos();
        p.line_segment(
            [
                Pos2::new(c.x + co * 5.2, c.y + s * 5.2),
                Pos2::new(c.x + co * 6.6, c.y + s * 6.6),
            ],
            st,
        );
    }
}

/// 月亮（暗色模式）。
pub fn moon(p: &egui::Painter, r: Rect, color: Color32) {
    let c = r.center();
    // 用两段圆弧拼出月牙
    arc(p, Pos2::new(c.x - 1.2, c.y), 4.6, 0.7, std::f32::consts::TAU - 0.7, stroke(color, W));
}

/// 回收站（侧栏底部）。
pub fn recycle(p: &egui::Painter, r: Rect, color: Color32) {
    let st = stroke(color, W);
    let c = r.center();
    p.circle_stroke(c, 4.6, st);
    // 内部箭头示意
    poly(
        p,
        &[
            Pos2::new(c.x - 1.4, c.y - 1.6),
            Pos2::new(c.x + 1.2, c.y - 1.6),
            Pos2::new(c.x + 1.2, c.y + 1.2),
        ],
        st,
    );
}

/// 向上箭头（发送）。
pub fn arrow_up(p: &egui::Painter, r: Rect, color: Color32) {
    let c = r.center();
    let st = stroke(color, 2.0);
    p.line_segment([Pos2::new(c.x, c.y + 4.5), Pos2::new(c.x, c.y - 4.5)], st);
    poly(
        p,
        &[
            Pos2::new(c.x - 3.4, c.y - 1.2),
            Pos2::new(c.x, c.y - 4.6),
            Pos2::new(c.x + 3.4, c.y - 1.2),
        ],
        st,
    );
}

/// 方形停止符。
pub fn stop(p: &egui::Painter, r: Rect, color: Color32) {
    let c = r.center();
    let sq = Rect::from_center_size(c, Vec2::splat(7.0));
    p.rect_filled(sq, Rounding::same(1.5), color);
}

/// 复制（顶部历史/副本图标）。
pub fn copy(p: &egui::Painter, r: Rect, color: Color32) {
    let st = stroke(color, W);
    let c = r.center();
    let back = Rect::from_min_max(
        Pos2::new(c.x - 4.5, c.y - 5.0),
        Pos2::new(c.x + 2.0, c.y + 1.5),
    );
    let front = Rect::from_min_max(
        Pos2::new(c.x - 1.5, c.y - 1.0),
        Pos2::new(c.x + 5.0, c.y + 5.5),
    );
    // 后层只画左侧和上方两条边，避免和前层重叠处出现双线
    p.rect_stroke(back, Rounding::same(2.0), st);
    p.rect_filled(front, Rounding::same(2.0), Color32::TRANSPARENT);
    p.rect_stroke(front, Rounding::same(2.0), st);
}
