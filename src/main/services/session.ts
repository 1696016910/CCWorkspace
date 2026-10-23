import { randomUUID } from 'node:crypto'
import type { ChatMessage, SessionSummary } from '../../shared/types'
import { getDb } from '../db'
import { resolveWorkspaceId } from './workspace'

const TITLE_MAX = 40

interface SessionRow {
  id: string
  workspace_id: string
  title: string
  auto_title: number
  created_at: string
  updated_at: string
  deleted_at: string | null
  message_count: number
  claude_session_id: string | null
  context_tokens: number
  context_max: number
  context_percent: number
}

interface MessageRow {
  id: string
  role: string
  content: string
  tools: string | null
  created_at: string
}

function toSummary(row: SessionRow): SessionSummary {
  return {
    id: row.id,
    workspaceId: row.workspace_id,
    title: row.title || '(新会话)',
    updatedAt: row.updated_at,
    messageCount: row.message_count,
    running: false,
    renamed: row.auto_title === 0,
    contextPercent: row.context_percent ?? 0,
    contextTokens: row.context_tokens ?? 0
  }
}

/** 从首条用户消息推导标题 */
export function deriveTitle(text: string): string {
  const oneLine = text.replace(/\s+/g, ' ').trim()
  if (!oneLine) return '(新会话)'
  return oneLine.length > TITLE_MAX ? `${oneLine.slice(0, TITLE_MAX)}…` : oneLine
}

/** 列出未删除的会话，按工作区分组由渲染层处理 */
export function listSessions(): SessionSummary[] {
  const rows = getDb()
    .prepare(
      `SELECT s.id, s.workspace_id, s.title, s.auto_title, s.created_at, s.updated_at, s.deleted_at,
              s.claude_session_id, s.context_tokens, s.context_max, s.context_percent,
              (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) AS message_count
         FROM sessions s
        WHERE s.deleted_at IS NULL
        ORDER BY s.updated_at DESC`
    )
    .all() as unknown as SessionRow[]
  return rows.map(toSummary)
}

export function listDeletedSessions(): SessionSummary[] {
  const rows = getDb()
    .prepare(
      `SELECT s.id, s.workspace_id, s.title, s.auto_title, s.created_at, s.updated_at, s.deleted_at,
              s.claude_session_id, s.context_tokens, s.context_max, s.context_percent,
              (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) AS message_count
         FROM sessions s
        WHERE s.deleted_at IS NOT NULL
        ORDER BY s.deleted_at DESC`
    )
    .all() as unknown as SessionRow[]
  return rows.map(toSummary)
}

/**
 * 新建会话。未指定工作区时回落到默认工作区；
 * 没有任何工作区时拒绝创建，由上层提示用户。
 */
export function createSession(workspaceId: string | null): SessionSummary {
  const target = resolveWorkspaceId(workspaceId)
  if (!target) throw new Error('请先添加工作区')

  const now = new Date().toISOString()
  const id = randomUUID()
  getDb()
    .prepare(
      'INSERT INTO sessions(id, workspace_id, title, auto_title, created_at, updated_at) VALUES(?, ?, ?, 1, ?, ?)'
    )
    .run(id, target, '', now, now)

  const row = getDb()
    .prepare(
      `SELECT s.id, s.workspace_id, s.title, s.auto_title, s.created_at, s.updated_at, s.deleted_at,
              s.claude_session_id, s.context_tokens, s.context_max, s.context_percent,
              0 AS message_count
         FROM sessions s WHERE s.id = ?`
    )
    .get(id) as unknown as SessionRow
  return toSummary(row)
}

/** 重命名：关闭 auto_title，之后不再被首条消息覆盖 */
export function renameSession(id: string, title: string): void {
  const trimmed = title.trim()
  if (!trimmed) throw new Error('标题不能为空')
  getDb()
    .prepare('UPDATE sessions SET title = ?, auto_title = 0, updated_at = ? WHERE id = ?')
    .run(trimmed, new Date().toISOString(), id)
}

/** 软删除：只打标记，消息保留以便恢复 */
export function deleteSession(id: string): void {
  getDb()
    .prepare('UPDATE sessions SET deleted_at = ? WHERE id = ?')
    .run(new Date().toISOString(), id)
}

export function restoreSession(id: string): void {
  getDb().prepare('UPDATE sessions SET deleted_at = NULL WHERE id = ?').run(id)
}

/** 彻底删除：连消息一起从数据库中移除，不可恢复 */
export function purgeSession(id: string): void {
  const db = getDb()
  db.exec('BEGIN')
  try {
    db.prepare('DELETE FROM messages WHERE session_id = ?').run(id)
    db.prepare('DELETE FROM sessions WHERE id = ?').run(id)
    db.exec('COMMIT')
  } catch (e) {
    db.exec('ROLLBACK')
    throw e
  }
}

/** 复制会话：连同消息一起复制，生成新的 id */
export function duplicateSession(id: string): SessionSummary {
  const db = getDb()
  const src = db.prepare('SELECT * FROM sessions WHERE id = ?').get(id) as
    | SessionRow
    | undefined
  if (!src) throw new Error('会话不存在')

  const now = new Date().toISOString()
  const newId = randomUUID()
  const title = src.title ? `${src.title} 副本` : ''

  db.exec('BEGIN')
  try {
    db.prepare(
      'INSERT INTO sessions(id, workspace_id, title, auto_title, created_at, updated_at) VALUES(?, ?, ?, 0, ?, ?)'
    ).run(newId, src.workspace_id, title, now, now)

    const messages = db
      .prepare('SELECT id, role, content, tools, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC')
      .all(id) as unknown as MessageRow[]
    const insert = db.prepare(
      'INSERT INTO messages(id, session_id, role, content, tools, created_at) VALUES(?, ?, ?, ?, ?, ?)'
    )
    for (const m of messages) {
      insert.run(randomUUID(), newId, m.role, m.content, m.tools, m.created_at)
    }
    db.exec('COMMIT')
  } catch (e) {
    db.exec('ROLLBACK')
    throw e
  }

  const row = db
    .prepare(
      `SELECT s.id, s.workspace_id, s.title, s.auto_title, s.created_at, s.updated_at, s.deleted_at,
              s.claude_session_id, s.context_tokens, s.context_max, s.context_percent,
              (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) AS message_count
         FROM sessions s WHERE s.id = ?`
    )
    .get(newId) as unknown as SessionRow
  return toSummary(row)
}

/** 读取某会话的全部消息 */
export function listMessages(sessionId: string): ChatMessage[] {
  const rows = getDb()
    .prepare('SELECT id, role, content, tools, created_at FROM messages WHERE session_id = ? ORDER BY created_at ASC')
    .all(sessionId) as unknown as MessageRow[]

  return rows.map((r) => ({
    id: r.id,
    role: r.role === 'assistant' ? 'assistant' : 'user',
    text: r.content,
    tools: r.tools ? (JSON.parse(r.tools) as ChatMessage['tools']) : undefined,
    timestamp: r.created_at
  }))
}

/** 追加一条消息；首条用户消息会顺带补上自动标题 */
export function appendMessage(
  sessionId: string,
  role: 'user' | 'assistant',
  content: string,
  tools?: ChatMessage['tools']
): ChatMessage {
  const db = getDb()
  const now = new Date().toISOString()
  const id = randomUUID()

  db.prepare(
    'INSERT INTO messages(id, session_id, role, content, tools, created_at) VALUES(?, ?, ?, ?, ?, ?)'
  ).run(id, sessionId, role, content, tools ? JSON.stringify(tools) : null, now)

  db.prepare('UPDATE sessions SET updated_at = ? WHERE id = ?').run(now, sessionId)

  // 首条用户消息补自动标题，但用户已手动命名的会话不覆盖
  if (role === 'user') {
    const s = db.prepare('SELECT auto_title, title FROM sessions WHERE id = ?').get(sessionId) as
      | { auto_title: number; title: string }
      | undefined
    if (s && s.auto_title === 1 && !s.title) {
      db.prepare('UPDATE sessions SET title = ? WHERE id = ?').run(deriveTitle(content), sessionId)
    }
  }

  return { id, role, text: content, tools, timestamp: now }
}

/** 取得会话绑定的 Claude CLI 会话 id（用于 resume 续接上下文） */
export function getClaudeSessionId(sessionId: string): string | null {
  const row = getDb()
    .prepare('SELECT claude_session_id FROM sessions WHERE id = ?')
    .get(sessionId) as { claude_session_id?: string | null } | undefined
  return row?.claude_session_id ?? null
}

/** 记住 CLI 返回的会话 id，后续轮次用它 resume */
export function setClaudeSessionId(sessionId: string, claudeSessionId: string): void {
  getDb()
    .prepare('UPDATE sessions SET claude_session_id = ? WHERE id = ?')
    .run(claudeSessionId, sessionId)
}

/** 持久化上下文用量 */
export function setContextUsage(
  sessionId: string,
  tokens: number,
  max: number,
  percent: number
): void {
  getDb()
    .prepare(
      'UPDATE sessions SET context_tokens = ?, context_max = ?, context_percent = ? WHERE id = ?'
    )
    .run(tokens, max, percent, sessionId)
}

