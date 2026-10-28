//! Markdown 渲染，样式严格对齐 Node 版 MessageItem.vue 的 `.ai-text` 规范。
//!
//! 支持：标题 h1-h4、段落、加粗、行内代码、围栏代码块、无序/有序列表、
//! 引用块、表格、分割线。
//!
//! 不引第三方解析库：需要精确控制的是字号/行高/背景/边框，自己切行更直接，
//! 也不用担心库的默认样式与设计稿打架。

use crate::theme::{self, Pal};
use egui::text::LayoutJob;
use egui::{Color32, Pos2, Rect, Rounding, Stroke, TextFormat};

/// 行内片段。
#[derive(Clone)]
struct Span {
    text: String,
    bold: bool,
    code: bool,
    link: bool,
}

impl Span {
    fn plain(text: impl Into<String>) -> Self {
        Self { text: text.into(), bold: false, code: false, link: false }
    }
}

/// 表格一行。
struct TableRow {
    cells: Vec<Vec<Span>>,
    header: bool,
}

/// 排版单元。
enum Block {
    Heading(usize, Vec<Span>),
    Para(Vec<Span>),
    Code(String, String),
    Bullet(Vec<Span>),
    Numbered(usize, Vec<Span>),
    Quote(Vec<Span>),
    Rule,
    Table(Vec<TableRow>),
}

// ── 解析 ──

fn parse(src: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut buf: Vec<String> = Vec::new();
    let mut code: Option<String> = None;
    let mut code_lang = String::new();

    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // 围栏代码块
        if let Some(src_buf) = &mut code {
            if line.trim_start().starts_with("```") {
                let body = std::mem::take(src_buf);
                let lang = std::mem::take(&mut code_lang);
                out.push(Block::Code(body, lang));
                code = None;
            } else {
                src_buf.push_str(line);
                src_buf.push('\n');
            }
            i += 1;
            continue;
        }
        if line.trim_start().starts_with("```") {
            flush_para(&mut buf, &mut out);
            code = Some(String::new());
            code_lang = line.trim_start()[3..].trim().to_string();
            i += 1;
            continue;
        }

        let trimmed = line.trim();

        // 表格：带 | 且下一行是 |---|---| 分隔
        if trimmed.contains('|') && i + 1 < lines.len() && is_table_sep(lines[i + 1]) {
            flush_para(&mut buf, &mut out);
            let mut rows = vec![TableRow { cells: split_row(trimmed), header: true }];
            i += 2;
            while i < lines.len() && lines[i].trim().contains('|') && !lines[i].trim().is_empty() {
                rows.push(TableRow { cells: split_row(lines[i].trim()), header: false });
                i += 1;
            }
            out.push(Block::Table(rows));
            continue;
        }

        if trimmed.is_empty() {
            flush_para(&mut buf, &mut out);
        } else if let Some((level, rest)) = heading(trimmed) {
            flush_para(&mut buf, &mut out);
            out.push(Block::Heading(level, inline(rest)));
        } else if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            flush_para(&mut buf, &mut out);
            out.push(Block::Rule);
        } else if let Some(rest) = trimmed.strip_prefix("> ") {
            flush_para(&mut buf, &mut out);
            out.push(Block::Quote(inline(rest)));
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            flush_para(&mut buf, &mut out);
            out.push(Block::Bullet(inline(&trimmed[2..])));
        } else if let Some((n, rest)) = numbered(trimmed) {
            flush_para(&mut buf, &mut out);
            out.push(Block::Numbered(n, inline(rest)));
        } else {
            buf.push(trimmed.to_string());
        }
        i += 1;
    }
    // 未闭合的代码块也要显示，否则内容凭空消失
    if let Some(body) = code {
        out.push(Block::Code(body, code_lang));
    }
    flush_para(&mut buf, &mut out);
    out
}

fn is_table_sep(s: &str) -> bool {
    let t = s.trim();
    if !t.contains('-') || !t.contains('|') {
        return false;
    }
    t.chars().all(|c| matches!(c, '-' | '|' | ':' | ' '))
}

fn split_row(s: &str) -> Vec<Vec<Span>> {
    let t = s.trim().trim_matches('|');
    t.split('|').map(|c| inline(c.trim())).collect()
}

fn heading(s: &str) -> Option<(usize, &str)> {
    let level = s.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = s[level..].trim_start();
    Some((level.min(4), rest))
}

fn flush_para(buf: &mut Vec<String>, out: &mut Vec<Block>) {
    if buf.is_empty() {
        return;
    }
    let joined = buf.join(" ");
    buf.clear();
    out.push(Block::Para(inline(&joined)));
}

fn numbered(s: &str) -> Option<(usize, &str)> {
    let dot = s.find(". ")?;
    let n: usize = s[..dot].parse().ok()?;
    Some((n, &s[dot + 2..]))
}

/// 切出行内代码、加粗、链接。
fn inline(s: &str) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut cur = String::new();

    macro_rules! push_cur {
        () => {
            if !cur.is_empty() {
                out.push(Span::plain(std::mem::take(&mut cur)));
            }
        };
    }

    while i < chars.len() {
        // 行内代码
        if chars[i] == '`' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '`' {
                j += 1;
            }
            if j < chars.len() {
                push_cur!();
                out.push(Span {
                    text: chars[i + 1..j].iter().collect(),
                    bold: false,
                    code: true,
                    link: false,
                });
                i = j + 1;
                continue;
            }
            // 落单反引号当普通字符
            cur.push('`');
            i += 1;
            continue;
        }

        // 加粗 **text**
        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            let mut j = i + 2;
            let mut closed = None;
            while j + 1 < chars.len() {
                if chars[j] == '*' && chars[j + 1] == '*' {
                    closed = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(end) = closed {
                push_cur!();
                out.push(Span {
                    text: chars[i + 2..end].iter().collect(),
                    bold: true,
                    code: false,
                    link: false,
                });
                i = end + 2;
                continue;
            }
        }

        // 链接 [text](url)：只渲染文字
        if chars[i] == '[' {
            if let Some(close) = (i + 1..chars.len()).find(|&j| chars[j] == ']') {
                if close + 1 < chars.len() && chars[close + 1] == '(' {
                    if let Some(paren) = (close + 2..chars.len()).find(|&j| chars[j] == ')') {
                        push_cur!();
                        out.push(Span {
                            text: chars[i + 1..close].iter().collect(),
                            bold: false,
                            code: false,
                            link: true,
                        });
                        i = paren + 1;
                        continue;
                    }
                }
            }
        }

        cur.push(chars[i]);
        i += 1;
    }
    push_cur!();
    out
}

// ── 绘制 ──

/// 构造 LayoutJob。`line_h` 是行高，对应 CSS line-height。
fn job(spans: &[Span], pal: &Pal, width: f32, size: f32, line_h: f32) -> LayoutJob {
    let mut j = LayoutJob::default();
    j.wrap.max_width = width;
    j.wrap.break_anywhere = false;
    for sp in spans {
        let (font, color, bg) = if sp.code {
            (theme::mono(theme::F_CODE), pal.t1, pal.bg_soft)
        } else if sp.bold {
            (theme::font_bold(size), pal.t1, Color32::TRANSPARENT)
        } else if sp.link {
            (theme::font(size), pal.blue, Color32::TRANSPARENT)
        } else {
            (theme::font(size), pal.t1, Color32::TRANSPARENT)
        };
        j.append(
            &sp.text,
            0.0,
            TextFormat {
                font_id: font,
                color,
                background: bg,
                line_height: Some(line_h),
                ..Default::default()
            },
        );
    }
    j
}

/// 画一段带行内代码背景的文本。
///
/// egui 的 TextFormat.background 不给内边距，而设计稿的 code 是
/// `padding:1px 5px` 加 1px 边框，所以按 glyph 的实际矩形自己补一层。
fn draw_spans(
    ui: &egui::Ui,
    spans: &[Span],
    pal: &Pal,
    left: f32,
    top: f32,
    width: f32,
    size: f32,
    line_h: f32,
) -> f32 {
    let j = job(spans, pal, width, size, line_h);
    let galley = ui.fonts(|f| f.layout_job(j));

    // 先画行内代码背景，再画文字，避免盖住字形
    for row in &galley.rows {
        let mut i = 0;
        while i < row.glyphs.len() {
            let sec = row.glyphs[i].section_index as usize;
            let is_code = spans.get(sec).map(|s| s.code).unwrap_or(false);
            if !is_code {
                i += 1;
                continue;
            }
            let start = i;
            while i < row.glyphs.len() && row.glyphs[i].section_index as usize == sec {
                i += 1;
            }
            let first = &row.glyphs[start];
            let last = &row.glyphs[i - 1];
            let x0 = left + first.pos.x - 5.0;
            let x1 = left + last.pos.x + last.advance_width + 5.0;
            let gh = first.font_height;
            let rect = Rect::from_min_max(
                Pos2::new(x0, top + row.rect.top() + 1.0),
                Pos2::new(x1, top + row.rect.top() + gh + 1.0),
            );
            ui.painter().rect_filled(rect, Rounding::same(4.0), pal.bg_soft);
            ui.painter()
                .rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, pal.line));
        }
    }

    let h = galley.size().y;
    ui.painter().galley(Pos2::new(left, top), galley, pal.t1);
    h
}

/// 渲染一整段助手正文，返回占用的高度。
pub fn draw(ui: &mut egui::Ui, pal: &Pal, text: &str, left: f32, top: f32, width: f32) -> f32 {
    let blocks = parse(text);
    let mut y = top;

    for b in blocks {
        match b {
            Block::Heading(level, spans) => {
                // .ai-text h1-h4：margin:18px 0 8px，字号 18/16/14.5/13.5
                let size = match level {
                    1 => 18.0,
                    2 => 16.0,
                    3 => 14.5,
                    _ => 13.5,
                };
                let mut bold_spans = spans;
                for s in &mut bold_spans {
                    s.bold = true;
                }
                y += 18.0;
                y += draw_spans(ui, &bold_spans, pal, left, y, width, size, size * 1.4);
                y += 8.0;
            }
            Block::Para(spans) => {
                // .ai-text：13.5px / line-height 23px
                y += draw_spans(ui, &spans, pal, left, y, width, theme::F_AI, 23.0);
                y += 12.0;
            }
            Block::Bullet(spans) => {
                // padding-left:22px
                ui.painter().circle_filled(Pos2::new(left + 7.0, y + 11.0), 2.2, pal.t2);
                y += draw_spans(ui, &spans, pal, left + 22.0, y, width - 22.0, theme::F_AI, 23.0);
                y += 4.0;
            }
            Block::Numbered(n, spans) => {
                ui.painter().text(
                    Pos2::new(left + 3.0, y + 3.0),
                    egui::Align2::LEFT_TOP,
                    format!("{n}."),
                    theme::font(theme::F_AI),
                    pal.t2,
                );
                y += draw_spans(ui, &spans, pal, left + 22.0, y, width - 22.0, theme::F_AI, 23.0);
                y += 4.0;
            }
            Block::Quote(spans) => {
                // border-left:3px solid line-3，padding:2px 0 2px 14px
                let start = y;
                y += 2.0;
                y += draw_spans(ui, &spans, pal, left + 14.0, y, width - 14.0, theme::F_AI, 23.0);
                y += 2.0;
                ui.painter().rect_filled(
                    Rect::from_min_max(Pos2::new(left, start), Pos2::new(left + 3.0, y)),
                    0.0,
                    pal.line3,
                );
                y += 12.0;
            }
            Block::Rule => {
                // hr：1px line，margin:16px 0
                y += 16.0;
                ui.painter()
                    .hline(left..=(left + width), y, Stroke::new(1.0_f32, pal.line));
                y += 16.0;
            }
            Block::Code(src, lang) => {
                // .code-block：8px 圆角边框，头部 6px 10px 带语言标签与复制按钮
                // pre：bg-soft + 1px line + 8px 圆角 + padding 12px 14px
                let body = src.trim_end();
                let mut j = LayoutJob::default();
                j.wrap.max_width = width - 28.0;
                j.append(
                    body,
                    0.0,
                    TextFormat {
                        font_id: theme::mono(theme::F_CODE),
                        color: pal.t1,
                        line_height: Some(theme::F_CODE * 1.7),
                        ..Default::default()
                    },
                );
                let galley = ui.fonts(|f| f.layout_job(j));
                let head_h = if lang.is_empty() { 0.0 } else { 29.0 };
                let box_h = galley.size().y + 24.0 + head_h;
                let rect = Rect::from_min_size(Pos2::new(left, y), egui::vec2(width, box_h));
                let p = ui.painter();
                p.rect_filled(rect, Rounding::same(8.0), pal.bg_soft);
                p.rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0_f32, pal.line));
                if head_h > 0.0 {
                    let head = Rect::from_min_size(rect.min, egui::vec2(width, head_h));
                    p.rect_filled(
                        Rect::from_min_max(head.min, Pos2::new(head.right(), head.bottom() - 1.0)),
                        Rounding { nw: 8.0, ne: 8.0, sw: 0.0, se: 0.0 },
                        pal.surface2,
                    );
                    p.hline(head.x_range(), head.bottom(), Stroke::new(1.0_f32, pal.line));
                    p.text(
                        Pos2::new(rect.left() + 10.0, head.center().y),
                        egui::Align2::LEFT_CENTER,
                        lang.to_uppercase(),
                        theme::mono(10.0),
                        pal.t4,
                    );
                    let bw = 44.0;
                    let btn = Rect::from_min_size(
                        Pos2::new(rect.right() - 10.0 - bw, head.center().y - 10.0),
                        egui::vec2(bw, 20.0),
                    );
                    p.rect_filled(btn, Rounding::same(4.0), pal.surface);
                    p.rect_stroke(btn, Rounding::same(4.0), Stroke::new(1.0_f32, pal.line2));
                    p.text(btn.center(), egui::Align2::CENTER_CENTER, "复制", theme::font(10.5), pal.t3);
                }
                ui.painter().galley(Pos2::new(left + 14.0, y + head_h + 12.0), galley, pal.t1);
                y += box_h + 12.0;
            }
            Block::Table(rows) => {
                y += draw_table(ui, pal, &rows, left, y, width);
                y += 12.0;
            }
        }
    }
    y - top
}

/// 表格：table-layout:fixed 等分列宽，单元格 padding 6px 10px，换行不截断。
fn draw_table(
    ui: &egui::Ui,
    pal: &Pal,
    rows: &[TableRow],
    left: f32,
    top: f32,
    width: f32,
) -> f32 {
    let cols = rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    if cols == 0 {
        return 0.0;
    }
    let col_w = width / cols as f32;
    let mut y = top;

    for row in rows {
        // 行高取各单元格里最高的那个
        let mut cell_galleys = Vec::new();
        let mut row_h: f32 = 0.0;
        for ci in 0..cols {
            let empty = Vec::new();
            let spans = row.cells.get(ci).unwrap_or(&empty);
            let mut spans = spans.clone();
            if row.header {
                for s in &mut spans {
                    s.bold = true;
                }
            }
            let j = job(&spans, pal, col_w - 20.0, 12.5, 19.0);
            let g = ui.fonts(|f| f.layout_job(j));
            row_h = row_h.max(g.size().y);
            cell_galleys.push(g);
        }
        let h = row_h + 12.0; // padding 6px 上下

        for ci in 0..cols {
            let x = left + col_w * ci as f32;
            let cell = Rect::from_min_size(Pos2::new(x, y), egui::vec2(col_w, h));
            if row.header {
                ui.painter().rect_filled(cell, Rounding::ZERO, pal.bg_soft);
            }
            // border:1px solid line-2
            ui.painter()
                .rect_stroke(cell, Rounding::ZERO, Stroke::new(1.0_f32, pal.line2));
            ui.painter().galley(
                Pos2::new(x + 10.0, y + 6.0),
                cell_galleys[ci].clone(),
                pal.t1,
            );
        }
        y += h;
    }
    y - top
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_code_fence() {
        let blocks = parse("前\n```\nlet x = 1;\n```\n后");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(&blocks[1], Block::Code(s, _) if s.contains("let x = 1;")));
    }

    #[test]
    fn unclosed_fence_still_renders() {
        let blocks = parse("```\nlet x = 1;");
        assert!(
            blocks.iter().any(|b| matches!(b, Block::Code(s, _) if s.contains("let x"))),
            "未闭合的代码块不能丢内容"
        );
    }

    #[test]
    fn inline_code_split() {
        let spans = inline("入参增加 `idempotencyKey` 字段");
        assert_eq!(spans.len(), 3);
        assert!(!spans[0].code);
        assert!(spans[1].code);
        assert_eq!(spans[1].text, "idempotencyKey");
    }

    #[test]
    fn lone_backtick_is_text() {
        let spans = inline("a ` b");
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].code);
    }

    #[test]
    fn bold_parsed() {
        let spans = inline("**已赢下**这局");
        assert_eq!(spans.len(), 2);
        assert!(spans[0].bold);
        assert_eq!(spans[0].text, "已赢下");
    }

    #[test]
    fn unclosed_bold_is_text() {
        let spans = inline("**没闭合");
        assert_eq!(spans.len(), 1);
        assert!(!spans[0].bold);
    }

    #[test]
    fn bullets_and_numbers() {
        let blocks = parse("- 第一\n- 第二\n1. 有序");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0], Block::Bullet(_)));
        assert!(matches!(blocks[2], Block::Numbered(1, _)));
    }

    #[test]
    fn paragraph_joins_lines() {
        let blocks = parse("第一行\n第二行");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Para(spans) => assert_eq!(spans[0].text, "第一行 第二行"),
            _ => panic!("应为段落"),
        }
    }

    #[test]
    fn headings_parsed() {
        let blocks = parse("# 一级\n## 二级\n### 三级");
        assert_eq!(blocks.len(), 3);
        assert!(matches!(blocks[0], Block::Heading(1, _)));
        assert!(matches!(blocks[2], Block::Heading(3, _)));
    }

    #[test]
    fn table_parsed() {
        let blocks = parse("| 能力 | 说明 |\n|---|---|\n| 写代码 | Java |\n| Debug | 根因 |");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Table(rows) => {
                assert_eq!(rows.len(), 3, "表头 + 两行数据");
                assert!(rows[0].header);
                assert_eq!(rows[0].cells.len(), 2);
                assert!(!rows[1].header);
            }
            _ => panic!("应解析成表格"),
        }
    }

    #[test]
    fn table_without_sep_is_paragraph() {
        let blocks = parse("| 不是 | 表格 |\n普通行");
        assert!(matches!(&blocks[0], Block::Para(_)), "缺少分隔行的竖线文本应退化为段落");
    }

    #[test]
    fn quote_parsed() {
        let blocks = parse("> 引用内容");
        assert!(matches!(&blocks[0], Block::Quote(_)));
    }

    #[test]
    fn rule_parsed() {
        let blocks = parse("---");
        assert!(matches!(&blocks[0], Block::Rule));
    }
}

/// 无 Ui 上下文的绘制入口（文件预览用 painter 直绘）。
///
/// 与 draw() 逻辑一致，只是拿不到 Ui 时用 ctx.fonts 排版、painter 落笔。
pub fn draw_painter(
    p: &egui::Painter,
    ctx: &egui::Context,
    pal: &Pal,
    text: &str,
    left: f32,
    top: f32,
    width: f32,
) -> f32 {
    let blocks = parse(text);
    let mut y = top;
    let lay = |spans: &[Span], size: f32, lh: f32, w: f32| -> std::sync::Arc<egui::Galley> {
        let j = job(spans, pal, w, size, lh);
        ctx.fonts(|f| f.layout_job(j))
    };

    for b in blocks {
        match b {
            Block::Heading(level, spans) => {
                let size = match level {
                    1 => 22.0,
                    2 => 16.0,
                    3 => 14.5,
                    _ => 13.5,
                };
                let mut sp = spans;
                for s in &mut sp {
                    s.bold = true;
                }
                y += if level == 1 { 0.0 } else { 26.0 };
                let g = lay(&sp, size, size * 1.4, width);
                let h = g.size().y;
                p.galley(Pos2::new(left, y), g, pal.t1);
                y += h;
                // h2 有下边框
                if level == 2 {
                    y += 6.0;
                    p.hline(left..=(left + width), y, Stroke::new(1.0_f32, pal.line));
                }
                y += if level == 1 { 16.0 } else { 10.0 };
            }
            Block::Para(spans) => {
                let g = lay(&spans, theme::F_AI, 23.0, width);
                let h = g.size().y;
                p.galley(Pos2::new(left, y), g, pal.t1);
                y += h + 12.0;
            }
            Block::Bullet(spans) => {
                p.circle_filled(Pos2::new(left + 7.0, y + 11.0), 2.2, pal.t2);
                let g = lay(&spans, theme::F_AI, 23.0, width - 22.0);
                let h = g.size().y;
                p.galley(Pos2::new(left + 22.0, y), g, pal.t1);
                y += h + 4.0;
            }
            Block::Numbered(n, spans) => {
                p.text(
                    Pos2::new(left + 3.0, y + 3.0),
                    egui::Align2::LEFT_TOP,
                    format!("{n}."),
                    theme::font(theme::F_AI),
                    pal.t2,
                );
                let g = lay(&spans, theme::F_AI, 23.0, width - 22.0);
                let h = g.size().y;
                p.galley(Pos2::new(left + 22.0, y), g, pal.t1);
                y += h + 4.0;
            }
            Block::Quote(spans) => {
                let start = y;
                y += 2.0;
                let g = lay(&spans, theme::F_AI, 23.0, width - 14.0);
                let h = g.size().y;
                p.galley(Pos2::new(left + 14.0, y), g, pal.t1);
                y += h + 2.0;
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(left, start), Pos2::new(left + 3.0, y)),
                    Rounding::ZERO,
                    pal.line3,
                );
                y += 12.0;
            }
            Block::Rule => {
                y += 16.0;
                p.hline(left..=(left + width), y, Stroke::new(1.0_f32, pal.line));
                y += 16.0;
            }
            Block::Code(src, lang) => {
                let body = src.trim_end();
                let mut j = LayoutJob::default();
                j.wrap.max_width = width - 28.0;
                j.append(
                    body,
                    0.0,
                    TextFormat {
                        font_id: theme::mono(theme::F_CODE),
                        color: pal.t1,
                        line_height: Some(theme::F_CODE * 1.7),
                        ..Default::default()
                    },
                );
                let g = ctx.fonts(|f| f.layout_job(j));
                let head_h = if lang.is_empty() { 0.0 } else { 29.0 };
                let box_h = g.size().y + 24.0 + head_h;
                let rect = Rect::from_min_size(Pos2::new(left, y), egui::vec2(width, box_h));
                p.rect_filled(rect, Rounding::same(8.0), pal.bg_soft);
                p.rect_stroke(rect, Rounding::same(8.0), Stroke::new(1.0_f32, pal.line));
                if head_h > 0.0 {
                    // 头部：surface-2 底 + 底边分隔线 + 语言标签 + 复制按钮
                    let head = Rect::from_min_size(rect.min, egui::vec2(width, head_h));
                    p.rect_filled(
                        Rect::from_min_max(
                            head.min,
                            Pos2::new(head.right(), head.bottom() - 1.0),
                        ),
                        Rounding { nw: 8.0, ne: 8.0, sw: 0.0, se: 0.0 },
                        pal.surface2,
                    );
                    p.hline(head.x_range(), head.bottom(), Stroke::new(1.0_f32, pal.line));
                    p.text(
                        Pos2::new(rect.left() + 10.0, head.center().y),
                        egui::Align2::LEFT_CENTER,
                        lang.to_uppercase(),
                        theme::mono(10.0),
                        pal.t4,
                    );
                    // 复制按钮 20px 高
                    let bw = 44.0;
                    let btn = Rect::from_min_size(
                        Pos2::new(rect.right() - 10.0 - bw, head.center().y - 10.0),
                        egui::vec2(bw, 20.0),
                    );
                    p.rect_filled(btn, Rounding::same(4.0), pal.surface);
                    p.rect_stroke(btn, Rounding::same(4.0), Stroke::new(1.0_f32, pal.line2));
                    p.text(
                        btn.center(),
                        egui::Align2::CENTER_CENTER,
                        "复制",
                        theme::font(10.5),
                        pal.t3,
                    );
                }
                p.galley(Pos2::new(left + 14.0, y + head_h + 12.0), g, pal.t1);
                y += box_h + 12.0;
            }
            Block::Table(rows) => {
                let cols = rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
                if cols > 0 {
                    let col_w = width / cols as f32;
                    for row in &rows {
                        let mut galleys = Vec::new();
                        let mut row_h: f32 = 0.0;
                        for ci in 0..cols {
                            let empty = Vec::new();
                            let spans = row.cells.get(ci).unwrap_or(&empty);
                            let mut sp = spans.clone();
                            if row.header {
                                for s in &mut sp {
                                    s.bold = true;
                                }
                            }
                            let g = lay(&sp, 12.5, 19.0, col_w - 20.0);
                            row_h = row_h.max(g.size().y);
                            galleys.push(g);
                        }
                        let rh = row_h + 12.0;
                        for ci in 0..cols {
                            let cell = Rect::from_min_size(
                                Pos2::new(left + col_w * ci as f32, y),
                                egui::vec2(col_w, rh),
                            );
                            if row.header {
                                p.rect_filled(cell, Rounding::ZERO, pal.bg_soft);
                            }
                            p.rect_stroke(cell, Rounding::ZERO, Stroke::new(1.0_f32, pal.line2));
                            p.galley(
                                Pos2::new(cell.left() + 10.0, cell.top() + 6.0),
                                galleys[ci].clone(),
                                pal.t1,
                            );
                        }
                        y += rh;
                    }
                }
                y += 12.0;
            }
        }
    }
    y - top
}
