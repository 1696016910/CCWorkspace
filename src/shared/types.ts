/** 工作区：与磁盘上的一个项目目录一一对应 */
export interface Workspace {
  id: string
  /** 绝对路径 */
  path: string
  /** 展示名称，默认取目录名 */
  name: string
  addedAt: string
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
