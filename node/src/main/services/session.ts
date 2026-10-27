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
  pinned_at?: string | null
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
  stopped?: number
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
    contextTokens: row.context_tokens ?? 0,
    pinnedAt: row.pinned_at ?? null
  }
}

/** 从首条用户消息推导标题 */
export function deriveTitle(text: string): string {
  const oneLine = text.replace(/\s+/g, ' ').trim()
  if (!oneLine) return '(新会话)'
  if (oneLine.length <= TITLE_MAX) return oneLine

  // 直接 slice 会从一个 emoji 的代理对中间切开，留下孤立半字符渲染成方块。
  // 用 Array.from 按码点切，再按码元预算回退到不超长的位置。
  let cut = ''
  for (const ch of oneLine) {
    if (cut.length + ch.length > TITLE_MAX) break
    cut += ch
  }
  return `${cut}…`
}

/** 列出未删除的会话，按工作区分组由渲染层处理 */
export function listSessions(): SessionSummary[] {
  const rows = getDb()
    .prepare(
      `SELECT s.id, s.workspace_id, s.title, s.auto_title, s.created_at, s.updated_at, s.deleted_at,
              s.claude_session_id, s.context_tokens, s.context_max, s.context_percent, s.pinned_at,
              COUNT(m.id) AS message_count
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
        WHERE s.deleted_at IS NULL
        GROUP BY s.id
        ORDER BY s.pinned_at IS NULL, s.pinned_at DESC, s.updated_at DESC`
    )
    .all() as unknown as SessionRow[]
  return rows.map(toSummary)
}

export function listDeletedSessions(): SessionSummary[] {
  const rows = getDb()
    .prepare(
      `SELECT s.id, s.workspace_id, s.title, s.auto_title, s.created_at, s.updated_at, s.deleted_at,
              s.claude_session_id, s.context_tokens, s.context_max, s.context_percent,
              COUNT(m.id) AS message_count
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
        WHERE s.deleted_at IS NOT NULL
        GROUP BY s.id
        ORDER BY s.deleted_at DESC`
    )
    .all() as unknown as SessionRow[]
  return rows.map(toSummary)
}

/**
 * 新建会话。会话总是归属某个工作区，未指定时拒绝创建。
 */
export function createSession(workspaceId: string | null): SessionSummary {
  const target = resolveWorkspaceId(workspaceId)
  if (!target) throw new Error('请先选择一个工作区')

  const exists = getDb().prepare('SELECT id FROM workspaces WHERE id = ?').get(target)
  if (!exists) throw new Error('指定的工作区不存在')

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
  const result = getDb()
    .prepare('UPDATE sessions SET title = ?, auto_title = 0, updated_at = ? WHERE id = ?')
    .run(trimmed, new Date().toISOString(), id)
  if (result.changes === 0) throw new Error('会话不存在')
}

/** 软删除：只打标记，消息保留以便恢复 */
export function deleteSession(id: string): void {
  const result = getDb()
    .prepare('UPDATE sessions SET deleted_at = ? WHERE id = ?')
    .run(new Date().toISOString(), id)
  if (result.changes === 0) throw new Error('会话不存在')
}

export function restoreSession(id: string): void {
  const result = getDb().prepare('UPDATE sessions SET deleted_at = NULL WHERE id = ?').run(id)
  if (result.changes === 0) throw new Error('会话不存在')
}

/** 彻底删除：连消息一起从数据库中移除，不可恢复 */
export function purgeSession(id: string): void {
  const db = getDb()
  const exists = db.prepare('SELECT id FROM sessions WHERE id = ?').get(id)
  if (!exists) throw new Error('会话不存在')
  db.exec('BEGIN')
  try {
    db.prepare('DELETE FROM messages WHERE session_id = ?').run(id)
    db.prepare('DELETE FROM sessions WHERE id = ?').run(id)
    db.exec('COMMIT')
  } catch (e) {
    // SQLITE_FULL 等错误会让 SQLite 自动回滚，此时 ROLLBACK 会抛
    // "cannot rollback - no transaction is active"，把真实原因盖掉
    try {
      db.exec('ROLLBACK')
    } catch {
      // 事务已不存在
    }
    throw e
  }
}

/** 置顶 / 取消置顶会话 */
export function setSessionPinned(id: string, pinned: boolean): void {
  const db = getDb()
  const exists = db.prepare('SELECT id FROM sessions WHERE id = ?').get(id)
  if (!exists) throw new Error('会话不存在')
  db.prepare('UPDATE sessions SET pinned_at = ? WHERE id = ?').run(
    pinned ? new Date().toISOString() : null,
    id
  )
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
  // 源会话还没有标题时，副本也必须保留 auto_title=1，
  // 否则它永远等不到首条消息自动命名，侧栏会一直显示"新会话"。
  const autoTitle = src.title ? 0 : 1

  db.exec('BEGIN')
  try {
    // 不继承 claude_session_id：副本是新的对话分支，
    // 复用原 CLI 会话会让两条线互相污染。界面上有历史消息，
    // 但模型上下文从零开始，这一点由 UI 的上下文占比体现。
    db.prepare(
      'INSERT INTO sessions(id, workspace_id, title, auto_title, created_at, updated_at) VALUES(?, ?, ?, ?, ?, ?)'
    ).run(newId, src.workspace_id, title, autoTitle, now, now)

    const messages = db
      .prepare('SELECT id, role, content, tools, created_at, stopped FROM messages WHERE session_id = ? ORDER BY created_at ASC')
      .all(id) as unknown as MessageRow[]
    const insert = db.prepare(
      'INSERT INTO messages(id, session_id, role, content, tools, created_at) VALUES(?, ?, ?, ?, ?, ?)'
    )
    for (const m of messages) {
      insert.run(randomUUID(), newId, m.role, m.content, m.tools, m.created_at)
    }
    db.exec('COMMIT')
  } catch (e) {
    // SQLITE_FULL 等错误会让 SQLite 自动回滚，此时 ROLLBACK 会抛
    // "cannot rollback - no transaction is active"，把真实原因盖掉
    try {
      db.exec('ROLLBACK')
    } catch {
      // 事务已不存在
    }
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

/** 工具调用以 JSON 存在列里；损坏时降级为空，不让整条消息读不出来 */
function parseTools(raw: string | null): ChatMessage['tools'] {
  if (!raw) return undefined
  try {
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? (parsed as ChatMessage['tools']) : undefined
  } catch {
    return undefined
  }
}

/** 读取某会话的全部消息 */
export function listMessages(sessionId: string, limit?: number): ChatMessage[] {
  const db = getDb()

  // 必须在校验后使用：负数会走 else 分支拼进 SQL 变成"取全部"，
  // 小数/NaN 会让 SQLite 报 datatype mismatch 并把内部错误抛到界面
  const n = typeof limit === 'number' && Number.isFinite(limit) ? Math.floor(limit) : 0

  if (n <= 0) {
    const rows = db
      .prepare(
        'SELECT id, role, content, tools, created_at, stopped FROM messages WHERE session_id = ? ORDER BY created_at ASC'
      )
      .all(sessionId) as unknown as MessageRow[]
    return rows.map(toChatMessage)
  }

  // 分页：先取最近 N 条，再反转为时间正序
  const rows = db
    .prepare(
      `SELECT id, role, content, tools, created_at, stopped FROM messages
        WHERE session_id = ? ORDER BY created_at DESC LIMIT ?`
    )
    .all(sessionId, n) as unknown as MessageRow[]
  return rows.reverse().map(toChatMessage)
}

function toChatMessage(r: MessageRow): ChatMessage {
  return {
    id: r.id,
    role: r.role === 'assistant' ? 'assistant' : 'user',
    text: r.content,
    tools: parseTools(r.tools),
    timestamp: r.created_at,
    stopped: r.stopped === 1
  }
}

/** 会话消息总数，用于判断是否还有更早的历史 */
export function countMessages(sessionId: string): number {
  const row = getDb()
    .prepare('SELECT COUNT(*) AS n FROM messages WHERE session_id = ?')
    .get(sessionId) as { n: number }
  return row.n
}

/** 追加一条消息；首条用户消息会顺带补上自动标题 */
export function appendMessage(
  sessionId: string,
  role: 'user' | 'assistant',
  content: string,
  tools?: ChatMessage['tools'],
  stopped = false
): ChatMessage {
  const db = getDb()
  const now = new Date().toISOString()
  const id = randomUUID()

  // 会话已被软删除时不再写入。
  // 这里必须抛错而不是返回一个假对象：返回假对象会让上层以为写入成功，
  // 界面上消息看着发出去了，刷新后却消失，用户完全不知道发生了什么。
  const target = db
    .prepare('SELECT deleted_at FROM sessions WHERE id = ?')
    .get(sessionId) as { deleted_at?: string | null } | undefined
  if (!target) throw new Error('会话不存在')
  if (target.deleted_at) throw new Error('会话已被删除，消息未保存')

  db.prepare(
    'INSERT INTO messages(id, session_id, role, content, tools, created_at, stopped) VALUES(?, ?, ?, ?, ?, ?, ?)'
  ).run(id, sessionId, role, content, tools ? JSON.stringify(tools) : null, now, stopped ? 1 : 0)

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

  return { id, role, text: content, tools, timestamp: now, stopped }
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

