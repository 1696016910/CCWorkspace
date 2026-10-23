# Proposal

## Why

MVP 功能已经落地，但在真实使用与系统性审计中暴露出若干**会导致应用不可用**的问题：数据库文件损坏时应用直接起不来、IPC 调用阻塞十余秒、设置保存因响应式代理序列化失败而报错、连接不可达时界面永久停在「生成中」。同时，作为要给他人使用的产品，缺少错误边界、统一提示、键盘可达性与暗色主题。

## What Changes

- **健壮性**：数据库损坏时自动备份并以空库启动；IPC 输入全部校验并返回可读错误；发送消息不再阻塞 IPC；连接异常有超时兜底
- **正确性**：修复 Markdown 净化在运行时崩溃（DOMPurify 工厂函数误用）；修复会话切换时消息串写；修复上下文与命令清单在 query 关闭后无法获取
- **交互**：会话内停止/重发、消息复制、文件路径点击打开、图片粘贴、上下文面板分类明细与压缩
- **视觉**：统一设计变量、暗色主题、消息入场动画、呼吸式生成指示、骨架屏、hover 操作
- **产品化**：错误边界、统一 toast、Esc 关闭弹层、Ctrl+, 打开设置、可访问性（title/label/焦点）

## Capabilities

### New Capabilities

- `app-resilience`: 存储损坏恢复、IPC 输入校验、超时兜底、错误边界与用户可见的错误提示
- `app-experience`: 主题切换、动效、加载与空状态、键盘可达性、全局反馈

### Modified Capabilities

- `conversation`: 发送改为非阻塞、按会话隔离流式状态、停止后恢复、上下文与命令的获取时机
- `configuration`: 保存路径修复响应式代理序列化、错误信息可读、移除工作区前确认并说明数据后果
- `workspace-management`: 支持重命名、展示路径、移除前的破坏性提示
- `session-management`: 会话重命名/删除入口、未命名会话序号、复制与软删恢复

## Impact

- **主进程**：`db/index.ts` 重写损坏恢复；`chat.ts` 重构发送循环（超时、事件分类、非阻塞）；`index.ts` 增加同步校验与错误转发
- **渲染进程**：新增 `ErrorBoundary`、`ToastHost`、`useToast`、`ContextPopover`；`ChatPane`/`Sidebar`/`MessageItem` 大量交互增强
- **测试**：新增 Markdown 安全、文件路径识别、标题推导、环境变量校验共 20 条单测
- **风险**：`node:sqlite` 依赖 Electron 45；主题变量需覆盖所有硬编码颜色
