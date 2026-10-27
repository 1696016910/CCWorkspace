# Design

## Context

现有实现是 Electron + Node，主进程 3354 行 TypeScript，前端 8281 行 Vue。它通过 `@anthropic-ai/claude-agent-sdk` 启动 `claude` 子进程，走 stdio JSON 通信。

**关键事实**（已实测确认）：该 SDK 不是 API 客户端，它只做三件事——拼命令行参数、spawn 子进程、解析 stdout 的逐行 JSON。真正的模型调用由 `claude` 可执行文件完成。因此 Go/Rust 版不需要任何 SDK，只需复刻这层进程管理与协议解析。

实测同一条命令即可走通：

```
echo '{"type":"user","message":{"role":"user","content":[{"type":"text","text":"hi"}]}}' \
  | claude --print --input-format stream-json --output-format stream-json \
           --verbose --include-partial-messages --permission-mode acceptEdits
```

输出是逐行 JSON。实测发现一个重要干扰：**SessionStart hooks 会在流开头吐出大量 `hook_started` / `hook_response` 事件**，内容可能达到数万字符。三份实现都必须按 `type`/`subtype` 精确过滤，否则会被 hook 输出淹没。

## Goals / Non-Goals

**Goals:**

- 三份实现功能等价：工作区、会话、对话、配置、Skill、插件六大模块行为一致
- Go/Rust 版安装包 < 30MB（对比 Node 版 285MB）
- 共享同一份 SQLite 数据文件格式，可在三份实现间切换而不丢数据
- 把 claude CLI 协议写成文档，作为三份实现的共同依据
- 每份实现能独立构建、独立打包、独立发布

**Non-Goals:**

- 不做跨语言代码共享（三份独立，允许各自演进）
- 不内置 claude 二进制（依赖 PATH）
- 不追求三份 UI 像素级一致（原生控件各有风格，只保证信息结构与交互一致）
- 不做自动更新（Go/Rust 版首版不含）
- 不做 macOS / Linux 打包验证（本机只有 Windows，代码按跨平台写但只验证 Windows）

## Decisions

### 决策 1：目录结构

```
CCWorkspace/
├── node/           Node 实现（整体迁入）
│   ├── src/{main,preload,renderer,shared}
│   ├── tests/
│   ├── package.json
│   ├── electron.vite.config.ts
│   ├── electron-builder.yml
│   ├── tsconfig.*.json
│   └── vitest.config.ts
├── go/             Go 实现
│   ├── main.go
│   ├── internal/{db,services,ui}
│   ├── go.mod
│   └── build/
├── rust/           Rust 实现
│   ├── src/{main.rs,db,services,ui}
│   ├── Cargo.toml
│   └── build/
├── prototype/      三份设计稿 + 原型（文档）
├── openspec/       规格（文档）
├── scripts/        CI 辅助脚本（跨实现共用）
└── README.md
```

**为什么 `prototype/` 与 `openspec/` 留在根**：它们是文档与规格，不属于任何一份实现。三份实现对同一套规格。

**为什么 `scripts/` 留根**：`check-build-lock.mjs` 等是构建辅助，与具体实现无关。

**替代方案**：把 script 也放进 node/ —— 否决，因为 Go/Rust 构建同样需要清理文件占用。

### 决策 2：claude CLI 调用协议

三份实现统一用以下命令行与数据格式。

**启动参数**

```
claude --print
       --input-format stream-json
       --output-format stream-json
       --verbose
       --include-partial-messages
       --permission-mode <mode>
       [--resume <session-id>]
       [--model <model>]
```

工作目录设为会话所属工作区路径，环境变量合并 `process.env` 与 `settings.json` 的 `env` 字段。

**stdin 写入**（NDJSON，单行一个 JSON）

```json
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"用户输入"}]}}
```

**stdout 读取**（逐行解析，按 `type` 分派）

| type | subtype | 含义 | 处理 |
|---|---|---|---|
| `system` | `init` | 会话初始化 | 记录 `session_id`、`slash_commands` |
| `system` | `hook_started` / `hook_response` | hook 执行 | **忽略** |
| `stream_event` | — | 增量输出 | 提取 `content_block_delta.text_delta` |
| `assistant` | — | 完整消息块 | 工具调用、`context_usage` |
| `result` | — | 本轮结束 | 取 `usage`、`total_cost_usd`、`modelUsage` |

**必须过滤的两类**

1. `parent_tool_use_id` 非空的 `stream_event` / `assistant` —— 来自 Task 子代理，不能混进主回复
2. `type` 不在上表内的一切事件 —— hook 输出、日志等

**错误分类**

| 现象 | 判定 | 用户可见提示 |
|---|---|---|
| `result.is_error === true` | 模型侧失败 | 取 `errors[]` 或 `subtype` 翻译 |
| 30 秒无有效事件 | 网络/连接超时 | 提示检查网络与 BASE_URL |
| 迭代结束但未收到 `result` | CLI 异常退出 | 提示重试 |
| stdout 非 JSON 行 | 协议不符 | 忽略该行，继续读 |

### 决策 3：Go 版技术选型

| 层 | 选择 | 理由 |
|---|---|---|
| GUI | Fyne v2 | 纯 Go 无 CGO、自带主题系统与滚动容器、文档最全 |
| 数据库 | `mattn/go-sqlite3` | 唯一成熟的 Go SQLite 驱动 |
| JSON | 标准库 `encoding/json` | 无需第三方 |
| 进程 | 标准库 `os/exec` | — |
| 字体 | 系统字体（`C:\Windows\Fonts\msyh.ttc`） | 不打包字体，省 10MB |

**替代方案**：Gio（即时模式，布局代码量大，长列表要自己实现虚拟滚动）—— 否决，Fyne 的 `widget.List` 直接可用。

**CGO 问题**：`mattn/go-sqlite3` 需要 CGO，Windows 下需要 gcc。若无 gcc，退用 `modernc.org/sqlite`（纯 Go 实现，体积大 3MB 但零依赖）。

### 决策 4：Rust 版技术选型

| 层 | 选择 | 理由 |
|---|---|---|
| GUI | `eframe` / `egui` | 纯 Rust、跨平台、生态最成熟 |
| 数据库 | `rusqlite`（bundled feature） | 自带 SQLite 源码编译，无系统依赖 |
| JSON | `serde` + `serde_json` | 事实标准 |
| 进程 | 标准库 `std::process::Command` | — |
| 字体 | 系统字体（运行时加载 msyh.ttc） | 不打包 |

**替代方案**：iced（Elm 架构，组件要自己写）、Slint（DSL 学习曲线陡）—— 均否决，egui 上手成本最低且 `ScrollArea` 直接可用。

### 决策 5：UI 重写策略

设计稿（`prototype/` 三份）定义了**信息结构与交互**，不是像素规格。两份原生实现按此实现：

| 区域 | Fyne / egui 对应 |
|---|---|
| 侧栏树 | 自定义 `VBox` + 折叠状态，非 Tree 控件 |
| 对话流 | `ScrollContainer` + 动态子元素；用户/AI 消息用不同容器样式 |
| 输入框 | 多行 `Entry` / `TextEdit`，高度随内容变化 |
| 弹出菜单 | `PopUp` / `Window` 层 |
| 确认弹窗 | 模态 `Dialog` |
| 设置页 | 左侧列表 + 右侧 `ScrollContainer` 换页 |

**已知取舍**：原生控件无法完全复刻 CSS 细节（圆角收口气泡、渐变遮罩）。只保证信息结构与交互流程一致。

### 决策 6：数据库兼容

三份实现读写**同一个** `workbench.db`，表结构完全一致（含版本 4 迁移后的 `pinned_at` / `stopped` 字段）。

**这就要求**：Go/Rust 版的迁移逻辑必须能读 Node 版创建的库，且不破坏它。策略是**只读已有 schema，不主动迁移**——若 `schema_version < 4`，提示用户先用 Node 版启动一次完成迁移。

### 决策 7：CI 拆分

现有 `.github/workflows/release.yml` 拆成三份：

```
.github/workflows/
├── release-node.yml      tag 触发，构建 Node 版
├── release-go.yml        tag 触发，构建 Go 版
└── release-rust.yml      tag 触发，构建 Rust 版
```

三者都监听 `v*` tag，但产物名不同：

```
ClaudeWorkbench-Node-<version>-x64.exe
ClaudeWorkbench-Go-<version>-x64.exe
ClaudeWorkbench-Rust-<version>-x64.exe
```

**为什么拆而不是合并**：合并后一个实现失败会阻断其他两个；拆开后各自独立，`fail-fast` 问题消失。

## Risks / Trade-offs

| 风险 | 缓解 |
|---|---|
| Fyne 的中文渲染需系统字体，换机器可能找不到 | 启动时按候选列表查找（msyh → deng → simhei）；全部缺失时用拉丁字体并提示 |
| `mattn/go-sqlite3` 需 CGO，CI 环境可能没 gcc | 优先用 `modernc.org/sqlite`（纯 Go），避免 CGO 依赖 |
| egui 中文需手动加载字体文件 | 同 Fyne，运行时按候选路径加载 |
| 迁移 gs 到 `node/` 后 CI 全部失效 | 迁移后立刻本地验证 `npm run build` / `npm test` / 打包，CI 在 tags 前先手动触发验证 |
| Go/Rust 版功能不完整 | 以现有 8 个 spec 的 47 条需求为验收清单，逐条勾选 |
| 三份实现数据竞争同一 DB 文件 | 沿用单实例锁思路；三份各自的锁文件路径不同，避免互相干扰 |
| 原生控件表现力有限，UI 观感降低 | 明确 Non-Goal：只保证信息结构与交互一致 |

## Migration Plan

1. 本地 `git tag` 前先完成目录迁移并验证 Node 版可构建、可打包
2. 三个 workflow 文件先只保留 Node 版启用，Go/Rust 用 `workflow_dispatch` 手动验证
3. 验证通过后，三者都改为 tag 触发
4. 回滚：目录迁移是一次性提交，`git revert` 即可恢复
