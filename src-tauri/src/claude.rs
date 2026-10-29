//! claude CLI 子进程：拼参数、写 stdin、逐行解析 stream-json。
//!
//! 协议见 docs/claude-cli-protocol.md。不依赖任何 SDK。
//!
//! 停止与超时：
//! - 读 stdout 走独立线程 + 定时轮询。原来直接 `for line in reader.lines()` 是阻塞的，
//!   模型思考期间不产出任何行，停止标志永远读不到（用户点停止毫无反应）。
//! - 终止必须是整棵进程树。Windows 下命令是 `cmd.exe /d /s /c claude`，
//!   只杀 cmd 会让真正的 node 进程存活（实测验证过）。

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MAX_OUTPUT_CHARS: usize = 400_000;
pub const MAX_TOOLS: usize = 500;

/// 连续这么久没收到有效事件就判定连接卡死。
///
/// 深度思考时只有 `thinking_tokens` 心跳，间隔可达十几秒；
/// 长文档生成一轮可能跑几分钟。取值要明显大于单次思考间隔。
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// 轮询间隔：既约等于停止响应延迟，又不至于空转烧 CPU。
const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub enum Event {
    Text(String),
    Tool(String, String),
    ClaudeSession(String),
    Context {
        total: i64,
        max: i64,
        percent: f64,
    },
    /// init 事件带来的可用能力清单。CLI 是唯一权威来源：用户自定义 Skill、
    /// 用户命令、项目命令、插件命令与内置命令都在里面，扫目录只能拿到子集。
    Meta(Meta),
    Error(String),
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub slash_commands: Vec<String>,
    pub skills: Vec<String>,
    pub agents: Vec<String>,
    pub plugins: Vec<String>,
}

/// 一轮结束的方式。调用方据此决定要不要把消息标记为已停止。
#[derive(Debug, Default, Clone, Copy)]
pub struct Outcome {
    pub interrupted: bool,
    pub timed_out: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAttachment {
    pub media_type: String,
    pub data: String,
}

pub struct Options {
    pub cwd: String,
    pub text: String,
    pub model: String,
    pub resume: String,
    pub mode: String,
    pub env: Vec<(String, String)>,
    pub images: Vec<ImageAttachment>,
}

/// 跑一轮对话。`should_stop` 由调用方置位以中断。
pub fn run(
    opts: Options,
    should_stop: Arc<AtomicBool>,
    mut on_event: impl FnMut(Event),
) -> Result<Outcome, String> {
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
    if !opts.cwd.is_empty() {
        cmd.current_dir(&opts.cwd);
    }
    for (k, v) in &opts.env {
        cmd.env(k, v);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动 claude 失败（确认已在 PATH 中）: {e}"))?;

    // 写一行 NDJSON。不立刻关 stdin：实测关闭过快只能收到 hook 事件。
    if let Some(mut stdin) = child.stdin.take() {
        let mut content = vec![json!({ "type": "text", "text": opts.text })];
        for img in &opts.images {
            content.push(json!({
                "type": "image",
                "source": { "type": "base64", "media_type": img.media_type, "data": img.data }
            }));
        }
        let payload = json!({
            "type": "user",
            "message": { "role": "user", "content": content }
        });
        let mut line = payload.to_string();
        line.push('\n');
        // 附件过大时这一行可能非常长，写失败要给出可解释的原因
        stdin.write_all(line.as_bytes()).map_err(|e| {
            if opts.images.is_empty() {
                format!("写入 stdin 失败: {e}")
            } else {
                format!("图片过大或格式不支持，Claude 未能接收（{e}）")
            }
        })?;
        let _ = stdin.flush();
    }

    let stdout = child.stdout.take().ok_or("无法获取 stdout")?;

    // 读线程只管读行，不解析。主线程靠 recv_timeout 保持对停止与超时的响应能力
    let (tx, rx) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut st = State::default();
    let mut outcome = Outcome::default();
    let mut last_valid = Instant::now();

    loop {
        match rx.recv_timeout(POLL_INTERVAL) {
            Ok(line) => {
                let t = line.trim();
                if t.is_empty() || !t.starts_with('{') {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(t) else {
                    continue;
                };
                if is_valid_event(&v) {
                    last_valid = Instant::now();
                }
                st.handle(&v, &mut on_event);
            }
            Err(RecvTimeoutError::Timeout) => {
                if should_stop.load(Ordering::Relaxed) {
                    kill_tree(&mut child);
                    outcome.interrupted = true;
                    break;
                }
                if last_valid.elapsed() >= IDLE_TIMEOUT {
                    kill_tree(&mut child);
                    outcome.timed_out = true;
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    let _ = child.wait();
    let _ = reader.join();

    if outcome.timed_out {
        on_event(Event::Error(format!(
            "连接超时：{} 秒内没有收到任何响应。请检查网络与 ANTHROPIC_BASE_URL 配置",
            IDLE_TIMEOUT.as_secs()
        )));
        return Ok(outcome);
    }
    // 中断时不当作错误：已收到的内容要保留，界面显示「已停止生成」
    if !outcome.interrupted {
        st.finish(&mut on_event);
    }
    Ok(outcome)
}

/// 有效事件：证明连接还活着、模型还在干活。
///
/// `thinking_tokens` 必须算在内——模型深度思考时可能几十秒不产出一句文本，
/// 只发这一种事件（实测一次长生成里 1719 条）。不认它就会把"正在思考"
/// 误判成"连接超时"并杀掉进程，界面表现为「模型没有返回任何内容」。
///
/// hook 事件仍要排除：挂住的 hook 会不断刷新计时器，让超时永远触发不了。
fn is_valid_event(v: &Value) -> bool {
    let typ = v.get("type").and_then(Value::as_str).unwrap_or("");
    let sub = v.get("subtype").and_then(Value::as_str).unwrap_or("");
    match typ {
        "assistant" | "stream_event" | "result" => true,
        "system" => sub == "init" || sub == "thinking_tokens" || sub == "status",
        _ => false,
    }
}

/// 终止整棵进程树。
///
/// 只杀直接子进程是不够的：Windows 下启动链是 cmd.exe -> node -> 工具子进程，
/// 杀 cmd 之后 node 还在跑，继续往管道写，表现为「点了停止，十秒后内容一起涌出来」。
fn kill_tree(child: &mut Child) {
    let pid = child.id();

    #[cfg(target_os = "windows")]
    {
        // /T 终止整棵子树，/F 强制。退出码不可信，不检查
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(target_os = "windows"))]
    {
        // 子进程以自身为组长启动（spawn 时 process_group(0)），对负 pid 发信号即整组
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = child.kill();
    }
    #[cfg(target_os = "windows")]
    let _ = child.kill();
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
        use std::os::unix::process::CommandExt;
        let mut c = Command::new("claude");
        c.args(args);
        // 自成进程组，停止时才能整组一起收
        c.process_group(0);
        c
    }
}

#[derive(Default)]
struct State {
    acc: String,
    tools: usize,
    run_err: String,
    /// 是否收到过 result 事件。没收到说明 CLI 异常退出。
    got_result: bool,
}

impl State {
    fn handle(&mut self, msg: &Value, emit: &mut impl FnMut(Event)) {
        let typ = msg.get("type").and_then(Value::as_str).unwrap_or("");
        let sub = msg.get("subtype").and_then(Value::as_str).unwrap_or("");

        // hook 事件必须整类丢掉：实测可吐出数万字符
        if typ == "system" && sub.starts_with("hook_") {
            return;
        }
        // 子代理事件不能混进主回复
        if msg
            .get("parent_tool_use_id")
            .is_some_and(|v| !v.is_null())
        {
            return;
        }

        match typ {
            "system" if sub == "init" => {
                if let Some(sid) = msg.get("session_id").and_then(Value::as_str) {
                    emit(Event::ClaudeSession(sid.to_string()));
                }
                let meta = parse_meta(msg);
                if meta != Meta::default() {
                    emit(Event::Meta(meta));
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
            "result" => {
                self.got_result = true;
                self.handle_result(msg, emit);
            }
            _ => {}
        }
    }

    fn handle_assistant(&mut self, msg: &Value, emit: &mut impl FnMut(Event)) {
        let Some(blocks) = msg
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array)
        else {
            return;
        };
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
                    let name = b
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let target = b.get("input").and_then(pick_target).unwrap_or_default();
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

        // 上下文占用。分两步取：
        //
        // 1. 若本轮输出是 `/context` 的结果，直接解析它——这是 CLI 自己算的权威值。
        // 2. 否则用本轮 usage 的输入三项之和（单轮值，不含输出）。
        //
        // 不要做「记住历史峰值」的平滑：那假设上下文只增不减，但实测里
        // CLI 会清理/压缩早期内容，峰值会一直虚高。真实案例：某轮读 PDF 到
        // 142k，之后实际只剩 21k，而峰值让界面一直显示 14%。
        let mut max_tokens = 0i64;
        if let Some(mu) = msg.get("modelUsage").and_then(Value::as_object) {
            for info in mu.values() {
                let cw = info.get("contextWindow").and_then(Value::as_i64).unwrap_or(0);
                if cw > max_tokens {
                    max_tokens = cw;
                }
            }
        }

        // `/context` 的正文里有一行 `**Tokens:** 21.2k / 1m (2%)`
        if let Some(text) = msg.get("result").and_then(Value::as_str) {
            if let Some((used, max)) = parse_context_report(text) {
                let max = if max > 0 { max } else { max_tokens };
                if max > 0 {
                    let raw = used as f64 / max as f64 * 100.0;
                    let pct = (raw.clamp(0.0, 100.0) * 10.0).round() / 10.0;
                    emit(Event::Context { total: used, max, percent: pct });
                    return;
                }
            }
        }

        let usage = msg.get("usage");
        let g = |k: &str| usage.and_then(|u| u.get(k)).and_then(Value::as_i64).unwrap_or(0);
        let used = g("input_tokens") + g("cache_read_input_tokens") + g("cache_creation_input_tokens");

        if max_tokens > 0 && used > 0 {
            let raw = used as f64 / max_tokens as f64 * 100.0;
            let pct = (raw.clamp(0.0, 100.0) * 10.0).round() / 10.0;
            emit(Event::Context {
                total: used,
                max: max_tokens,
                percent: pct,
            });
        }
    }

    fn finish(&mut self, emit: &mut impl FnMut(Event)) {
        if !self.run_err.is_empty() {
            emit(Event::Error(self.run_err.clone()));
            return;
        }
        if !self.got_result {
            // 进程结束了都没见到 result：CLI 中途死了，不是模型的问题
            emit(Event::Error("CLI 异常退出，本轮没有正常结束，请重试".into()));
            return;
        }
        if self.acc.is_empty() && self.tools == 0 {
            emit(Event::Error("模型没有返回任何内容，请重试".into()));
        }
    }
}

/// 从 init 事件里取能力清单。
///
/// 实测字段：`slash_commands`(143) / `skills`(65) / `agents`(20) / `plugins`(21 个对象)。
/// 缺字段时留空，不报错——不同 CLI 版本的字段集合会变。
fn parse_meta(msg: &Value) -> Meta {
    let names = |key: &str| -> Vec<String> {
        msg.get(key)
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let plugins = msg
        .get("plugins")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    Meta {
        slash_commands: names("slash_commands"),
        skills: names("skills"),
        agents: names("agents"),
        plugins,
    }
}

/// 解析 `/context` 输出里的 `**Tokens:** 21.2k / 1m (2%)`。
///
/// 这是 CLI 自己算的上下文占用，比我们从 usage 推算的可靠。
fn parse_context_report(text: &str) -> Option<(i64, i64)> {
    let line = text.lines().find(|l| l.contains("Tokens:"))?;
    let num = |s: &str| -> Option<i64> {
        let s = s.trim();
        let (v, unit) = if let Some(x) = s.strip_suffix(['k', 'K']) {
            (x, 1_000f64)
        } else if let Some(x) = s.strip_suffix(['m', 'M']) {
            (x, 1_000_000f64)
        } else {
            (s, 1f64)
        };
        v.trim().parse::<f64>().ok().map(|n| (n * unit) as i64)
    };
    // 形如 "21.2k / 1m"
    let after = line.split("Tokens:").nth(1)?;
    let mut parts = after.split('/');
    let used = num(parts.next()?.trim_matches(|c: char| !c.is_alphanumeric() && c != '.'))?;
    let max_part = parts.next()?.split('(').next()?.trim();
    let max = num(max_part.trim_matches(|c: char| !c.is_alphanumeric() && c != '.')).unwrap_or(0);
    if used <= 0 {
        return None;
    }
    Some((used, max))
}

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

#[cfg(test)]mod tests {
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
    fn missing_result_reports_cli_exit() {
        let (_, _, err) = feed(&[
            r#"{"type":"system","subtype":"init","session_id":"s1"}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"一半"}}}"#,
        ]);
        assert!(err.contains("CLI 异常退出"), "缺 result 应判为 CLI 异常，实际: {err}");
    }

    #[test]
    fn valid_event_excludes_hooks() {
        let hook: Value =
            serde_json::from_str(r#"{"type":"system","subtype":"hook_response"}"#).unwrap();
        assert!(!is_valid_event(&hook), "hook 会无限刷新空闲计时器");

        let init: Value = serde_json::from_str(r#"{"type":"system","subtype":"init"}"#).unwrap();
        assert!(is_valid_event(&init));

        let delta: Value = serde_json::from_str(r#"{"type":"stream_event"}"#).unwrap();
        assert!(is_valid_event(&delta));
    }

    #[test]
    fn thinking_heartbeat_counts_as_valid() {
        // 实测一次长生成里有 1719 条 thinking_tokens 心跳。不认它的话，
        // 模型思考超过阈值就会被误判成连接超时并杀进程，
        // 界面表现为「模型没有返回任何内容」。
        let beat: Value =
            serde_json::from_str(r#"{"type":"system","subtype":"thinking_tokens"}"#).unwrap();
        assert!(is_valid_event(&beat), "思考心跳必须刷新空闲计时器");

        let status: Value =
            serde_json::from_str(r#"{"type":"system","subtype":"status"}"#).unwrap();
        assert!(is_valid_event(&status));
    }

    #[test]
    fn parses_init_meta() {
        let line = r#"{"type":"system","subtype":"init","session_id":"s1",
            "slash_commands":["clear","compact","java-style","opsx:apply"],
            "skills":["java-style"],
            "agents":["claude","test-engineer"],
            "plugins":[{"name":"superpowers"},{"name":"ponytail"}]}"#;
        let v: Value = serde_json::from_str(line).unwrap();
        let m = parse_meta(&v);
        assert_eq!(m.slash_commands.len(), 4);
        assert_eq!(m.skills, vec!["java-style"]);
        assert_eq!(m.agents.len(), 2);
        assert_eq!(m.plugins, vec!["superpowers", "ponytail"]);
    }

    #[test]
    fn parse_meta_tolerates_missing_fields() {
        let v: Value = serde_json::from_str(r#"{"type":"system","subtype":"init"}"#).unwrap();
        assert_eq!(parse_meta(&v), Meta::default(), "缺字段不该报错");
    }

    // ── 上下文占用 ──

    /// 连续喂多条 result，返回每条的 (percent, total, max)
    fn ctx_series(lines: &[&str]) -> Vec<(f64, i64, i64)> {
        let mut st = State::default();
        let mut out = Vec::new();
        for l in lines {
            let v: Value = serde_json::from_str(l).unwrap();
            st.handle(&v, &mut |e| {
                if let Event::Context { percent, total, max } = e {
                    out.push((percent, total, max));
                }
            });
        }
        out
    }

    #[test]
    fn context_counts_all_input_kinds() {
        // input + cache_read + cache_creation 三项相加才是本轮送进模型的总量
        let s = ctx_series(&[
            r#"{"type":"result","is_error":false,"usage":{"input_tokens":1000,"cache_read_input_tokens":9000,"cache_creation_input_tokens":0},"modelUsage":{"m1":{"contextWindow":100000}}}"#,
        ]);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, 10000, "三项必须都算进去");
        assert_eq!(s[0].0, 10.0, "实际 {}", s[0].0);
    }

    #[test]
    fn context_follows_current_value_not_peak() {
        // 关键回归：曾经用「历史峰值」平滑，导致读过大文件之后永远虚高。
        // 真实案例：某轮读 PDF 到 142k，之后实际只剩 21k，界面却一直显示 14%。
        // 上下文是会被清理的，必须跟着当前值走。
        let s = ctx_series(&[
            r#"{"type":"result","is_error":false,"usage":{"input_tokens":21342,"cache_read_input_tokens":72960,"cache_creation_input_tokens":0},"modelUsage":{"m1":{"contextWindow":1000000}}}"#,
            r#"{"type":"result","is_error":false,"usage":{"input_tokens":11138,"cache_read_input_tokens":36352,"cache_creation_input_tokens":0},"modelUsage":{"m1":{"contextWindow":1000000}}}"#,
        ]);
        assert_eq!(s[0].0, 9.4, "第一轮实际 {}", s[0].0);
        assert!(s[1].0 < s[0].0, "回落必须如实反映，实际 {} -> {}", s[0].0, s[1].0);
    }

    #[test]
    fn context_prefers_slash_context_report() {
        // `/context` 的输出里 CLI 自己算好了占用，必须优先用它。
        // 这份样本来自实测：CLI 说 21.2k / 1m，而 usage 三项会是另一个数。
        let report = r#"## Context Usage

**Model:** yyy[1m]  
**Tokens:** 21.2k / 1m (2%)

### Estimated usage by category"#;
        let line = format!(
            r#"{{"type":"result","is_error":false,"result":{:?},"usage":{{"input_tokens":1000,"cache_read_input_tokens":1000}},"modelUsage":{{"m1":{{"contextWindow":1000000}}}}}}"#,
            report
        );
        let s = ctx_series(&[&line]);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].1, 21_200, "应取 /context 报的数值");
        assert_eq!(s[0].2, 1_000_000);
        assert!((s[0].0 - 2.1).abs() < 0.2, "实际 {}", s[0].0);
    }

    #[test]
    fn parse_context_report_handles_units() {
        assert_eq!(
            parse_context_report("**Tokens:** 21.2k / 1m (2%)"),
            Some((21_200, 1_000_000))
        );
        assert_eq!(
            parse_context_report("Tokens: 950 / 200k (0%)"),
            Some((950, 200_000))
        );
        assert_eq!(
            parse_context_report("**Tokens:** 142.3k / 1.0m (14%)"),
            Some((142_300, 1_000_000))
        );
        assert_eq!(parse_context_report("没有这行"), None);
    }

    #[test]
    fn context_grows_with_real_usage() {
        // 真实增长必须体现出来
        let s = ctx_series(&[
            r#"{"type":"result","is_error":false,"usage":{"input_tokens":1000,"cache_read_input_tokens":1000,"cache_creation_input_tokens":0},"modelUsage":{"m1":{"contextWindow":100000}}}"#,
            r#"{"type":"result","is_error":false,"usage":{"input_tokens":5000,"cache_read_input_tokens":5000,"cache_creation_input_tokens":0},"modelUsage":{"m1":{"contextWindow":100000}}}"#,
        ]);
        assert_eq!(s[0].0, 2.0);
        assert_eq!(s[1].0, 10.0, "真实增长要反映出来");
    }

    #[test]
    fn context_excludes_output_tokens() {
        let s = ctx_series(&[
            r#"{"type":"result","is_error":false,"usage":{"input_tokens":1000,"output_tokens":50000,"cache_read_input_tokens":0},"modelUsage":{"m1":{"contextWindow":100000}}}"#,
        ]);
        assert_eq!(s[0].0, 1.0, "输出不该计入，实际 {}", s[0].0);
    }

    #[test]
    fn context_percent_is_clamped() {
        let s = ctx_series(&[
            r#"{"type":"result","is_error":false,"usage":{"input_tokens":900000,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"modelUsage":{"m1":{"contextWindow":100000}}}"#,
        ]);
        assert_eq!(s[0].0, 100.0, "超过 100% 必须夹住，实际 {}", s[0].0);
    }

    #[test]
    fn context_skips_when_window_unknown() {
        // 没有 contextWindow 就不发事件，避免除零
        let s = ctx_series(&[
            r#"{"type":"result","is_error":false,"usage":{"input_tokens":1000,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}"#,
        ]);
        assert!(s.is_empty(), "缺 contextWindow 时不该发 Context 事件");
    }
}
