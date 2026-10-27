//! 管理 claude 子进程，按 docs/claude-cli-protocol.md 解析输出。
//!
//! 不依赖任何 SDK：拼参数、spawn、写 stdin、逐行读 stdout。

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

pub const MAX_OUTPUT_CHARS: usize = 400_000;
pub const MAX_TOOLS: usize = 500;

#[derive(Debug, Clone)]
pub enum Event {
    Text(String),
    Tool(String, String),
    ClaudeSession(String),
    Context { total: i64, max: i64, percent: f64 },
    Error(String),
}

pub struct Options {
    pub cwd: String,
    pub text: String,
    pub model: String,
    pub resume: String,
    pub mode: String,
    pub env: Vec<(String, String)>,
}

/// 跑一轮对话，事件通过回调推给调用方。
pub fn run(opts: Options, mut on_event: impl FnMut(Event)) -> Result<(), String> {
    let mut args: Vec<String> = vec![
        "--print".into(),
        "--input-format".into(),
        "stream-json".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--include-partial-messages".into(),
    ];
    let mode = if opts.mode.is_empty() { "acceptEdits" } else { &opts.mode };
    args.push("--permission-mode".into());
    args.push(mode.into());
    if !opts.resume.is_empty() {
        args.push("--resume".into());
        args.push(opts.resume.clone());
    }
    if !opts.model.is_empty() {
        args.push("--model".into());
        args.push(opts.model.clone());
    }

    let mut cmd = build_command(&args);
    cmd.current_dir(&opts.cwd);
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());

    let mut child = cmd.spawn().map_err(|e| format!("启动 claude 失败（确认已在 PATH 中）: {e}"))?;

    // 写一行 NDJSON。不立刻关 stdin：实测关闭过快只能收到 hook 事件。
    if let Some(mut stdin) = child.stdin.take() {
        let payload = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{ "type": "text", "text": opts.text }]
            }
        });
        let mut line = payload.to_string();
        line.push('\n');
        stdin.write_all(line.as_bytes()).map_err(|e| format!("写入 stdin 失败: {e}"))?;
        let _ = stdin.flush();
        // stdin 在这里 drop，子进程读到 EOF 后正常结束
    }

    let stdout = child.stdout.take().ok_or("无法获取 stdout")?;
    let reader = BufReader::new(stdout);

    let mut st = State::default();
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let t = line.trim();
        if t.is_empty() || !t.starts_with('{') {
            continue; // 非 JSON 行跳过
        }
        if let Ok(v) = serde_json::from_str::<Value>(t) {
            st.handle(&v, &mut on_event);
        }
    }

    let _ = child.wait();
    st.finish(&mut on_event);
    Ok(())
}

fn build_command(args: &[String]) -> Command {
    #[cfg(target_os = "windows")]
    {
        // npm 装的 claude 是 .cmd 垫片，直接 spawn 会 ENOENT
        let mut c = Command::new("cmd.exe");
        c.arg("/d").arg("/s").arg("/c").arg("claude");
        c.args(args);
        c
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut c = Command::new("claude");
        c.args(args);
        c
    }
}

#[derive(Default)]
struct State {
    acc: String,
    tools: usize,
    run_err: String,
    // 空闲超时用：记录最后一条有效事件的时间
}

impl State {
    fn handle(&mut self, msg: &Value, emit: &mut impl FnMut(Event)) {
        let typ = msg.get("type").and_then(Value::as_str).unwrap_or("");
        let sub = msg.get("subtype").and_then(Value::as_str).unwrap_or("");

        // hook 事件必须整类丢掉：实测可吐出数万字符
        if typ == "system" && sub.starts_with("hook_") {
            return;
        }
        // 子代理的事件不能混进主回复
        if msg.get("parent_tool_use_id").is_some_and(|v| !v.is_null()) {
            return;
        }

        match typ {
            "system" if sub == "init" => {
                if let Some(sid) = msg.get("session_id").and_then(Value::as_str) {
                    emit(Event::ClaudeSession(sid.to_string()));
                }
            }
            "stream_event" => {
                if let Some(d) = delta_text(msg) {
                    if self.acc.len() < MAX_OUTPUT_CHARS {
                        self.acc.push_str(&d);
                        emit(Event::Text(d));
                    }
                }
            }
            "assistant" => self.handle_assistant(msg, emit),
            "result" => self.handle_result(msg, emit),
            _ => {}
        }
    }

    fn handle_assistant(&mut self, msg: &Value, emit: &mut impl FnMut(Event)) {
        let blocks = msg
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array);
        let Some(blocks) = blocks else { return };

        for b in blocks {
            match b.get("type").and_then(Value::as_str) {
                Some("text") => {
                    // stream_event 已累积过就不重复加
                    if self.acc.is_empty() {
                        if let Some(t) = b.get("text").and_then(Value::as_str) {
                            self.acc.push_str(t);
                            emit(Event::Text(t.to_string()));
                        }
                    }
                }
                Some("tool_use") => {
                    if self.tools >= MAX_TOOLS {
                        continue;
                    }
                    self.tools += 1;
                    let name = b.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                    let target = b
                        .get("input")
                        .and_then(pick_target)
                        .unwrap_or_default();
                    emit(Event::Tool(name, target));
                }
                _ => {}
            }
        }
    }

    fn handle_result(&mut self, msg: &Value, emit: &mut impl FnMut(Event)) {
        if msg.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
            self.run_err = msg
                .get("errors")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("；")
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    if msg.get("subtype").and_then(Value::as_str) == Some("error_max_turns") {
                        "已达到本轮最大轮次限制".into()
                    } else {
                        "模型返回了错误结果".into()
                    }
                });
        }

        let usage = msg.get("usage");
        let input = usage.and_then(|u| u.get("input_tokens")).and_then(Value::as_i64).unwrap_or(0);
        let output = usage.and_then(|u| u.get("output_tokens")).and_then(Value::as_i64).unwrap_or(0);
        let cache_read = usage
            .and_then(|u| u.get("cache_read_input_tokens"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let cache_create = usage
            .and_then(|u| u.get("cache_creation_input_tokens"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let used = input + cache_read + cache_create + output;

        // 分母取 modelUsage 里 contextWindow 最大的那条
        let mut max_tokens = 0i64;
        if let Some(mu) = msg.get("modelUsage").and_then(Value::as_object) {
            for info in mu.values() {
                let cw = info.get("contextWindow").and_then(Value::as_i64).unwrap_or(0);
                max_tokens = max_tokens.max(cw);
            }
        }
        if max_tokens > 0 {
            let pct = (used as f64 / max_tokens as f64 * 1000.0).round() / 10.0;
            emit(Event::Context { total: used, max: max_tokens, percent: pct });
        }
    }

    fn finish(&mut self, emit: &mut impl FnMut(Event)) {
        if !self.run_err.is_empty() {
            emit(Event::Error(self.run_err.clone()));
            return;
        }
        if self.acc.is_empty() && self.tools == 0 {
            emit(Event::Error("模型没有返回任何内容，请重试".into()));
        }
    }
}

/// 从 stream_event 里取 text_delta。
fn delta_text(msg: &Value) -> Option<String> {
    let ev = msg.get("event")?;
    if ev.get("type").and_then(Value::as_str) != Some("content_block_delta") {
        return None;
    }
    let d = ev.get("delta")?;
    if d.get("type").and_then(Value::as_str) != Some("text_delta") {
        return None;
    }
    d.get("text").and_then(Value::as_str).map(str::to_string)
}

fn pick_target(input: &Value) -> Option<String> {
    for k in ["file_path", "command", "pattern", "path", "url"] {
        if let Some(s) = input.get(k).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.chars().take(500).collect());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(lines: &[&str]) -> (String, usize, String) {
        let mut st = State::default();
        let mut text = String::new();
        let mut tools = 0;
        let mut err = String::new();
        for l in lines {
            if let Ok(v) = serde_json::from_str::<Value>(l) {
                st.handle(&v, &mut |e| match e {
                    Event::Text(t) => text.push_str(&t),
                    Event::Tool(..) => tools += 1,
                    Event::Error(m) => err = m,
                    _ => {}
                });
            }
        }
        st.finish(&mut |e| {
            if let Event::Error(m) = e {
                err = m;
            }
        });
        (text, tools, err)
    }

    #[test]
    fn filters_hook_events() {
        let (text, _, _) = feed(&[
            r#"{"type":"system","subtype":"hook_started","hook_name":"SessionStart"}"#,
            r#"{"type":"system","subtype":"hook_response","output":"xxxxx"}"#,
            r#"{"type":"system","subtype":"init","session_id":"s1"}"#,
        ]);
        assert_eq!(text, "", "hook 事件不应产出文本");
    }

    #[test]
    fn filters_subagent() {
        let (text, _, _) = feed(&[
            r#"{"type":"stream_event","parent_tool_use_id":"t1","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"子代理"}}}"#,
            r#"{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"主回复"}}}"#,
        ]);
        assert_eq!(text, "主回复");
    }

    #[test]
    fn ignores_thinking_delta() {
        let (text, _, _) = feed(&[
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"内部"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"可见"}}}"#,
        ]);
        assert_eq!(text, "可见");
    }

    #[test]
    fn parses_tool_calls() {
        let (_, tools, _) = feed(&[
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/b.ts"}}]}}"#,
        ]);
        assert_eq!(tools, 1);
    }

    #[test]
    fn reports_is_error() {
        let (_, _, err) = feed(&[r#"{"type":"result","is_error":true,"errors":["密钥无效"]}"#]);
        assert!(err.contains("密钥无效"), "错误应透出原始信息，实际: {err}");
    }

    #[test]
    fn empty_result_reports_error() {
        let (_, _, err) = feed(&[r#"{"type":"result","is_error":false,"usage":{"input_tokens":5}}"#]);
        assert!(err.contains("没有返回"), "无内容时应提示重试，实际: {err}");
    }

    #[test]
    fn context_includes_cache_tokens() {
        let mut st = State::default();
        let mut pct = 0.0;
        let line = r#"{"type":"result","is_error":false,"usage":{"input_tokens":1000,"output_tokens":0,"cache_read_input_tokens":9000},"modelUsage":{"m1":{"contextWindow":100000}}}"#;
        let v: Value = serde_json::from_str(line).unwrap();
        st.handle(&v, &mut |e| {
            if let Event::Context { percent, .. } = e {
                pct = percent;
            }
        });
        assert_eq!(pct, 10.0, "cache token 漏算会让百分比变成 1");
    }
}

#[cfg(test)]
mod real_tests {
    use super::*;

    /// 真实调用 claude 验证端到端链路。
    /// `cargo test --release -- --ignored` 才会跑。
    #[test]
    #[ignore]
    fn real_chat_roundtrip() {
        let mut acc = String::new();
        let mut got_session = false;
        let res = run(
            Options {
                cwd: "D:/Project/AI/CCWorkspace".into(),
                text: "只回复两个字：收到".into(),
                model: String::new(),
                resume: String::new(),
                mode: "acceptEdits".into(),
                env: vec![],
            },
            |e| match e {
                Event::Text(t) => acc.push_str(&t),
                Event::ClaudeSession(_) => got_session = true,
                _ => {}
            },
        );
        assert!(res.is_ok(), "对话失败: {res:?}");
        assert!(!acc.is_empty(), "没有收到任何文本");
        assert!(got_session, "没有收到 CLI 会话 id");
        println!("回复: {acc:?}");
    }
}
