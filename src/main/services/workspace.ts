import { basename, resolve } from 'node:path'
import { randomUUID } from 'node:crypto'
import { stat } from 'node:fs/promises'
import type { Workspace } from '../../shared/types'
import { getDb } from '../db'

interface WorkspaceRow {
  id: string
  path: string
  name: string
  created_at: string
  pinned_at: string | null
}

function toWorkspace(row: WorkspaceRow): Workspace {
  return {
    id: row.id,
    path: row.path,
    name: row.name,
    addedAt: row.created_at,
    pinnedAt: row.pinned_at ?? null
  }
}

async function isDirectory(p: string): Promise<boolean> {
  try {
    return (await stat(p)).isDirectory()
  } catch {
    return false
  }
}

export function listWorkspaces(): Workspace[] {
  const rows = getDb()
    // 置顶的排前面，其余按创建时间
    .prepare(
      `SELECT id, path, name, created_at, pinned_at FROM workspaces
        ORDER BY pinned_at IS NULL, pinned_at DESC, created_at ASC`
    )
    .all() as unknown as WorkspaceRow[]
  return rows.map(toWorkspace)
}

export async function addWorkspace(
  dir: string | null | undefined,
  customName?: string
): Promise<Workspace[]> {
  if (typeof dir !== 'string' || !dir.trim()) {
    throw new Error('请选择一个目录')
  }
  const absolute = resolve(dir)
  if (!(await isDirectory(absolute))) {
    throw new Error(`目录不存在或不可读：${absolute}`)
  }

  const db = getDb()
  // Windows 与 macOS 的路径大小写不敏感，精确比较会让同一目录被加两次：
  // 会话被劈成两半、Skill 扫描重复。SQLite 的 UNIQUE 索引是二进制比较，
  // 兜不住这种情况，只能在这里按不敏感做预查。
  const caseInsensitive = process.platform === 'win32' || process.platform === 'darwin'
  const existing = caseInsensitive
    ? db.prepare('SELECT id FROM workspaces WHERE path = ? COLLATE NOCASE').get(absolute)
    : db.prepare('SELECT id FROM workspaces WHERE path = ?').get(absolute)
  if (existing) throw new Error('该工作区已存在')

  const name = (customName ?? '').trim() || basename(absolute)
  db.prepare('INSERT INTO workspaces(id, path, name, created_at) VALUES(?, ?, ?, ?)').run(
    randomUUID(),
    absolute,
    name,
    new Date().toISOString()
  )

  return listWorkspaces()
}

/**
 * 移除工作区：数据库里级联删除其会话与消息。
 * 不触碰磁盘目录。
 */
export function removeWorkspace(id: string): Workspace[] {
  getDb().prepare('DELETE FROM workspaces WHERE id = ?').run(id)
  return listWorkspaces()
}

/** 重命名工作区（只改展示名，不动磁盘目录） */
export function renameWorkspace(id: string, name: string): Workspace[] {
  const trimmed = name.trim()
  if (!trimmed) throw new Error('名称不能为空')
  getDb().prepare('UPDATE workspaces SET name = ? WHERE id = ?').run(trimmed, id)
  return listWorkspaces()
}

/** 置顶 / 取消置顶工作区 */
export function setWorkspacePinned(id: string, pinned: boolean): Workspace[] {
  const db = getDb()
  const exists = db.prepare('SELECT id FROM workspaces WHERE id = ?').get(id)
  if (!exists) throw new Error('工作区不存在')
  db.prepare('UPDATE workspaces SET pinned_at = ? WHERE id = ?').run(
    pinned ? new Date().toISOString() : null,
    id
  )
  return listWorkspaces()
}

/** 读取界面主题偏好 */
export function getTheme(): 'light' | 'dark' {
  const row = getDb().prepare("SELECT value FROM meta WHERE key = 'theme'").get() as
    | { value?: string }
    | undefined
  return row?.value === 'dark' ? 'dark' : 'light'
}

export function setTheme(theme: 'light' | 'dark'): void {
  getDb()
    .prepare(
      "INSERT INTO meta(key, value) VALUES('theme', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
    )
    .run(theme)
}

/**
 * 解析新建会话应归属的工作区。
 *
 * 现在会话一律在指定工作区下创建，不再有"默认工作区"的概念，
 * 未指定时直接报错，避免用户以为建成功了。
 */
export function resolveWorkspaceId(requested: string | null): string | null {
  return requested
}
