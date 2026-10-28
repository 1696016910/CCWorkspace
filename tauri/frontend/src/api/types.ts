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

export type ChatEvent =
  | { type: 'start'; sessionId: string; runId: number }
  | { type: 'text'; sessionId: string; runId: number; delta: string }
  | { type: 'tool'; sessionId: string; runId: number; tool: ToolCall }
  | { type: 'done'; sessionId: string; runId: number; message: ChatMessage }
  | { type: 'error'; sessionId: string; runId: number; error: string }
