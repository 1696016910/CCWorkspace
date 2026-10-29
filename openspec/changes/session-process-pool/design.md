# Design

## Context

动机见 `proposal.md - Why`。这里只记决定实现路径的实测事实。

| 事实 | 数据 |
|---|---|
| 一轮对话 8.54s 中启动占 4.39s | `duration_ms=4147` |
| 同一进程可处理多轮 stdin | 3 轮：17.6s / 6.9s / 5.6s |
| 读到 `result` 后进程不退出 | 10 秒后仍存活 |
| 常驻进程内上下文连续 | 记住先前暗号 |
| kill 后 `--resume` 能接回 | 也记住了暗号 |
| 单进程内存 | 534 MB |
| `/context` 4.32s 但 `duration_api_ms=0` | 全是启动开销 |
| `/compact` 31.31s = 27.66s 模型 + 3.9s 启动 | |
| 9 次 SessionStart hook 串行 3.2s | 4 个插件贡献 |

## Goals / Non-Goals

**Goals:**

- 第 2 轮起单轮从 8.5s 降到 4.2s 左右
- 内存可控（常驻上限 3 个进程）
- 中断、僵死、空闲都能自动恢复

**Non-Goals:**

- 不追求首轮变快（首次仍要启动，4.4s 省不掉）
- 不改 CLI 行为（hook、插件、模型参数一律不动）
- 不做进程池的跨启动持久化

## Decisions

### 决策 1：进程池而非每次重建

**方案**：每个会话一个 `SessionProcess`，`LRU` 容量 3。

```rust
struct SessionProcess {
    child: Child,
    stdin: ChildStdin,
    /// 这一轮读到 result 的标记，用于判断轮次边界
    last_used: Instant,
    /// CLI 自己的会话 id，重建时用 --resume
    claude_sid: String,
}
```

**为什么容量是 3**：单进程 534MB，3 个约 1.6GB。用户确认 3 个足够日常使用。

**超出怎么办**：起临时进程，`result` 后立刻 kill。这样 10 并发不会把内存撑爆，同时常用会话仍然快。

**替代方案**：全部常驻——10 并发要 5.2GB，否决。

### 决策 2：轮次边界靠 `result` 事件

一个常驻进程的 stdout 是连续流。要区分「这一轮结束」和「进程结束」，靠 `type == "result"`。

实测确认：读到 `result` 后进程仍存活，能继续接收 stdin。

**注意**：工具调用会产生额外的 `user` 事件（tool_result），但那不是轮次结束，只有 `result` 才是。

### 决策 3：中断 = kill，重建时 `--resume`

```
中断 → kill 进程树 → 标记 Dead
下次发消息 → 新建进程，带 --resume <claude_sid>
```

**已验证**：kill 后 `--resume` 能接回上下文（实测暗号仍记得）。

**替代方案**：尝试向 stdin 发中断信号——CLI 没有这个协议，否决。

**代价**：中断后下一条消息要多付 4.4s 启动。中断本身不频繁，可接受。

### 决策 4：僵死检测用「30 秒无事件」

```
spawn → 30s 内没有任何事件 → 判定僵死 → kill 重建
```

与现有 `IDLE_TIMEOUT` 一致。区别是现在它作用于「进程」而不是「一轮」。

### 决策 5：空闲回收 5 分钟

`sweeper` 线程每分钟扫一次，`last_used` 超过 5 分钟就 kill 并移出池。

用户明确要求 5 分钟。

### 决策 6：上下文探测改为按需

**删掉**：

- `done` 之后的 `probe_context`
- 压缩之后的 `probe_context`

**新增**：`context_probe(session_id)` 命令，用户打开弹窗时调。

弹窗加 loading 态：探测要 4.3s，必须有明确反馈。

**为什么接受这个取舍**：那 4.3s 是纯粹的进程启动开销（`duration_api_ms=0`），每轮付一次却只为一个用户未必在看的数字。

### 决策 7：flusher 改 channel

```rust
// 之前：sleep 轮询，空转 12 次/秒，首字延迟 0-80ms
while flushing { sleep(80ms); take(&mut buf); if empty continue; emit(chunk); }

// 改成：有数据立即推
while let Ok(chunk) = rx.recv() { emit(chunk); }
```

`rx` 由 `claude::run` 内部在拿到文本增量时 `send`。

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| 常驻进程僵死（stdin 无响应） | 30s 无事件判死，kill 重建 |
| 内存超预期 | 池容量 3 可配置；超出走临时进程 |
| stdin 写阻塞 | 写操作放在独立线程，5s 超时判死 |
| 进程池与 10 轮并发冲突 | 临时进程兜底，不占常驻槽位 |
| 中断后重建慢 4.4s | 已有明确提示，可接受 |
| 进程泄漏（应用退出未清理） | `Drop` 时遍历 kill 全部 |

## Migration Plan

1. **阶段 1**：删自动 probe + flusher 改 channel（低风险，可独立验证）
2. **阶段 2**：进程池核心（`pool.rs` + `claude.rs` 拆分 + `chat_send` 改造）
3. **阶段 3**：按需 probe + 弹窗 loading
4. **阶段 4**：压力验证

**回退**：进程池出问题就退回「每轮新建」，只需把 `chat_send` 里的取进程改成 `spawn`。

## Open Questions

1. 常驻进程的 `--resume` 参数在首次 spawn 时应带还是不带？——首次不带（新会话），重建时带。
2. 10 轮并发下临时进程的内存峰值是多少？——需实测，超过阈值再调池容量。
