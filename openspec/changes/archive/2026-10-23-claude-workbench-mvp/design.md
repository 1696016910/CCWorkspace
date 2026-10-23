# Design

## Context

项目此前没有任何应用代码，这是从零开始的一个 Electron 桌面应用。设计需要贴合两个既有事实：

1. **Claude Code 已经在磁盘上维护了完整状态**。会话在 `~/.claude/projects/<编码路径>/<sessionId>.jsonl`，配置在 `~/.claude/settings.json`，Skill 与 Agent 是 Markdown 文件。这些数据由 Claude 拥有，应用是读者。
2. **Agent 引擎是一个原生二进制**。`claude` 不是 JS 包，而是一个按平台分发的可执行文件；Agent SDK 的职责是 spawn 它并解析 stdio 上的 JSON 行协议。

需求见 `specs/`，动机见 `proposal.md`。本文件只记录"怎么实现"以及为什么这样选。

约束：

- 跨平台（Windows 为主，macOS / Linux 需可用）
- 单人使用，无账号、无服务端
- 应用自身的数据量极小（工作区与别名），不需要数据库
- 必须容忍 Claude 私有格式的版本漂移

## Goals / Non-Goals

**Goals:**

- 用一层薄服务把"磁盘上的 Claude 状态"翻译成界面可用的模型，界面不接触文件系统
- 对话能力以 Agent SDK 为唯一入口，不自己实现 stdio 协议
- 任何对 `settings.json` 的写入都可回滚，且不会与其它进程的写入互相覆盖
- 新增能力时不需要改动已有的三块（工作区 / 会话 / 配置）之间的边界

**Non-Goals:**

- 不做编辑器、LSP、终端、Git 面板、文件浏览器
- 不做多进程会话编排（一次只跑当前会话）
- 不做离线可用（对话必然依赖网络）
- 不做插件市场的浏览与搜索，只做已添加仓库内的安装
- 不追求安装包体积或用 Rust/Go 重写协议层

## Decisions

### 决策 1：Electron + Node，而不是 Tauri 或 Wails

**选择**：Electron 45，主进程用 Node 直接 `import` Agent SDK。

**理由**：三种方案都要 spawn 同一个 `claude` 二进制，差别只在由谁解析 stdio 协议。Agent SDK 已经封装了协议（含未文档化的 `control_request` / `mcp_response` 包装层），并且与 CLI 同版本发布。自己实现等于长期维护一个私有协议的逆向适配器。

**放弃的方案**：

- Tauri + Rust：安装包 10MB、内存 60MB，但协议要自己写，每次 CLI 升级都要跟。体积收益对个人工具无感，维护成本是持续的。
- Wails + Go：同 Tauri，体积略大，生态更弱。
- 直接嵌终端跑 `claude` TUI：省掉协议，但拿不到结构化事件，工具调用无法折叠、用量无法展示，且 Windows 下的 PTY 需要原生模块。

**代价**：安装包约 130MB，常驻内存约 300MB。对单人本机工具可接受。

### 决策 2：会话由应用自己持有，使用 SQLite 存储

**选择**：应用在 `userData/workbench.db` 维护自己的 `workspaces` / `sessions` / `messages` 三张表。会话与消息完全由应用产生和读取，不读 `~/.claude/projects/`。

**理由**：这个应用的目的是把"日常事务性工作"与"开发用的 Claude Code"分开。若共用同一份会话数据，两边会互相污染——应用里冒出一堆开发会话，开发时又会被日常任务干扰。数据归属清楚了，隔离才是真的。

**放弃的方案**：

- 读 Claude 的 jsonl：无法实现隔离，且 jsonl 是私有格式，随版本漂移。
- 用 JSON 文件存全部消息：消息量会随对话增长到几 MB，每次读写整文件不划算，也无法按会话增量查询。

**代价**：需要自己设计表结构与迁移。但存储规模小、schema 简单，成本可控。

### 决策 3：用 `node:sqlite`，不引入原生依赖

**选择**：使用 Electron 内置 Node 提供的 `node:sqlite`（`DatabaseSync`）。

**理由**：已在本机 Electron 45 运行时实测可用。它随 Node 内置，不需要 `better-sqlite3` 这类需要针对 Electron ABI 重编译的原生模块——打包与跨平台都不会因为编译失败而卡住。

**放弃的方案**：

- `better-sqlite3`：功能更全，但要 `electron-rebuild`，Windows 还需要 VS Build Tools，是打包阶段最常见的翻车点。
- `sql.js`：纯 WASM，无需编译，但每次都要手动持久化整库，失去事务与并发优势。

**代价**：`node:sqlite` 的 API 比 better-sqlite3 朴素（同步 API、无链式查询），复杂查询要自己拼 SQL。对本项目的规模足够。

**风险**：`node:sqlite` 在旧版 Electron 上不可用。已在 `electron` 版本上锁 45，并在打包验证任务中检查。

### 决策 5：配置写入采用备份 + 原子写 + mtime 比对

**选择**：写入前把原文件复制到 `~/.claude/backups/settings-<时间戳>.json`；新内容写入同目录临时文件后 `rename` 覆盖；写前比对文件 mtime 与打开时记录的是否一致。

**理由**：`settings.json` 写坏会让 Claude 无法启动，而 Claude CLI 可能同时在写这个文件。三个措施分别解决"可回滚"、"不产生半截文件"、"不覆盖他人修改"。

**放弃的方案**：

- 直接 `writeFile`：崩溃或断电会留下截断的 JSON。
- 文件锁：Windows 与 Unix 的锁语义不同，且 Claude 不会遵守我们的锁。

**代价**：mtime 精度在某些文件系统上只到秒级，极端情况下漏检。可接受——备份仍在。

### 决策 6：对话走 Agent SDK 的流式模式，一次一个会话进程

**选择**：`query({ prompt, options: { cwd, resume, env, permissionMode, skills } })`，监听事件流。

**理由**：这是唯一能同时拿到流式文本、工具调用事件与用量统计的入口。`claude -p` 每轮冷启动延迟明显且拿不到完整事件。

**放弃的方案**：

- 每轮 `claude -p` 独立调用：无法续接上下文，除非每轮读整个 jsonl 再拼成 prompt，浪费 token。
- 长驻进程 + 自写协议：即决策 1 的 Tauri 路线。

**代价**：每个活跃会话一个子进程。首版只允许一个活跃会话，内存可控。

### 决策 7：主进程承担全部 IO，渲染进程只能通过 IPC 调用

**选择**：`contextIsolation: true`，预加载脚本用 `contextBridge` 暴露白名单通道；渲染进程不引入 `fs`、`child_process`。

**理由**：渲染进程要渲染来自仓库的 Markdown 与模型输出，属于不可信内容来源。隔离后即使渲染层被注入，也拿不到任意文件读写能力。

**放弃的方案**：`nodeIntegration: true` 图省事——把一个本地工具变成任意代码执行的入口，代价不对等。

### 决策 8：不引入 UI 组件库

**选择**：手写 CSS，样式基于原型文件。

**理由**：原型已经是完整样式表（浅色、描边分层、单一绿色强调）。引入 Element Plus 之后需要覆盖其全部默认变量才能达到同样效果，净收益为负。

**代价**：开关、下拉、表格等多状态组件要自己实现。数量有限（约 6 个），可接受。

### 决策 9：Skill 与插件开关走不同的持久化路径

**选择**：

- Skill 开关存应用自己的数据库，通过 SDK 的 `skills` 选项传给会话。
- 插件开关写 `settings.json` 的 `enabledPlugins`，由 CLI 自身读取。

**理由**：插件是 CLI 的一等公民，有自己的启用机制，绕开它会与 CLI 行为不一致。Skill 没有等价的 CLI 开关，SDK 的 `skills` 选项是官方支持的控制点。

**放弃的方案**：两者都写 `settings.json`——Skill 没有对应字段，写进去无效。

### 决策 10：数据库表结构与迁移

**选择**：三张表 + 一张元信息表，版本号存 `meta.schema_version`。启动时若版本低于当前，按顺序执行迁移脚本。

```sql
workspaces(id TEXT PK, path TEXT UNIQUE, name TEXT, created_at TEXT)
sessions(id TEXT PK, workspace_id TEXT, title TEXT, auto_title INTEGER,
         created_at TEXT, updated_at TEXT, deleted_at TEXT)
messages(id TEXT PK, session_id TEXT, role TEXT, content TEXT,
         tools TEXT, created_at TEXT)
meta(key TEXT PK, value TEXT)
```

**理由**：会话标题需要区分"用户命名"与"自动生成"——`auto_title = 1` 时首条消息到达才写标题，之后不再覆盖。

**放弃的方案**：把消息塞进 sessions 的一列 JSON——查询、分页、单条更新都会变复杂。

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| `node:sqlite` 在旧版 Electron 不可用 | 锁定 Electron 45；打包验证任务中显式检查 |
| 数据库文件损坏 | 启动时 `PRAGMA integrity_check`，失败则以空库启动并备份损坏文件 |
| Agent SDK 版本与本地 CLI 版本漂移 | 锁定 SDK 版本；升版本前跑一次端到端验证 |
| 第三方中转不兼容工具事件流 | 首屏提供连接体检；错误显式展示而非吞掉 |
| 与 CLI 并发写 `settings.json` | 备份 + 原子写 + mtime 比对 |
| 上下文成本失控（一次空对话可达 3.7 万 token） | 输入框常驻用量显示；设置页展示启用中的 Skill/插件数量 |
| 打包后找不到 `claude` 可执行文件 | 用 SDK 随包分发的平台二进制，不依赖用户 PATH |
| 范围回弹（又想加编辑器） | `proposal.md` 的 Capabilities 与"不做"清单为边界，扩范围先改 proposal |
| Electron 内存占用高于原生应用 | 单窗口、单活跃会话 |

## Migration Plan

全新项目，无历史数据迁移。首次启动时：

1. 创建 `userData/workbench.db`，建表并写入 `schema_version`
2. 工作区与会话均为空，界面进入引导状态
3. 不读取任何外部数据，不写 `~/.claude` 下的任何文件

回滚策略：删除 `workbench.db` 即恢复初始状态。`settings.json` 的修改有 `backups/` 可还原。

## Open Questions

- 会话列表的刷新时机：文件监听（`fs.watch`）还是定时轮询？首版可用"窗口获得焦点时刷新"，具体实现留到 M1 定。
- 回收目录的保留期限与清理策略，留到 M3（会话删除）时确定。
- 是否需要在会话内展示每条消息的花费，还是只显示累计值。首版只显示累计。
