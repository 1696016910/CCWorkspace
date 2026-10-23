# Proposal

## Why

Claude Code 的会话、配置、Skill 和插件全部散落在 `~/.claude` 的文件系统里，日常只能靠终端命令和手工改 JSON 来管理。换一个项目就要重新 `cd` 再 `claude --resume`，想换模型要打开配置文件，想知道哪个插件在加载也没有地方看。

这套东西本身数据齐全，只是缺一个能点、能看、能改的界面。做一个桌面应用把它们收进窗口，可以省掉每天的终端往返和手工编辑配置的风险。

## What Changes

- 新增桌面应用，提供工作区（项目目录）的添加、移除与默认工作区设置
- 新增按工作区分组的会话列表，支持新建、切换、重命名、删除、复制
- 新增应用内对话：流式回复、工具调用折叠展示、上下文用量指示
- 新增 Skill 管理：列出用户级与项目级 Skill，开关控制新会话加载哪些
- 新增插件管理：启用、禁用、安装、卸载，以及插件仓库（marketplace）的增删
- 新增配置管理：编辑 `~/.claude/settings.json` 的 `env`、权限规则与模型映射，写入前备份
- 应用本身不建立会话数据库，会话数据直接读取 Claude 已写入的 jsonl 记录

**不做**：代码编辑器、LSP、内置终端、Git 面板、文件浏览器、语音输入、账号与云同步、多窗口任务看板、自动更新。

## Capabilities

### New Capabilities

- `workspace-management`: 工作区的发现、添加、移除与默认工作区设定；工作区与磁盘目录一一对应
- `session-management`: 会话在磁盘上的发现与分组，以及新建、重命名、删除、复制等生命周期操作
- `conversation`: 在会话内发送消息、接收流式回复、折叠工具调用、展示上下文用量
- `configuration`: `settings.json` 中 env、权限、模型三部分的读取、校验、备份与写入
- `skill-management`: 用户级与项目级 Skill 的发现、展示与启用开关
- `plugin-management`: 插件的启用/禁用/安装/卸载，以及插件仓库的增删

### Modified Capabilities

无（项目此前没有任何 spec）。

## Impact

- **新增代码**：一个 Electron 应用（主进程 + 渲染进程），此前仓库中没有应用代码
- **新增依赖**：`electron`、`electron-vite`、`vue`、`pinia`、`@anthropic-ai/claude-agent-sdk`、`marked`、`dompurify`
- **读取的外部文件**：`~/.claude/settings.json`、`~/.claude.json`、`~/.claude/projects/**/*.jsonl`、`~/.claude/skills/`、`~/.claude/agents/`
- **写入的外部文件**：`~/.claude/settings.json`（经备份与原子写），以及应用自身的配置文件
- **调用的外部命令**：`claude plugin`（安装、卸载、详情）、`claude agents --json`（检测会话占用）
- **子进程**：每个活跃会话一个 `claude` 进程，由 Agent SDK 管理
- **风险面**：Claude 的文件格式与 CLI 子命令输出均为非公开契约，解析层必须容错
