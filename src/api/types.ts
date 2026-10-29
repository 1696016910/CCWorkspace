/** 与后端 service.rs 的序列化结构一一对应 */

export interface Workspace {
  id: string
  path: string
  name: string
  addedAt: string
  pinnedAt: string | null
}

export interface SessionSummary {
  id: string
  workspaceId: string
  title: string
  updatedAt: string
  messageCount: number
  running: boolean
  renamed: boolean
  contextPercent: number
  contextTokens: number
  contextMax: number
  pinnedAt: string | null
}

export interface ToolCall {
  name: string
  target: string
}

export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  text: string
  tools?: ToolCall[]
  timestamp?: string
  stopped?: boolean
  /** 这条消息带过的附件。图片只存路径，不存内容。 */
  attachments?: StoredAttachment[]
  /** 本轮耗时（毫秒）。只在本次运行内有效，不落库。 */
  durationMs?: number
}

/** 已落库的附件元数据。与待发送的 Attachment 区分：没有 base64 与预览。 */
export interface StoredAttachment {
  kind: 'image' | 'file'
  name: string
  path?: string
  size: number
}

export interface EnvEntry {
  key: string
  value: string
}

export interface Permissions {
  defaultMode: string
  allow: string[]
  deny: string[]
}

export interface SkillInfo {
  name: string
  description: string
  source: string
  /** 目录内除 SKILL.md 外的文件数，删除时用来提示连带影响 */
  extraFiles: number
}

export interface RuleInfo {
  name: string
  description: string
  source: string
}

export interface CommandInfo {
  name: string
  description: string
  source: string
}

/** CLI 在 init 事件里上报的能力清单，首轮对话之后才有值 */
export interface CliMeta {
  slashCommands: string[]
  skills: string[]
  agents: string[]
  plugins: string[]
}

/** 待发送的附件。只有图片能真正传给模型，其余作为路径提示参与发送。 */
export interface Attachment {
  id: string
  name: string
  /** image = 有 base64 可发给模型；file = 只带路径 */
  kind: 'image' | 'file'
  /** 本地绝对路径；剪贴板截图没有路径 */
  path?: string
  size: number
  /** 仅图片：data URL 形式，用于卡片预览 */
  preview?: string
  /** 仅图片：纯 base64（不含 data: 前缀） */
  data?: string
  mediaType?: string
}

export interface PluginInfo {
  id: string
  name: string
  version: string
  marketplace: string
  enabled: boolean
}

export interface MarketplaceInfo {
  name: string
  source: string
  builtin: boolean
}

export type ChatEvent =
  | { type: 'start'; sessionId: string; runId: number }
  | { type: 'text'; sessionId: string; runId: number; delta: string }
  | { type: 'tool'; sessionId: string; runId: number; tool: ToolCall }
  | { type: 'meta'; sessionId: string; runId: number }
  | { type: 'context'; sessionId: string; runId: number; usage?: { tokens: number; max: number; percent: number } }
  | {
      type: 'done'
      sessionId: string
      runId: number
      message: ChatMessage
      interrupted?: boolean
      /** 本轮统计。CLI 在 result 里给的耗时 */
      usage?: { durationMs?: number; ttftMs?: number; numTurns?: number }
    }
  | { type: 'error'; sessionId: string; runId: number; error: string }

/** 压缩结果事件，与 chat:event 分开推送 */
export interface CompactResult {
  sessionId: string
  ok: boolean
  error: string | null
}
