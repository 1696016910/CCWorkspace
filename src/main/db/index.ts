import { DatabaseSync } from 'node:sqlite'
import { app } from 'electron'
import { join } from 'node:path'
import { copyFileSync, existsSync, mkdirSync } from 'node:fs'
import { MIGRATIONS, SCHEMA_VERSION } from './schema'

let db: DatabaseSync | null = null

function dbPath(): string {
  return join(app.getPath('userData'), 'workbench.db')
}

/**
 * 打开数据库并补齐 schema。
 *
 * 若文件损坏（integrity_check 失败），把原文件改名保留，然后以空库继续，
 * 避免应用因一个坏文件完全无法启动。
 */
export function initDb(): DatabaseSync {
  if (db) return db

  const path = dbPath()
  mkdirSync(join(path, '..'), { recursive: true })

  let handle = new DatabaseSync(path)

  try {
    const check = handle.prepare('PRAGMA integrity_check').get() as { integrity_check?: string }
    if (check?.integrity_check !== 'ok') throw new Error('integrity check failed')
  } catch {
    handle.close()
    const backup = `${path}.corrupt-${Date.now()}`
    if (existsSync(path)) copyFileSync(path, backup)
    handle = new DatabaseSync(path)
  }

  handle.exec('PRAGMA journal_mode = WAL')
  handle.exec('PRAGMA foreign_keys = ON')

  migrate(handle)
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
      .prepare("INSERT INTO meta(key, value) VALUES('schema_version', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
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
