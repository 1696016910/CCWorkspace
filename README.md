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

产物统一命名为 `ClaudeWorkbench-<版本>-<架构>.<扩展名>`。必须是 ASCII：
中文名会被百分号编码进 `latest.yml` 的下载地址，多数更新代理会直接返回 400。

---

## 发版与自动更新

### 发一个新版本

```bash
npm version patch        # 0.1.0 → 0.1.1，自动 commit 并打 tag
git push --follow-tags   # 推送 tag 触发 GitHub Actions
```

Actions 会在 Windows / Linux / macOS 三个 runner 上分别构建，产物与
`latest.yml` 汇总到同一个 GitHub Release。约 45 分钟完成。

### 更新怎么工作

应用启动 5 秒后静默检查一次，之后可在「设置 → 更新」手动触发。
下载与安装分开：发现新版本由你决定何时下载，下载完成后重启生效。

更新源是 GitHub 的**固定地址**，不随版本号变化：

```
https://github.com/<owner>/<repo>/releases/latest/download/
```

`electron-updater` 从这个目录取 `latest.yml`，再按其中的相对文件名下载安装包。

| 平台 | 产物 | 自动更新 |
|---|---|---|
| Windows | `.exe` | ✅ 可用（未签名只影响首次安装的 SmartScreen，不影响更新） |
| Linux | `.AppImage` | ✅ 可用 |
| macOS | `.dmg` + `.zip` | ⚠️ 需 Apple 开发者签名，未签名时只能手动下载 |

macOS 的 Squirrel 只接受已签名的包，这是系统限制，无法绕过。
未签名时应用会把错误降级为一句提示并给出「打开发布页」按钮，不会崩溃。

### 首次安装被系统拦截怎么办

| 平台 | 现象 | 处理 |
|---|---|---|
| Windows | SmartScreen 提示"未知发布者" | 点「更多信息」→「仍要运行」 |
| macOS | 提示"已损坏"或"无法验证开发者" | 右键点应用 →「打开」，或终端执行 `xattr -dr com.apple.quarantine /Applications/Claude\ 工作台.app` |
| Linux | AppImage 报 `libfuse.so.2` 缺失 | 装 `libfuse2`，或改用 `.deb` 包 |

产物用 ad-hoc 签名（`identity: '-'`），能让 macOS 接受运行，但不满足
Squirrel 的自动更新要求——所以 mac 端更新仍需手动下载。

### 网络不通时用代理

「设置 → 更新 → 代理前缀」填一个地址即可，留空表示直连。拼接规则：

```
留空   → https://github.com/<owner>/<repo>/releases/latest/download/latest.yml
填前缀 → <前缀>/https://github.com/<owner>/<repo>/releases/latest/download/latest.yml
```

`latest.yml` 里的文件名是相对路径，下载时会继承同一个前缀，所以填一次就覆盖全部请求。

**自建一个最简单的代理**（Cloudflare Workers，免费额度足够）：

```js
export default {
  async fetch(req) {
    // 请求形如 https://<worker>/https://github.com/o/r/releases/...
    // 去掉开头的斜杠后，剩下的本身就是完整目标地址，不能再拼一次域名
    const target = new URL(req.url).pathname.slice(1) + new URL(req.url).search
    const res = await fetch(target, {
      redirect: 'follow' // latest/download 会 302 跳到真实资源，必须跟随
    })
    const headers = new Headers(res.headers)
    // 去掉编码相关头，避免中转后内容长度与实际不符
    headers.delete('content-encoding')
    headers.delete('content-length')
    return new Response(res.body, { status: res.status, headers })
  }
}
```

> 注意 `slice(1)`：如果把 `pathname` 直接拼到 `https://github.com` 后面，
> 会得到 `https://github.com/https://github.com/...`，代理必然 404。

部署后把 `https://<你的子域>.workers.dev` 填进代理前缀即可。

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
