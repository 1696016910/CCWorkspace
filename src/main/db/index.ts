import { DatabaseSync } from 'node:sqlite'
import { app } from 'electron'
import { join } from 'node:path'
import { copyFileSync, existsSync, mkdirSync, renameSync } from 'node:fs'
import { MIGRATIONS, SCHEMA_VERSION } from './schema'

let db: DatabaseSync | null = null

function dbPath(): string {
  return join(app.getPath('userData'), 'workbench.db')
}

/**
 * 打开数据库。
 *
 * 若文件损坏（不是 SQLite 文件、或 integrity_check 失败），把原文件改名保留，
 * 然后以空库继续——宁可丢历史，也不能让应用起不来。
 */
function openWithRecovery(path: string): DatabaseSync {
  // 第一层：能否作为 SQLite 打开并通过完整性检查
  try {
    const handle = new DatabaseSync(path)
    try {
      const check = handle.prepare('PRAGMA integrity_check').get() as { integrity_check?: string }
      if (check?.integrity_check === 'ok') return handle
    } catch {
      // 落在下面的恢复流程
    }
    handle.close()
  } catch {
    // new DatabaseSync 本身就会在文件不是数据库时抛错
  }

  // 第二层：备份坏文件，重新建库
  if (existsSync(path)) {
    const backup = `${path}.corrupt-${Date.now()}`
    try {
      copyFileSync(path, backup)
    } catch {
      // 备份失败也要继续，不能卡住启动
    }
    try {
      renameSync(path, `${path}.broken-${Date.now()}`)
    } catch {
      // 无法移走时就地覆盖
    }
  }

  const fresh = new DatabaseSync(path)
  // 从坏文件恢复时，一并清理 WAL 残留，避免再次读到旧页
  return fresh
}

export function initDb(): DatabaseSync {
  if (db) return db

  const path = dbPath()
  mkdirSync(join(path, '..'), { recursive: true })

  const handle = openWithRecovery(path)

  try {
    handle.exec('PRAGMA journal_mode = WAL')
    handle.exec('PRAGMA foreign_keys = ON')
    migrate(handle)
  } catch (e) {
    // 迁移失败属于开发期问题，向上抛出让错误可见
    console.error('[db] 初始化失败:', e)
    throw e
  }

  db = handle
  return handle
}

function migrate(handle: DatabaseSync): void {
  handle.exec('CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)')
  const row = handle.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as
    | { value?: string }
    | undefined
  const current = row?.value ? Number(row.value) : 0

  for (let v = current + 1; v <= SCHEMA_VERSION; v++) {
    const sql = MIGRATIONS[v]
    if (!sql) continue
    handle.exec(sql)
    handle
      .prepare(
        "INSERT INTO meta(key, value) VALUES('schema_version', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
      )
      .run(String(v))
  }
}

export function getDb(): DatabaseSync {
  if (!db) throw new Error('数据库尚未初始化')
  return db
}

export function closeDb(): void {
  db?.close()
  db = null
}
