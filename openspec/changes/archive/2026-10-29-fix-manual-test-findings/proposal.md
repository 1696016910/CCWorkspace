# Proposal

## Why

0.2.1 版做了 14 条手工测试，暴露的问题集中在三类：

1. **停止按钮失效**——点停止没反应，10 秒后内容一起涌出。模型思考期间无输出时，
   停止标志根本没机会被读到；`child.kill()` 只杀了 `cmd.exe`，真正的 `claude` 进程存活。
2. **只支持单会话单轮**——全局一个 `ActiveRun` 槽位、前端一份 `streaming` 状态，
   导致一个会话在跑时所有会话都显示停止按钮、都发不出消息。
3. **半暗色化与占位承诺**——暗色下多个控件白底白字；输入框 placeholder 承诺
   「输入 / 调用命令与 Skill」但该功能从未实现。

这些问题在多会话并行使用时会互相放大，必须在下一版集中修掉。

## What Changes

**对话与并发**

- 并发：`active` 从单槽位改为 `HashMap<session_id, ActiveRun>`，上限 10 轮；
  `chat_interrupt` 改为可指定目标轮次
- 停止：Windows 下杀整棵进程树；stdout 读取移入独立线程，主循环按 100ms 轮询停止标志；
  补上空闲超时（文档早已承诺但代码未实现）
- 流式事件：后端按 60-100ms 攒批推送，替代当前「每个 token 一次 IPC」
- 附件：支持拖拽 / 粘贴 / 选择文件，缩略图卡片，上限 6 个；已发送消息不保留附件
- 上下文压缩：改为异步执行；对话进行中禁止压缩
- 草稿：按会话保存输入草稿，切换会话不串味
- `/` 补全面板：从 CLI 的 `slash_commands` 取全量清单（实测 143 条，含用户 Skill、命令与内置命令）

**界面**

- 暗色模式：补 4 组设计令牌，替换 16 处写死的浅色
- 上下文弹窗改为锚定百分比按钮，不再靠「居中 + 固定 110px」猜位置
- 侧栏增加「生成中」与「已完成未查看」两种状态标记
- 文件卡片：修正正则误判；工作区外文件灰显不可点
- 设置页：权限规则改表格（增删 + 空行标红 + 重复拦截）；新增「规则」分区；
  去掉与关闭按钮重复的「返回」

**不做的事情**

- 不做 `@` 引用文件
- 不把附件写进数据库（历史回放不显示当时发的图）
- 不限制附件大小（超过 4.5MB 只标黄提示，让失败可解释）
- 不做跨重启的「未查看」状态（只在本次运行内有效）
- 不开放工作区外的文件读取通道

## Capabilities

### New Capabilities

> `openspec/specs/` 当前为空（此前整体清空过），因此以下四条均按新能力建立，
> 不存在可引用的既有 delta 路径。

- `conversation`: 对话的中断语义、并发轮次、附件、上下文压缩、草稿与 `/` 补全
- `configuration`: 权限规则编辑、规则分区、Skill 视图
- `app-experience`: 暗色模式完整性、浮层定位、侧栏状态标记、设置页导航
- `file-preview`: 对话中被引用的文件路径识别与预览边界

### Modified Capabilities

无。既有 spec 库为空，没有可修改的对象。

## Impact

**受影响的代码**

| 位置 | 影响 |
|---|---|
| `src-tauri/src/main.rs` | `AppState.active` 改 Map；`chat_interrupt` 签名变更；`context_compact` 改异步；新增 `file_check`、`command_list`、`rule_list`、`workspace_reveal` |
| `src-tauri/src/claude.rs` | 进程树终止、读线程 + 停止轮询、空闲超时、init 事件解析 `slash_commands` |
| `src-tauri/src/file.rs` | `within()` 归一斜杠；补 `within` 单元测试 |
| `src-tauri/src/skills.rs` | 删 `toggle_skill`；新增规则扫描与增删改 |
| `src/stores/chat.ts` | 单例状态改 `Map<sessionId, ...>`；草稿与未查看集合 |
| `src/components/ChatPane.vue` | 按钮状态按会话判断；锚点定位；附件区 |
| `src/utils/markdown.ts` | `extractFilePaths` 正则收紧到扩展名白名单 |
| `src/styles/global.css` | 新增暗色令牌 |
| `src/components/settings/*` | 权限表格、规则分区、按钮样式 |

**接口变更（BREAKING，仅内部）**

- `chat_interrupt` 从无参改为接受 `runId`
- `skill_toggle` 删除
- `SkillInfo.enabled` 语义改变（不再有开关）

**新增依赖**

无。不引入 shell / opener / notification 插件。

**数据影响**

- 数据库 schema 不变
- `meta.disabled_skills` 键保留但不再读写

**验证要求**

14 条手工测试问题逐条复测；停止与并发相关改动需在真机确认无残留 `claude` 进程。
