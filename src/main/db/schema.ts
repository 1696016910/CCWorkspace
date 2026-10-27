/**
 * 数据库 schema 与迁移。
 *
 * 版本号写入 meta.schema_version；启动时按版本顺序补齐。
 * 迁移只增不改——已发布的版本不再修改，避免用户库结构漂移。
 */

export const SCHEMA_VERSION = 4

export const MIGRATIONS: Record<number, string> = {
  1: `
    CREATE TABLE IF NOT EXISTS workspaces (
      id          TEXT PRIMARY KEY,
      path        TEXT NOT NULL UNIQUE,
      name        TEXT NOT NULL,
      created_at  TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS sessions (
      id           TEXT PRIMARY KEY,
      workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
      title        TEXT NOT NULL DEFAULT '',
      auto_title   INTEGER NOT NULL DEFAULT 1,
      created_at   TEXT NOT NULL,
      updated_at   TEXT NOT NULL,
      deleted_at   TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_sessions_workspace
      ON sessions(workspace_id, updated_at DESC);

    CREATE TABLE IF NOT EXISTS messages (
      id         TEXT PRIMARY KEY,
      session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
      role       TEXT NOT NULL,
      content    TEXT NOT NULL,
      tools      TEXT,
      created_at TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_messages_session
      ON messages(session_id, created_at);

    CREATE TABLE IF NOT EXISTS meta (
      key   TEXT PRIMARY KEY,
      value TEXT NOT NULL
    );
  `,

  /**
   * 记录每个会话对应的 Claude CLI 会话 id。
   * SDK 的 resume 需要它来续接上下文，否则每轮都是全新的对话。
   */
  2: `
    ALTER TABLE sessions ADD COLUMN claude_session_id TEXT;
    ALTER TABLE sessions ADD COLUMN context_tokens INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE sessions ADD COLUMN context_max INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE sessions ADD COLUMN context_percent REAL NOT NULL DEFAULT 0;
  `,

  /**
   * 置顶：值为置顶时间，NULL 表示未置顶。
   * 排序时置顶项优先，未置顶按原有顺序。
   */
  3: `
    ALTER TABLE workspaces ADD COLUMN pinned_at TEXT;
    ALTER TABLE sessions ADD COLUMN pinned_at TEXT;
  `,

  /**
   * 记录该条助手消息是否被用户手动中断。
   * 中断后的半截内容仍然保存，但界面要显示「已停止生成」。
   */
  4: `
    ALTER TABLE messages ADD COLUMN stopped INTEGER NOT NULL DEFAULT 0;
  `
}
