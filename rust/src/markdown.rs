//! 极简 Markdown 渲染，对齐原型 `.ai-text` 的排版规则。
//!
//! 只处理对话里真正会出现的几类：段落、行内代码、围栏代码块、无序/有序列表。
//! 不引第三方解析库：需要控制的只有字号/行高/背景色，自己切行更直接，
//! 也不用担心库的默认样式和设计稿打架。

use crate::theme::{self, Pal};
use egui::{text::LayoutJob, Color32, Rounding, Stroke, TextFormat};

/// 行内片段。
struct Span {
    text: String,
    code: bool,
}

/// 一个排版单元。
enum Block {
    Para(Vec<Span>),
    Code(String),
    Bullet(Vec<Span>),
    Numbered(usize, Vec<Span>),
}

/// 把文本切成一串块。
fn parse(src: &str) -> Vec<Block> {
    let mut out = Vec::new();
    let mut buf: Vec<&str> = Vec::new();
    let mut code: Option<(String, usize)> = None;

    for line in src.lines() {
        // 围栏代码块：进入/退出
        if let Some((src_buf, _)) = &mut code {
            if line.trim_start().starts_with("```") {
                out.push(Block::Code(std::mem::take(src_buf)));
                code = None;
            } else {
                src_buf.push_str(line);
                src_buf.push('\n');
            }
            continue;
        }
        if line.trim_start().starts_with("```") {
            flush_para(&mut buf, &mut out);
            code = Some((String::new(), 0));
            continue;
        }

        let trimmed = line.trim_start();
        if line.trim().is_empty() {
            flush_para(&mut buf, &mut out);
        } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            flush_para(&mut buf, &mut out);
            out.push(Block::Bullet(inline(&trimmed[2..])));
        } else if let Some((n, rest)) = numbered(trimmed) {
            flush_para(&mut buf, &mut out);
            out.push(Block::Numbered(n, inline(rest)));
        } else {
            buf.push(trimmed);
        }
    }
    // 未闭合的代码块也要显示出来，否则内容凭空消失
    if let Some((s, _)) = code {
        out.push(Block::Code(s));
    }
    flush_para(&mut buf, &mut out);
    out
}

fn flush_para(buf: &mut Vec<&str>, out: &mut Vec<Block>) {
    if buf.is_empty() {
        return;
    }
    let joined = buf.join(" ");
    buf.clear();
    out.push(Block::Para(inline(&joined)));
}

/// `12. xxx` → (12, "xxx")
fn numbered(s: &str) -> Option<(usize, &str)> {
    let dot = s.find(". ")?;
    let n: usize = s[..dot].parse().ok()?;
    Some((n, &s[dot + 2..]))
}

/// 切出行内代码：`code` 之外都是普通文本。
fn inline(s: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find('`') {
        if start > 0 {
            out.push(Span { text: rest[..start].to_string(), code: false });
        }
        let after = &rest[start + 1..];
        match after.find('`') {
            Some(end) => {
                out.push(Span { text: after[..end].to_string(), code: true });
                rest = &after[end + 1..];
            }
            None => {
                // 落单的反引号当普通字符，整段合并回一个 span
                let tail = &rest[start..];
                match out.last_mut() {
                    Some(last) if !last.code => last.text.push_str(tail),
                    _ => out.push(Span { text: tail.to_string(), code: false }),
                }
                rest = "";
            }
        }
    }
    if !rest.is_empty() {
        out.push(Span { text: rest.to_string(), code: false });
    }
    out
}

/// 构造一段 LayoutJob：普通文本用 .ai-text 字号行高，行内代码换等宽字 + 背景。
fn job(spans: &[Span], pal: &Pal, width: f32, indent: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    job.wrap.break_anywhere = false;

    // .ai-text line-height:1.75
    let body_font = theme::font(theme::F_AI);

    if indent > 0.0 {
        job.append(
            " ",
            0.0,
            TextFormat {
                font_id: body_font.clone(),
                color: Color32::TRANSPARENT,
                line_height: Some(theme::F_AI * 1.75),
                ..Default::default()
            },
        );
    }

    for sp in spans {
        if sp.code {
            // 行内代码：等宽 + bg-soft 背景
            job.append(
                &sp.text,
                0.0,
                TextFormat {
                    font_id: theme::mono(theme::F_CODE),
                    color: pal.t1,
                    background: pal.bg_soft,
                    ..Default::default()
                },
            );
        } else {
            job.append(
                &sp.text,
                0.0,
                TextFormat {
                    font_id: body_font.clone(),
                    color: pal.t1,
                    // .ai-text line-height:1.75
                    line_height: Some(theme::F_AI * 1.75),
                    ..Default::default()
                },
            );
        }
    }
    job
}

/// 渲染一整段助手正文，返回占用的高度。
pub fn draw(
    ui: &mut egui::Ui,
    pal: &Pal,
    text: &str,
    left: f32,
    top: f32,
    width: f32,
) -> f32 {
    let blocks = parse(text);
    let mut y = top;

    for b in blocks {
        match b {
            Block::Para(spans) => {
                let job = job(&spans, pal, width, 0.0);
                let galley = ui.fonts(|f| f.layout_job(job));
                let h = galley.size().y;
                ui.painter().galley(egui::pos2(left, y), galley, pal.t1);
                y += h + 12.0; // .ai-text p margin-bottom:12px
            }
            Block::Bullet(spans) => {
                // 项目符号：圆点 + 缩进 20px
                ui.painter().circle_filled(egui::pos2(left + 6.0, y + 11.0), 2.2, pal.t2);
                let job = job(&spans, pal, width - 20.0, 0.0);
                let galley = ui.fonts(|f| f.layout_job(job));
                let h = galley.size().y;
                ui.painter().galley(egui::pos2(left + 20.0, y), galley, pal.t1);
                y += h + 4.0; // li margin-bottom:4px
            }
            Block::Numbered(n, spans) => {
                let num = format!("{n}.");
                ui.painter().text(
                    egui::pos2(left + 4.0, y + 4.0),
                    egui::Align2::LEFT_TOP,
                    num,
                    theme::font(theme::F_AI),
                    pal.t2,
                );
                let job = job(&spans, pal, width - 20.0, 0.0);
                let galley = ui.fonts(|f| f.layout_job(job));
                let h = galley.size().y;
                ui.painter().galley(egui::pos2(left + 20.0, y), galley, pal.t1);
                y += h + 4.0;
            }
            Block::Code(src) => {
                // .ai-text pre：bg-soft、1px line、8px 圆角、12px/14px 内边距
                let body = src.trim_end();
                let job = {
                    let mut j = LayoutJob::default();
                    j.wrap.max_width = width - 28.0;
                    j.append(
                        body,
                        0.0,
                        TextFormat {
                            font_id: theme::mono(theme::F_CODE),
                            color: pal.t1,
                            // pre 的 line-height:1.7
                            line_height: Some(theme::F_CODE * 1.7),
                            ..Default::default()
                        },
                    );
                    j
                };
                let galley = ui.fonts(|f| f.layout_job(job));
                let box_h = galley.size().y + 24.0;
                let rect = egui::Rect::from_min_size(
                    egui::pos2(left, y),
                    egui::vec2(width, box_h),
                );
                ui.painter()
                    .rect_filled(rect, Rounding::same(8.0), pal.bg_soft);
                ui.painter().rect_stroke(
                    rect,
                    Rounding::same(8.0),
                    Stroke::new(1.0_f32, pal.line),
                );
                ui.painter()
                    .galley(egui::pos2(left + 14.0, y + 12.0), galley, pal.t1);
                y += box_h + 12.0;
            }
        }
    }
    y - top
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_code_fence() {
        let blocks = parse("前\n```\nlet x = 1;\n```\n后");
        // Para + Code + Para
        assert_eq!(blocks.len(), 3);
        match &blocks[1] {
            Block::Code(s) => assert!(s.contains("let x = 1;")),
            _ => panic!("中间应是代码块"),
        }
    }

    #[test]
    fn unclosed_fence_still_renders() {
        let blocks = parse("```\nlet x = 1;");
        assert!(
            blocks.iter().any(|b| matches!(b, Block::Code(s) if s.contains("let x"))),
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
}
