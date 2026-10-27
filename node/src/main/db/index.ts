import { DatabaseSync } from 'node:sqlite'
import { app } from 'electron'
import { join } from 'node:path'
import { copyFileSync, existsSync, mkdirSync, renameSync, unlinkSync } from 'node:fs'
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
    } catch (e) {
      // 锁竞争不是损坏。SQLITE_BUSY(5) / SQLITE_LOCKED(6) 表示另一个进程
      // 正在写，稍后重试即可；把它当成损坏会走"备份+重建空库"，
      // 用户还会被提示删掉 workbench.db —— 等于把好数据删了。
      const code = (e as { errcode?: number }).errcode
      if (code === 5 || code === 6 || /database is locked/i.test(String(e))) {
        handle.close()
        throw new Error('数据库正被另一个进程使用，请关闭其他实例后重试')
      }
      // 其余异常（malformed、not a database）才是真的损坏
    }
    handle.close()
  } catch (e) {
    // 上面主动抛出的锁错误要原样传出去，不能再当成损坏
    if (e instanceof Error && e.message.includes('正被另一个进程使用')) throw e
    // new DatabaseSync 本身就会在文件不是数据库时抛错
  }

  // 第二层：备份坏文件，重新建库
  //
  // WAL 模式下崩溃前已提交的数据全在 -wal 里，只备份主库会得到一个
  // 半截快照，用户几乎无法自行还原。三个文件必须当成一个整体处理。
  const stamp = Date.now()
  const sidecars = ['-wal', '-shm']
  if (existsSync(path)) {
    try {
      copyFileSync(path, `${path}.corrupt-${stamp}`)
      for (const suffix of sidecars) {
        if (existsSync(path + suffix)) {
          copyFileSync(path + suffix, `${path}.corrupt-${stamp}${suffix}`)
        }
      }
    } catch {
      // 备份失败也要继续，不能卡住启动
    }
    try {
      renameSync(path, `${path}.broken-${stamp}`)
      // 主库移走后必须连带移走 WAL，否则新库会去读旧页
      for (const suffix of sidecars) {
        if (existsSync(path + suffix)) {
          try {
            renameSync(path + suffix, `${path}.broken-${stamp}${suffix}`)
          } catch {
            // 移不走就尽量删掉，避免污染新库
            try {
              unlinkSync(path + suffix)
            } catch {
              // 忽略
            }
          }
        }
      }
    } catch {
      // 无法移走时就地覆盖
    }
  }

  const fresh = new DatabaseSync(path)
  return fresh
}

export function initDb(): DatabaseSync {
  if (db) return db

  const path = dbPath()
  mkdirSync(join(path, '..'), { recursive: true })

  const handle = openWithRecovery(path)

  try {
    handle.exec('PRAGMA journal_mode = WAL')
    // WAL 与单实例都只是加速与防并发，真正靠它保证级联删除生效
    handle.exec('PRAGMA foreign_keys = ON')
    // 另一个进程正在写时等一会儿再报错，而不是立刻失败
    handle.exec('PRAGMA busy_timeout = 5000')
    migrate(handle)
  } catch (e) {
    // 不关闭句柄的话文件会一直被占用，用户连"删掉 workbench.db 重试"都做不到
    try {
      handle.close()
    } catch {
      // 关不掉也要继续往上抛原始错误
    }
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
  // 版本号被写成非数字（手改或将来降级写脏值）时，NaN 会让下面的循环一次都不跑，
  // 库表根本不建，后续每个 IPC 都报 no such table。
  const parsed = Number(row?.value)
  const current = Number.isFinite(parsed) && parsed >= 0 ? parsed : 0

  for (let v = current + 1; v <= SCHEMA_VERSION; v++) {
    const sql = MIGRATIONS[v]
    if (!sql) continue
    // 单个迁移必须原子：多语句迁移中途失败会留下"列已加但版本没升"的残局，
    // 下次启动重跑就撞 duplicate column，应用再也起不来。
    handle.exec('BEGIN')
    try {
      handle.exec(sql)
      handle
        .prepare(
          "INSERT INTO meta(key, value) VALUES('schema_version', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
        )
        .run(String(v))
      handle.exec('COMMIT')
    } catch (e) {
      // 磁盘满等错误会让 SQLite 自动回滚，此时再 ROLLBACK 会抛
      // "cannot rollback - no transaction is active"，把真正的错误掩盖掉
      try {
        handle.exec('ROLLBACK')
      } catch {
        // 事务已不存在，忽略
      }
      // 0.1.1 发布的版本没有事务保护，可能留下"列已加、版本号还是 1"的库。
      // 重跑必然撞 duplicate column，这里把这种情况视为该迁移已完成。
      if (isAlreadyApplied(e)) {
        try {
          handle
            .prepare(
              "INSERT INTO meta(key, value) VALUES('schema_version', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value"
            )
            .run(String(v))
          continue
        } catch {
          // 补版本号也失败就按原错误抛出
        }
      }
      throw e
    }
  }
}

/**
 * 判断迁移失败是否只是因为"这一版已经生效过"。
 *
 * 老版本（0.1.1）执行多语句迁移时没有事务，中途失败会留下部分生效的库；
 * 重跑就会撞 duplicate column。这种情况应当跳过而不是让应用永久起不来。
 */
function isAlreadyApplied(e: unknown): boolean {
  const msg = e instanceof Error ? e.message : String(e)
  return /duplicate column name/i.test(msg)
}

export function getDb(): DatabaseSync {
  if (!db) throw new Error('数据库尚未初始化')
  return db
}

export function closeDb(): void {
  db?.close()
  db = null
}
