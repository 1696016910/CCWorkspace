# Claude 工作台

Claude Code CLI 的图形外壳：把终端里的会话、工作区、配置、插件、Skill 管理搬到桌面上。

不做的：代码编辑器、LSP、终端、Git、文件浏览器、账号与云同步。

---

## 技术栈

| 层 | 技术 |
|---|---|
| 界面 | Vue 3 + TypeScript + Vite |
| 外壳 | Tauri 2（系统 WebView2） |
| 后端 | Rust（SQLite、claude 子进程、配置读写） |

界面用系统 WebView 渲染，不打包 Chromium；后端是原生 Rust。所以安装包 1.8MB，启动 0.5 秒级。

依赖 PATH 中的 `claude` 可执行文件，不内置。

---

## 目录结构

```
.
├── src/                     前端（Vue 3）
│   ├── api/                 IPC 封装，与后端命令一一对应
│   ├── components/          主界面组件（Sidebar / ChatPane / MessageItem）
│   │   ├── dialogs/         弹窗与浮层（确认 / 新建工作区 / 回收站 / 文件预览 / 上下文）
│   │   └── settings/        设置页 8 个分区
│   ├── stores/              Pinia：workspace / session / chat
│   ├── styles/              设计令牌与全局样式
│   ├── utils/               Markdown 渲染、主题
│   ├── App.vue
│   └── main.ts
├── src-tauri/               后端（Rust）
│   ├── src/
│   │   ├── main.rs          IPC 命令注册
│   │   ├── db.rs            schema 与迁移
│   │   ├── service.rs       工作区 / 会话 / 消息
│   │   ├── config.rs        settings.json 读写
│   │   ├── claude.rs        claude CLI 子进程与协议解析
│   │   ├── skills.rs        Skill / 插件 / 仓库
│   │   └── file.rs          工作区内文件读取
│   ├── icons/               应用图标（多尺寸）
│   └── tauri.conf.json
├── assets/                  图标源图（1024px，用 tauri icon 生成全套）
├── prototype/               设计稿（改 UI 时对照）
├── docs/                    claude CLI stdio 协议说明
└── openspec/                规格与变更记录
```

---

## 开发

需要 Node.js 20+、Rust 1.77+、以及 Windows 上的 WebView2（Win11 自带）。

```bash
npm install
npm run tauri:dev
```

`tauri:dev` 会同时起 Vite 和 Rust 后端，改前端热更新，改 Rust 自动重编译。

---

## 测试

```bash
npm test                    # 前端（Vitest）
cd src-tauri && cargo test   # 后端
```

---

## 图标

改了 `assets/icon-source.png` 后重新生成全套尺寸：

```bash
npx tauri icon assets/icon-source.png -o src-tauri/icons
```

## 打包

```bash
npm run tauri:build
```

产物在 `src-tauri/target/release/bundle/`：
- `nsis/Claude 工作台_<版本>_x64-setup.exe` — 安装包（推荐）
- `msi/…` — 需要 WiX，默认不生成

只想要免安装的 exe：

```bash
npm run tauri:build -- --no-bundle
```

产物在 `src-tauri/target/release/`。**注意**：Windows GNU 工具链下 `WebView2Loader.dll`
是运行时动态加载的，必须和 exe 放在同一目录，否则报「找不到 WebView2Loader.dll」。
想得到单文件，改用 MSVC 工具链（`rustup target add x86_64-pc-windows-msvc`）。

---

## 数据

与 claude CLI 共用配置：

- 数据库：`%APPDATA%/claude-workbench/workbench.db`（SQLite，schema v4）
- 配置：`~/.claude/settings.json`（保存前自动备份到 `~/.claude/backups/`）

---

## 授权

私有项目。
