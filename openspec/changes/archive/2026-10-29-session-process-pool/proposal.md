# Proposal

## Why

对话慢的主因不是模型，是**每次发消息都重新启动一个 CLI 进程**。实测：

```
一轮对话 8.54s
├── 4.39s  进程启动（51%）—— 4 个插件 hook 串行跑 3.2s
└── 4.15s  模型回答（49%）
```

轮数越多，浪费越大：10 轮对话里有 44 秒纯粹花在反复启动上。

此外每轮结束还会额外跑一次 `/context` 校准，又是 4.3 秒——只为刷新一个用户未必在看的百分比。

## What Changes

- **会话进程池**：每个会话保持一个常驻 CLI 进程，LRU 最多 3 个；超出的会话起临时进程，用完即杀
- **中断语义**：中断即杀进程，下次发消息用 `--resume` 接回上下文（已验证接得上）
- **上下文探测改为按需**：删掉每轮与压缩后的自动 `/context`，改为用户打开弹窗时查询，弹窗加 loading 态
- **流式推送改事件驱动**：去掉 80ms 轮询，改为有数据立即推送

**不做的事情**

- 不改 CLI 的 hook 配置（保留 PUA / superpowers 等 4 个插件的 SessionStart hook，接受每进程 3.2s 启动成本）
- 不做多窗口支持（单窗口场景，同一会话不会并发打开）
- 不改模型、prompt 或压缩策略
- 不在本次引入持久化进程池（应用退出即全部回收）

## Capabilities

### New Capabilities

无新增能力——本次只改实现方式，对外行为不变（除了上下文百分比从"自动刷新"变成"按需查询"，这一条写进 conversation 的修改里）。

### Modified Capabilities

- `conversation`: 上下文用量从每轮自动刷新改为按需查询；中断后靠 `--resume` 恢复上下文

## Impact

**代码**

| 位置 | 影响 |
|---|---|
| `src-tauri/src/claude.rs` | 从「一次 run」拆成「spawn / 写一轮 / 读一轮 / kill」四个可组合操作 |
| `src-tauri/src/pool.rs` | 新增：会话进程池、LRU、空闲回收、僵死检测 |
| `src-tauri/src/main.rs` | `chat_send` 改为从池里取进程；删两处 `probe_context`；新增按需探测命令 |
| `src/components/ChatPane.vue` | 打开上下文弹窗时触发探测 |
| `src/components/dialogs/ContextPopover.vue` | 加 loading 态 |

**接口变更**

- 新增 `context_probe(session_id)` 命令
- `ChatEvent` 增加 `state` 字段（可选）

**数据**

- 数据库 schema 不变

**验证要求**

- 10 并发不崩、内存不失控
- 中断后下一条消息上下文接得上
- 进程僵死能自动恢复
