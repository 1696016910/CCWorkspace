import { describe, expect, it } from 'vitest'
import { DatabaseSync } from 'node:sqlite'
import { MIGRATIONS, SCHEMA_VERSION } from '../src/main/db/schema'

/**
 * 迁移必须原子执行。
 *
 * 如果多语句迁移中途失败而没有事务，会留下"部分列已加、版本号没升"的残局，
 * 下次启动重跑同一条迁移就会撞 duplicate column，应用永久无法启动。
 */
function runMigrations(db: DatabaseSync, versions: number[]): void {
  db.exec('CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)')
  const row = db.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as
    | { value?: string }
    | undefined
  const current = row?.value ? Number(row.value) : 0

  for (let v = current + 1; v <= SCHEMA_VERSION; v++) {
    if (!versions.includes(v)) continue
    const sql = MIGRATIONS[v]
    if (!sql) continue
    db.exec('BEGIN')
    try {
      db.exec(sql)
      db.prepare(
        "INSERT INTO meta(key, value) VALUES('schema_version', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
      ).run(String(v))
      db.exec('COMMIT')
    } catch (e) {
      db.exec('ROLLBACK')
      throw e
    }
  }
}

describe('数据库迁移', () => {
  it('从空库一次跑到最新版本', () => {
    const db = new DatabaseSync(':memory:')
    const allVersions = Array.from({ length: SCHEMA_VERSION }, (_, i) => i + 1)
    runMigrations(db, allVersions)
    const v = db.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as {
      value: string
    }
    expect(v.value).toBe(String(SCHEMA_VERSION))

    // 迁移 2 的列都应存在
    const cols = (db.prepare('PRAGMA table_info(sessions)').all() as { name: string }[]).map(
      (c) => c.name
    )
    expect(cols).toContain('claude_session_id')
    expect(cols).toContain('context_tokens')
    expect(cols).toContain('context_percent')
    db.close()
  })

  it('迁移失败时回滚，不会留下半成品', () => {
    const db = new DatabaseSync(':memory:')
    runMigrations(db, [1])

    // 故意让迁移 2 的中途失败：先手工加一列，再跑迁移 2
    db.exec('ALTER TABLE sessions ADD COLUMN claude_session_id TEXT')
    expect(() => runMigrations(db, [2])).toThrow()

    // 关键：失败后版本号仍是 1，且已加的列被回滚掉，可以安全重跑
    const v = db.prepare("SELECT value FROM meta WHERE key = 'schema_version'").get() as {
      value: string
    }
    expect(v.value).toBe('1')

    const cols = (db.prepare('PRAGMA table_info(sessions)').all() as { name: string }[]).map(
      (c) => c.name
    )
    expect(cols).not.toContain('context_tokens')
    db.close()
  })

  it('重复调用是幂等的', () => {
    const db = new DatabaseSync(':memory:')
    const allVersions = Array.from({ length: SCHEMA_VERSION }, (_, i) => i + 1)
    runMigrations(db, allVersions)
    expect(() => runMigrations(db, allVersions)).not.toThrow()
    db.close()
  })
})
