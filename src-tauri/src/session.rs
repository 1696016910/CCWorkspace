//! 常驻的 claude 会话进程。
//!
//! 为什么不是每轮起一个进程：实测一轮对话 8.54s 里 4.39s 花在进程启动上
//! （4 个插件 hook 串行 3.2s + 初始化）。同一个进程可以连续处理多轮 stdin
//! ——实测 3 轮分别 17.6s / 6.9s / 5.6s，第 2 轮起不再有启动开销。
//!
//! 边界：读到 `result` 事件即本轮结束；进程在 result 之后仍然存活。

use crate::claude::{self, Event, Options, Outcome};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 一轮里多久没有有效事件就判定进程僵死。
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// 轮询间隔：约等于停止响应延迟。
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// 一个常驻会话进程。
///
/// 读线程随进程一起创建、一起销毁——**不能每轮新建**。原因是常驻进程
/// 这一轮结束后不会关闭 stdout，读线程会一直阻塞在 `read_line` 上；
/// 如果每轮 `join()` 它，就会永久卡住，表现为「回答完了但界面还在转」。
pub struct SessionProcess {
    pub child: Child,
    pub stdin: ChildStdin,
    /// 读线程推来的原始行。线程随进程存活，跨轮复用。
    rx: Receiver<String>,
    /// CLI 自己报的会话 id，进程重建时用它 `--resume` 接回上下文。
    pub claude_sid: String,
    pub last_used: Instant,
    /// 读线程是否还活着（stdout 关闭后会退出）
    reader_alive: Arc<AtomicBool>,
}

impl Drop for SessionProcess {
    fn drop(&mut self) {
        // 进程不关就是僵尸：应用退出、LRU 淘汰、空闲回收都靠这里收尾
        claude::kill_tree(&mut self.child);
        let _ = self.child.wait();
    }
}

impl SessionProcess {
    /// 启动一个新进程。`resume` 非空时接回既有会话。
    pub fn spawn(opts: &Options) -> Result<Self, String> {
        let args = claude::build_args(opts);
        let mut cmd = claude::build_command(&args);
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
        let stdin = child.stdin.take().ok_or("无法获取 stdin")?;
        let stdout = child.stdout.take().ok_or("无法获取 stdout")?;

        // 读线程终身只有一个：把 stdout 的所有行源源不断送进 channel
        let (tx, rx) = mpsc::channel::<String>();
        let reader_alive = Arc::new(AtomicBool::new(true));
        let alive2 = reader_alive.clone();
        std::thread::spawn(move || {
            let mut rd = BufReader::new(stdout);
            loop {
                let mut buf = String::new();
                match rd.read_line(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(buf.trim_end().to_string()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            alive2.store(false, Ordering::Relaxed);
        });

        Ok(Self {
            child,
            stdin,
            rx,
            claude_sid: String::new(),
            last_used: Instant::now(),
            reader_alive,
        })
    }

    /// 测试专用：用一个外部进程构造，跳过 CLI 参数拼装。
    #[cfg(test)]
    pub fn from_child(mut child: Child, stdin: ChildStdin) -> Self {
        let stdout = child.stdout.take().expect("测试进程要有 stdout");
        let (tx, rx) = mpsc::channel::<String>();
        let reader_alive = Arc::new(AtomicBool::new(true));
        let alive2 = reader_alive.clone();
        std::thread::spawn(move || {
            let mut rd = BufReader::new(stdout);
            loop {
                let mut buf = String::new();
                match rd.read_line(&mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(buf.trim_end().to_string()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            alive2.store(false, Ordering::Relaxed);
        });
        Self {
            child,
            stdin,
            rx,
            claude_sid: String::new(),
            last_used: Instant::now(),
            reader_alive,
        }
    }

    /// 进程是否还能继续用。
    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None)) && self.reader_alive.load(Ordering::Relaxed)
    }

    /// 跑一轮。读到 `result` 即返回；进程保留供下一轮复用。
    pub fn run_turn(
        &mut self,
        text: &str,
        images: &[claude::ImageAttachment],
        should_stop: Arc<AtomicBool>,
        mut on_event: impl FnMut(Event),
    ) -> Result<Outcome, String> {
        self.last_used = Instant::now();

        // 写一行 NDJSON。不关 stdin：常驻进程要继续收下一轮。
        let mut content = vec![json!({ "type": "text", "text": text })];
        for img in images {
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
        self.stdin.write_all(line.as_bytes()).map_err(|e| {
            if images.is_empty() {
                format!("写入 stdin 失败: {e}")
            } else {
                format!("图片过大或格式不支持，Claude 未能接收（{e}）")
            }
        })?;
        self.stdin
            .flush()
            .map_err(|e| format!("写入 stdin 失败: {e}"))?;

        let mut st = claude::State::default();
        let mut outcome = Outcome::default();
        let mut last_valid = Instant::now();

        loop {
            match self.rx.recv_timeout(POLL_INTERVAL) {
                Ok(line) => {
                    let t = line.trim();
                    if t.is_empty() || !t.starts_with('{') {
                        continue;
                    }
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(t) else {
                        continue;
                    };
                    if claude::is_valid_event(&v) {
                        last_valid = Instant::now();
                    }
                    if let Some(sid) = capture_sid(&v) {
                        self.claude_sid = sid;
                    }
                    st.handle(&v, &mut on_event);
                    // 读到 result 就是本轮终点，进程继续留着
                    if v.get("type").and_then(|x| x.as_str()) == Some("result") {
                        break;
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    if should_stop.load(Ordering::Relaxed) {
                        outcome.interrupted = true;
                        break;
                    }
                    if last_valid.elapsed() >= IDLE_TIMEOUT {
                        outcome.timed_out = true;
                        break;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    // 读线程退出 = stdout 关了 = 进程要没了
                    outcome.process_died = true;
                    break;
                }
            }
        }

        if outcome.timed_out {
            on_event(Event::Error(format!(
                "连接超时：{} 秒内没有收到任何响应。请检查网络与 ANTHROPIC_BASE_URL 配置",
                IDLE_TIMEOUT.as_secs()
            )));
            return Ok(outcome);
        }
        if !outcome.interrupted && !outcome.process_died {
            st.finish(&mut on_event);
        }
        Ok(outcome)
    }
}

/// 从 init 事件里取 CLI 的会话 id。
fn capture_sid(v: &serde_json::Value) -> Option<String> {
    let is_init = v.get("type").and_then(|x| x.as_str()) == Some("system")
        && v.get("subtype").and_then(|x| x.as_str()) == Some("init");
    if !is_init {
        return None;
    }
    v.get("session_id")
        .and_then(|x| x.as_str())
        .map(str::to_string)
}
