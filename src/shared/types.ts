/** 工作区：与磁盘上的一个项目目录一一对应 */
export interface Workspace {
  id: string
  /** 绝对路径 */
  path: string
  /** 展示名称，默认取目录名 */
  name: string
  addedAt: string
  /** 是否置顶（置顶时间戳，null 表示未置顶） */
  pinnedAt: string | null
}

/** 会话在列表中的展示形态 */
export interface SessionSummary {
  id: string
  workspaceId: string
  title: string
  /** ISO 时间字符串 */
  updatedAt: string
  messageCount: number
  /** 该会话是否正在生成回复 */
  running: boolean
  /** 是否为用户手动命名 */
  renamed: boolean
  /** 上下文占用百分比 0-100+ */
  contextPercent: number
  /** 上下文已用 token */
  contextTokens: number
  /** 是否置顶（置顶时间戳，null 表示未置顶） */
  pinnedAt: string | null
}

/** 对话流中的一条消息 */
export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  text: string
  /** 该条助手消息中发生的工具调用 */
  tools?: ToolCall[]
  timestamp?: string
}

export interface ToolCall {
  name: string
  target: string
  result?: string
  ok?: boolean
}

/** 上下文用量 */
export interface ContextUsage {
  inputTokens: number
  outputTokens: number
}

/** 上下文明细：总量、窗口、百分比与分类 */
export interface ContextUsageDetail {
  model: string
  totalTokens: number
  maxTokens: number
  /** 0-100+，超过 100 表示已超窗口 */
  percentage: number
  categories: { name: string; tokens: number; kind: string }[]
}

/** 斜杠命令或 Skill */
export interface SlashCommandInfo {
  name: string
  description: string
  argumentHint: string
  builtin: boolean
}

/** 应用自身状态，持久化在 userData/config.json */
export interface AppConfig {
  workspaces: Workspace[]
  defaultWorkspace: string | null
  sessionAliases: Record<string, string>
  /** 被软删除的会话，key 为 sessionId */
  deletedSessions: Record<string, { originalPath: string; deletedAt: string }>
  /** 被用户禁用的 Skill 名称 */
  disabledSkills: string[]
  ui: { lastSessionId: string | null }
}

/** 环境变量条目 */
export interface EnvEntry {
  key: string
  value: string
}

/** Claude 配置文件的关注子集 */
export interface ClaudeSettings {
  env: Record<string, string>
  permissions: {
    defaultMode?: string
    allow: string[]
    deny: string[]
  }
  enabledPlugins: Record<string, boolean>
  extraKnownMarketplaces: Record<string, unknown>
  modelFields: Record<string, string>
}

export interface SkillInfo {
  name: string
  description: string
  source: 'user' | 'project' | 'plugin'
  enabled: boolean
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

/** IPC 统一返回结构 */
export type Result<T> = { ok: true; data: T } | { ok: false; error: string }

/** 自动更新状态机 */
export type UpdateState =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'downloaded'
  | 'error'

export interface UpdateStatus {
  state: UpdateState
  currentVersion: string
  /** 新版本号，仅 available / downloaded 有值 */
  version?: string
  /** 下载进度 0-100 */
  percent?: number
  /** 失败原因或"已是最新"等提示 */
  message?: string
  /** 实际请求的更新地址，含代理前缀，便于排查 */
  feed: string
}
