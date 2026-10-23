# Claude 工作台

一个桌面应用，把 Claude Code 的**会话**、**配置**、**Skill** 和**插件**收进窗口里管理。

会话数据完全由本应用自己持有，与终端里开发用的 Claude Code 会话**互不干扰**。

---

## 它做什么

| 功能 | 说明 |
|---|---|
| 工作区 | 添加项目目录，指定默认工作区 |
| 会话 | 每个工作区下新建、切换、重命名、删除、复制会话 |
| 对话 | 流式回复、工具调用折叠、上下文用量显示 |
| Skill | 列出可用 Skill，开关控制新会话加载哪些 |
| 插件 | 启用/禁用/卸载，管理插件仓库 |
| 配置 | 编辑 `settings.json` 的 env、权限、模型，写入前备份 |

不做的：代码编辑器、LSP、终端、Git、文件浏览器、账号与云同步。

---

## 安装与运行

需要 Node.js 20+ 与 Electron 45。

```bash
npm install
npm run dev        # 开发模式
```

首次使用 Electron 时如果二进制下载失败，设置国内镜像：

```bash
export ELECTRON_MIRROR="https://npmmirror.com/mirrors/electron/"
node node_modules/electron/install.js
```

### 打包

```bash
npm run dist        # Windows
npm run dist:mac    # macOS
npm run dist:linux  # Linux
```

产物在 `release/` 下。

---

## 首次启动

1. 点击左栏的「添加工作区」，选择一个项目目录
2. 点击「新建会话」，输入第一条消息
3. 回复会流式显示，工具调用折叠为「已处理 · N 步」

应用不会读取你终端里已有的 Claude Code 会话，也不会往 `~/.claude/projects` 写任何东西。

---

## 数据位置

| 内容 | 位置 |
|---|---|
| 会话、消息、工作区 | `%APPDATA%/claude-workbench/workbench.db`（Windows）<br>`~/Library/Application Support/claude-workbench/`（macOS）<br>`~/.config/claude-workbench/`（Linux） |
| 配置修改 | `~/.claude/settings.json` |
| 配置备份 | `~/.claude/backups/settings-<时间戳>.json` |

删除 `workbench.db` 即恢复初始状态（会话数据全部丢失，不可恢复）。

---

## 已知限制

- **单会话**：同一时间只能跑一个会话的回复，切换会话不会中断正在生成的回复
- **权限模式固定**：会话使用 `acceptEdits`，暂不支持在会话内切换权限
- **模型只在设置里改**：会话界面不提供模型选择器
- **Skill 开关**影响新会话，已在运行的会话不受影响
- **插件安装**依赖本机的 `claude` CLI；找不到时插件页会显示错误但不会崩溃

---

## 开发

```bash
npm run dev        # 热重载
npm test           # 单元测试
npm run typecheck  # 类型检查
npm run build      # 构建（不含打包）
```

目录结构：

```
src/
├── main/          主进程
│   ├── db/        SQLite schema 与迁移
│   └── services/  工作区、会话、对话、配置、Skill、插件
├── preload/       IPC 白名单
├── renderer/      Vue 3 界面
└── shared/        两端共用的类型
```

数据库使用 Node 内置的 `node:sqlite`，不需要编译原生模块。

---

## 规划文档

- 方案：`20261023-项目方案.md`
- 界面原型：`prototype/20261023-界面原型.html`
- 变更规格：`openspec/changes/claude-workbench-mvp/`
