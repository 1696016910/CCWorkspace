//! 会话进程池。
//!
//! 常驻 LRU 最多 3 个会话进程（单进程实测 534MB），超出的会话起临时进程、
//! 用完即杀。这样常用的会话几乎不付启动开销，同时不会让 10 并发把内存撑爆。
//!
//! 空闲 5 分钟回收（用户指定）。

use crate::session::SessionProcess;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 常驻进程上限。单进程约 534MB，3 个约 1.6GB。
const RESIDENT_LIMIT: usize = 3;
/// 空闲多久回收。
const IDLE_TTL: Duration = Duration::from_secs(5 * 60);

/// 进程池。持有一个 session_id → 常驻进程的映射。
#[derive(Default)]
pub struct Pool {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    resident: HashMap<String, Resident>,
}

struct Resident {
    /// 包一层 Option，方便取出运行一轮再放回
    proc: Option<SessionProcess>,
}

impl Pool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// 取出某会话的常驻进程（不删除，只是借用）。
    ///
    /// 拿不到就返回 None，调用方自行决定起临时进程还是新建常驻。
    pub fn take_resident(&self, session_id: &str) -> Option<SessionProcess> {
        let mut g = self.inner.lock().ok()?;
        g.resident.get_mut(session_id)?.proc.take()
    }

    /// 把进程放回常驻槽位。
    ///
    /// 超过上限时淘汰最久未用的那个（Drop 会 kill 进程）。
    pub fn put_resident(&self, session_id: String, proc: SessionProcess) {
        let Ok(mut g) = self.inner.lock() else { return };

        // 已在池里就直接替换（期间可能被淘汰过）
        if let Some(slot) = g.resident.get_mut(&session_id) {
            slot.proc = Some(proc);
            return;
        }

        if g.resident.len() >= RESIDENT_LIMIT {
            // 找出 last_used 最早的淘汰掉
            let victim = g
                .resident
                .iter()
                .filter_map(|(k, v)| v.proc.as_ref().map(|p| (k.clone(), p.last_used)))
                .min_by_key(|(_, t)| *t)
                .map(|(k, _)| k);
            if let Some(k) = victim {
                if let Some(mut old) = g.resident.remove(&k) {
                    // old.proc 的 Drop 会 kill 进程
                    drop(old.proc.take());
                }
            }
        }

        g.resident.insert(
            session_id,
            Resident {
                proc: Some(proc),
            },
        );
    }

    /// 该会话是否有常驻进程（在槽位里）。
    #[cfg(test)]
    pub fn has_resident(&self, session_id: &str) -> bool {
        self.inner
            .lock()
            .map(|g| g.resident.get(session_id).is_some_and(|r| r.proc.is_some()))
            .unwrap_or(false)
    }

    /// 主动丢弃某会话的进程（中断、僵死、会话删除时调用）。
    pub fn evict(&self, session_id: &str) {
        let Ok(mut g) = self.inner.lock() else { return };
        if let Some(mut old) = g.resident.remove(session_id) {
            drop(old.proc.take());
        }
    }

    /// 扫描并回收空闲超时的进程。由后台线程周期调用。
    pub fn sweep_idle(&self) -> usize {
        let Ok(mut g) = self.inner.lock() else { return 0 };
        let now = Instant::now();
        let stale: Vec<String> = g
            .resident
            .iter()
            .filter_map(|(k, v)| {
                v.proc
                    .as_ref()
                    .filter(|p| now.duration_since(p.last_used) >= IDLE_TTL)
                    .map(|_| k.clone())
            })
            .collect();
        let n = stale.len();
        for k in stale {
            if let Some(mut old) = g.resident.remove(&k) {
                drop(old.proc.take());
            }
        }
        n
    }

    /// 当前常驻进程数，供界面或诊断使用。
    pub fn resident_count(&self) -> usize {
        self.inner
            .lock()
            .map(|g| g.resident.values().filter(|r| r.proc.is_some()).count())
            .unwrap_or(0)
    }

    /// 全部回收（应用退出时）。
    pub fn shutdown(&self) {
        let Ok(mut g) = self.inner.lock() else { return };
        g.resident.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake() -> SessionProcess {
        // 用 cat 之类的长驻进程代替 claude：测试只关心池的淘汰逻辑
        #[cfg(target_os = "windows")]
        let mut child = std::process::Command::new("cmd.exe")
            .args(["/d", "/s", "/c", "more"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn fake");
        #[cfg(not(target_os = "windows"))]
        let mut child = std::process::Command::new("cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn fake");

        let stdin = child.stdin.take().unwrap();
        SessionProcess::from_child(child, stdin)
    }

    #[test]
    fn caps_resident_at_limit() {
        let pool = Pool::default();
        for i in 0..RESIDENT_LIMIT + 2 {
            pool.put_resident(format!("s{i}"), fake());
        }
        assert_eq!(
            pool.resident_count(),
            RESIDENT_LIMIT,
            "常驻数必须停在 {RESIDENT_LIMIT}"
        );
    }

    #[test]
    fn recycle_marks_oldest() {
        let pool = Pool::default();
        // 手工造一个"很久没用"的
        let mut old = fake();
        old.last_used = Instant::now() - Duration::from_secs(600);
        pool.put_resident("old".into(), old);
        pool.put_resident("fresh".into(), fake());

        let n = pool.sweep_idle();
        assert_eq!(n, 1, "只该回收空闲超时的那一个");
        assert!(!pool.has_resident("old"));
        assert!(pool.has_resident("fresh"));
    }

    #[test]
    fn take_and_put_roundtrip() {
        let pool = Pool::default();
        pool.put_resident("s1".into(), fake());
        assert!(pool.has_resident("s1"));

        let p = pool.take_resident("s1").expect("应能取出");
        assert!(!pool.has_resident("s1"), "取出后槽位里没有进程");
        pool.put_resident("s1".into(), p);
        assert!(pool.has_resident("s1"));
    }

    #[test]
    fn same_session_reuses_one_process() {
        // 断言池的语义：同一 session_id put 两次拿到的是同一个槽位，
        // 不会累积成两个进程。这是「永远只付一次启动开销」的前提。
        let pool = Pool::default();
        pool.put_resident("s1".into(), fake());
        let first = pool.take_resident("s1").expect("第一次取出");
        let pid = first.child.id();
        pool.put_resident("s1".into(), first);

        let second = pool.take_resident("s1").expect("第二次取出");
        assert_eq!(second.child.id(), pid, "应是同一个进程，不是新建的");
        assert_eq!(pool.resident_count(), 0, "取出后槽位为空");
    }

    #[test]
    fn different_sessions_get_different_slots() {
        let pool = Pool::default();
        pool.put_resident("a".into(), fake());
        pool.put_resident("b".into(), fake());
        assert_eq!(pool.resident_count(), 2);
        assert!(pool.has_resident("a"));
        assert!(pool.has_resident("b"));
    }
}
