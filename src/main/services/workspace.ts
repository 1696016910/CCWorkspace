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
}

function toWorkspace(row: WorkspaceRow): Workspace {
  return { id: row.id, path: row.path, name: row.name, addedAt: row.created_at }
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
    .prepare('SELECT id, path, name, created_at FROM workspaces ORDER BY created_at ASC')
    .all() as unknown as WorkspaceRow[]
  return rows.map(toWorkspace)
}

export async function addWorkspace(dir: string): Promise<Workspace[]> {
  const absolute = resolve(dir)
  if (!(await isDirectory(absolute))) {
    throw new Error(`目录不存在或不可读：${absolute}`)
  }

  const db = getDb()
  const existing = db.prepare('SELECT id FROM workspaces WHERE path = ?').get(absolute)
  if (existing) throw new Error('该工作区已存在')

  db.prepare('INSERT INTO workspaces(id, path, name, created_at) VALUES(?, ?, ?, ?)').run(
    randomUUID(),
    absolute,
    basename(absolute),
    new Date().toISOString()
  )

  return listWorkspaces()
}

/**
 * 移除工作区：数据库里级联删除其会话与消息。
 * 不触碰磁盘目录。默认工作区若被移除，由调用方重新选定。
 */
export function removeWorkspace(id: string): Workspace[] {
  getDb().prepare('DELETE FROM workspaces WHERE id = ?').run(id)
  return listWorkspaces()
}

/** 默认工作区记在 meta 表，持久化在同一个库里 */
export function getDefaultWorkspaceId(): string | null {
  const row = getDb()
    .prepare("SELECT value FROM meta WHERE key = 'default_workspace_id'")
    .get() as { value?: string } | undefined
  return row?.value ?? null
}

export function setDefaultWorkspaceId(id: string | null): void {
  const db = getDb()
  if (id === null) {
    db.prepare("DELETE FROM meta WHERE key = 'default_workspace_id'").run()
    return
  }
  const exists = db.prepare('SELECT id FROM workspaces WHERE id = ?').get(id)
  if (!exists) throw new Error('指定的工作区不存在')
  db.prepare(
    "INSERT INTO meta(key, value) VALUES('default_workspace_id', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
  ).run(id)
}

/** 重命名工作区（只改展示名，不动磁盘目录） */
export function renameWorkspace(id: string, name: string): Workspace[] {
  const trimmed = name.trim()
  if (!trimmed) throw new Error('名称不能为空')
  getDb().prepare('UPDATE workspaces SET name = ? WHERE id = ?').run(trimmed, id)
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

/** 解析新建会话应归属的工作区：显式指定 → 默认 → 第一个 */
export function resolveWorkspaceId(requested: string | null): string | null {
  if (requested) return requested
  const def = getDefaultWorkspaceId()
  if (def) return def
  const first = getDb().prepare('SELECT id FROM workspaces ORDER BY created_at ASC LIMIT 1').get() as
    | { id: string }
    | undefined
  return first?.id ?? null
}
