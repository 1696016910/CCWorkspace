# Proposal

## Why

工作里有大量散落在终端里的 Claude 会话，和开发时使用的 Claude Code 混在一起，既找不到也理不清。需要把它们切成两半：日常事务性工作收进一个独立的应用里自行管理，开发工作继续留在终端。

这个应用因此必须**自成一个数据体系**——自己的会话、自己的工作区，不读取也不污染系统里 Claude Code 已有的会话记录。

## What Changes

- 新增桌面应用，工作区（项目目录）与会话全部由应用自己管理，存储在应用私有的 SQLite 数据库中
- 会话列表按工作区分组，支持新建、切换、重命名、删除、复制
- 新增应用内对话：流式回复、工具调用折叠展示、上下文用量指示
- 新增 Skill 管理：列出可用的 Skill，开关控制新会话加载哪些
- 新增插件管理：启用、禁用、安装、卸载，以及插件仓库的增删
- 新增配置管理：编辑 `env`、权限规则与模型映射，写入前备份
- **不读取** `~/.claude/projects/` 下的历史会话；应用有自己的会话库
- **不修改** 开发用的 Claude Code 会话数据；配置写入仍落在 `~/.claude/settings.json`（因为 Claude 二进制从那里读）

**不做**：代码编辑器、LSP、内置终端、Git 面板、文件浏览器、语音输入、账号与云同步、多窗口任务看板、自动更新。

## Capabilities

### New Capabilities

- `workspace-management`: 工作区的添加、移除与默认工作区设定；工作区与磁盘目录一一对应，由应用自己的数据库记录
- `session-management`: 会话在应用数据库中的持久化、按工作区分组，以及新建、重命名、删除、复制等生命周期操作
- `conversation`: 在会话内发送消息、接收流式回复、折叠工具调用、展示上下文用量
- `configuration`: `settings.json` 中 env、权限、模型三部分的读取、校验、备份与写入
- `skill-management`: 用户级与项目级 Skill 的发现、展示与启用开关
- `plugin-management`: 插件的启用/禁用/安装/卸载，以及插件仓库的增删

### Modified Capabilities

无（项目此前没有任何 spec）。

## Impact

- **新增代码**：一个 Electron 应用（主进程 + 渲染进程），此前仓库中没有应用代码
- **新增依赖**：`electron`、`electron-vite`、`vue`、`pinia`、`@anthropic-ai/claude-agent-sdk`、`marked`、`dompurify`
- **新增存储**：应用私有 SQLite 数据库（`userData/workbench.db`），保存工作区、会话、消息与配置快照
- **读取的外部文件**：`~/.claude/settings.json`、`~/.claude/skills/`、`~/.claude/agents/`
- **写入的外部文件**：`~/.claude/settings.json`（经备份与原子写）
- **不读取**：`~/.claude/projects/**/*.jsonl`（Claude Code 的开发会话，与本应用无关）
- **调用的外部命令**：`claude plugin`（安装、卸载、详情）
- **子进程**：每个活跃会话一个 `claude` 进程，由 Agent SDK 管理
- **风险面**：Claude 的配置文件格式与 CLI 子命令输出均为非公开契约，解析层必须容错
