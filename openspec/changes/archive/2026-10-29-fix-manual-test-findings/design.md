# Design

## Context

动机见 `proposal.md - Why`，需求见 `specs/`。本节只记录决定实现路径的现状事实。

**已实测确认的事实**

| 事实 | 来源 |
|---|---|
| Windows 下 `child.kill()` 只杀 `cmd.exe`，`claude`（node）存活 | 用 node 模拟子进程实测，杀 cmd 后 node 仍存活 |
| `claude` 的 `system/init` 事件含 `slash_commands`(143)、`skills`(65)、`agents`(20)、`plugins`(21) | 真实跑一条 `claude --print`，样本见 `temp/init_sample.ndjson` |
| CLI 无单独禁用某个 Skill 的参数，只有 `--disable-slash-commands`（全禁） | `claude --help` |
| `file.rs::within()` 不认正斜杠分隔符，`C:/a/b.md` 被判为工作区外 | 单测脚本实测 7 个用例 |
| `extractFilePaths` 把「Word/Excel/PPT/PDF」识别为路径 | 从数据库读真实消息复现 |
| `context_compact` 同步执行 `claude::run`，`chat_send` 则起线程 | 源码对照 |

**结构约束**

- 前端不直接调 IPC，统一经 `src/api/*` 收口
- 后端命令返回传输对象，不返回数据库实体
- 配置文件写入前备份，解析失败时拒绝覆盖
- 数据库 schema 本次不变

## Goals / Non-Goals

**Goals:**

- 停止在任何阶段都能在 3 秒内生效，且不留残余进程
- 10 个会话的生成状态彼此完全独立
- 14 条测试问题逐条可复测通过

**Non-Goals:**

- 不重写对话事件协议（仍走 NDJSON 逐行解析，只是推送节奏变化）
- 不做跨重启的状态持久化（未查看标记、草稿都只在内存）
- 不引入 shell / opener / notification 插件
- 不改变数据库结构

## Decisions

### 决策 1：进程终止用进程树，不用单进程 kill

**问题**：`cmd.exe /d /s /c claude` 是一条三代进程链（cmd → node → 可能的工具子进程），
杀最外层等于没杀。

**方案**：Windows 下改用 `taskkill /PID <pid> /T /F`。
`/T` 终止整棵子树，`/F` 强制。

**替代方案**：
- Job Object（`CreateJobObject` + `AssignProcessToJobObject`）——更彻底，但需要
  `windows` crate 或手写 FFI，引入平台依赖。当 pid 已知时 `taskkill /T` 足够。
- 逐个枚举子进程再杀——需要在杀掉父进程后仍能查到子进程，时序不可靠。

**非 Windows**：`setsid` 启动 + `kill(-pgid)`。本次只在 Windows 验证，
其余平台分支写好但标注未验证。

### 决策 2：stdout 读取移入独立线程 + 停止标志轮询

**问题**：`for line in reader.lines()` 是阻塞的。模型思考期间不产出任何行，
循环卡在 `.next()` 上，`should_stop` 永远读不到。

**方案**：读取线程负责读行并送入 `mpsc::channel`；主循环用 `recv_timeout(100ms)`：

```
读线程 ──lines──> channel ──recv_timeout(100ms)──> 主循环
                                     │
                                     ├─ 超时 → 检查 stop 标志、检查空闲超时
                                     └─ 收到 → 解析并派发事件
```

同一处顺带实现空闲超时（连续 30 秒无有效事件 → 终止），
补上 `docs/claude-cli-protocol.md` 已承诺但代码未实现的行为。

**替代方案**：给 stdout 设读超时。Windows 管道不支持读超时，需要改非阻塞 IO + 轮询，
比通道更复杂且更容易出错。

### 决策 3：事件攒批推送

**问题**：`stream_event` 每个 text_delta 都 `app.emit` 一次。一次长回复数千条事件，
10 个并发会话就是每秒数万条，前端响应式更新与 markdown 调度被打满。

**方案**：后端维护 per-run 文本缓冲，按 60-100ms 定时 flush 一次。
前端收到的是批量增量，`streamText` 写入次数降到每秒 10-16 次。

**替代方案**：前端节流。已有 120ms 渲染节流，但**事件本身的 IPC 开销仍在**，
且 10 并发下前端要处理 10 条流，仅靠节流不够。

### 决策 4：并发状态用 Map，前后端一致

**后端**：

```rust
struct AppState {
    run_seq: AtomicU64,
    active: Mutex<HashMap<String, ActiveRun>>,   // session_id → run
}
```

- 插入前检查 `len() >= 10`，超限返回可读错误
- 每轮结束（成功/失败/中断）都从 Map 移除自己

**前端**：

```ts
const runs = ref<Map<string, { runId: number; text: string; tools: ToolCall[]; stopping: boolean }>>()
```

`chat_interrupt` 必须带目标标识，否则停一个会停全部。**这是与决策 1/2 的强耦合点：
三者必须同批实现**，先做任何一个都会返工或产生「能并发但停不下来」的中间状态。

### 决策 5：斜杠命令清单来自 CLI，不扫目录

**已实测**：`system/init` 的 `slash_commands` 143 条 = 用户 Skill(6) + 用户命令(26) +
项目命令(6，以 `opsx:apply` 形式) + 插件命令 + CLI 内置命令，**全部包含**。

**方案**：后端在 `init` 分支解析并保存 `slash_commands`，
新增命令供前端拉取；描述从本地文件补（Skill 的 frontmatter、命令文件首段），
补不到的内置命令只显示名字。

**为什么不全靠扫目录**：扫目录拿不到 CLI 内置命令（`clear`、`compact`、`model`…），
也拿不到插件的命名空间形式。**扫目录是子集，CLI 是超集。**

**代价**：`init` 只在发消息后到达，首屏为空。接受这个限制，面板给出说明文案。

### 决策 6：附件只在内存，不进数据库

附件随 `chat_send` 一次性传给后端，拼进同一条 NDJSON 的 content 数组，
发送后前端即丢弃。数据库 `messages` 表不加字段。

**理由**：用户明确要求已发送消息不显示附件。省掉表结构变更与存储膨胀。

**风险**：无法回放当时发的内容；`claude` 是否接受 image 块尚未验证（见风险表）。

### 决策 7：暗色模式用令牌收口，不逐处打补丁

现状是「补一个变量，漏一片散点」——16 处写死浅色绕过令牌体系。
新增令牌：`--btn-bg`、`--info-bg`/`--info-line`/`--info-t`、`--dot-off`、`--track`。
组件里所有写死值改引用令牌，之后新增控件只要用令牌就自动适配。

### 决策 8：浮层用坐标锚定

`getBoundingClientRect()` 取触发按钮位置，`position: fixed` 定位；
窗口 resize 与滚动时重算；下方空间不足时向上翻转；右侧超出时夹回窗口内。

**替代方案**：CSS 锚点定位（`anchor-name`/`position-anchor`）——只改 CSS 更省事，
但依赖 Chromium 125+，WebView2 版本不确定，风险高于收益。

### 决策 9：工作区外文件灰显，不开读通道

`within()` 修复斜杠归一后，仍只接受工作区内路径。
前端新增 `file_check(paths) -> Vec<bool>` 批量判定，`MessageItem` 据此渲染可点或灰态。

**为什么判定放后端**：前端有 `ws.list`，但工作区目录可能被删除或移动，
只有后端 `canonicalize` 后的判断才准。

**为什么批量**：`MessageItem` 每条消息一个实例，50 条历史消息若各自调用就是 50 次 IPC。
由上层一次算完传给子组件。

**替代方案**：点击时弹确认后放行——体验更好，但等于开一条能读任意路径的通道，
与本项目「预览不得越界」的既有安全设计冲突。

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| `claude` CLI 是否接受 `image` 内容块未验证，若不接受则附件功能需推翻重做 | **实现第一步先跑一条带 image 块的命令验证**，验证不通过则退回「把文件路径写进文本」方案 |
| 10 并发 = 最多 10 个 `claude` 子进程，内存与 API 并发额度都会显著上升 | 上限值由用户指定；实现后实测单进程内存与总占用，超标再议 |
| `taskkill /T` 依赖 pid 有效，若父进程已退出而子进程被收养，可能杀不到 | 停止前先取 pid 快照；超时兜底（前端 15 秒强制复位）保证用户始终有出路 |
| 攒批推送会让流式输出呈现为「一小段一小段」而非逐字 | flush 间隔取 60-100ms，人眼感知接近逐字；若观感变差再调小 |
| `slash_commands` 仅在首轮之后可用 | 面板给出明确说明，不是静默失效 |
| 附件不设大小上限，超限失败不可避免 | 卡片显示实际大小，超阈值标黄，失败文案明确指向附件 |
| 暗色适配靠人工逐页目检，可能仍有遗漏 | 修复完成后在暗色下走完全部 8 个设置分区与 5 个弹窗，逐页确认 |
| 删除 Skill 会连带删除目录内附属文件 | 确认弹窗明确列出会一并删除的文件 |

## Migration Plan

1. **先验证未知项**：附件 image 块、`taskkill /T` 在真实 claude 进程上的效果。
   两项都通过再动其余代码。
2. **后端并发与停止**（决策 1/2/3/4）同批提交，作为一次原子变更；此期间不前移任何前端状态。
3. **前端状态改造**（Map 化、按钮判断、侧栏标记）随后进行。
4. **独立的小改动**（暗色令牌、浮层定位、设置页导航、权限表格、规则分区、文件正则）各自可独立提交与回滚。
5. **复测**：14 条逐条验证，含任务管理器检查残留进程。

**回滚**：每个阶段独立提交。并发改造若引发不可控问题，
可回退到单槽位实现，代价是失去多会话并行，但不影响其他修复。

## Open Questions

1. 附件 image 块的具体格式与大小上限，待第一步验证后确定。
2. `slash_commands` 中带参数的命令（如 `/model`）是否需要参数提示；
   CLI 未提供该信息，可能只能硬编码少数几个或干脆不提示。
3. 10 个并发的实际内存占用未估算，实现后实测再决定是否调整上限。
