# Proposal

## Why

当前只有 Electron + Node 的单一实现，安装包 285MB，其中约 285MB 是 Chromium 与内置的 claude 二进制。日常使用中，Electron 的内存占用与启动时间明显高于同等功能的原生应用。

同时这个项目已经沉淀了完整的交互设计与功能规格（8 个 capability、47 条需求、三份设计稿）。把它们在 Go 与 Rust 上各实现一次，可以验证这些规格是否与语言/框架无关，并得到一个更轻量的自用客户端。

## What Changes

- **目录重构**：现有 Node 内容整体迁入 `node/`，根目录只保留 `prototype/`、`openspec/` 与说明文件。**BREAKING**：所有构建路径、CI 配置随之变化。
- **新增 Go 实现**（`go/`）：Fyne GUI，纯 Go，复用同一套 claude CLI stdio 协议。
- **新增 Rust 实现**（`rust/`）：egui GUI，纯 Rust，同一协议。
- **CI 拆分**：现有 `release.yml` 拆为按实现分工的三个 workflow 文件，各自独立触发与产物。
- **协议文档化**：把 claude CLI 的 `stream-json` 输入输出协议、事件类型、错误分类写成文档，作为三份实现的共同依据。

三份实现**代码完全独立**，不共享源码、不共享构建产物。Go/Rust 版不内嵌 claude 二进制，运行时从 PATH 查找。

## Capabilities

### New Capabilities

无。本次不引入新的用户可见功能——Go/Rust 版实现的是现有 8 个 capability 已经描述的行为。按 spec 的语义，行为未变则规格不变。

### Modified Capabilities

无。现有需求在 Go/Rust 版中逐条实现，不增不删不改。

**因此本次变更在 `.openspec.yaml` 中标记 `skip_specs: true`**：这是实现层的移植与重构，没有 spec 级行为变化。

## Impact

**受影响的代码**

| 位置 | 影响 |
|---|---|
| `src/**` → `node/src/**` | 全部文件移动 |
| `package.json` / `tsconfig.*.json` / `vitest.config.ts` | 路径与 include 调整 |
| `electron.vite.config.ts` / `electron-builder.yml` | 输出路径调整 |
| `.github/workflows/release.yml` | 拆成三份 |
| `scripts/*.mjs` | 路径基准调整 |

**新增依赖**

| 实现 | 依赖 |
|---|---|
| Go | Fyne v2（GUI）、`mattn/go-sqlite3`（数据库） |
| Rust | `eframe`/`egui`（GUI）、`rusqlite`（数据库）、`serde_json` |

**运行时依赖**

- 三份实现都依赖 PATH 中的 `claude` 可执行文件（不内置）
- Windows 下 Go/Rust 版需要系统中文字体（微软雅黑或等线）

**不改变的**

- SQLite 数据库文件格式与位置（`%APPDATA%/claude-workbench/workbench.db`）
- `~/.claude/settings.json` 的读写行为
- 与终端里 Claude Code 会话的隔离关系
- 现有 Node 版的全部功能与交互
