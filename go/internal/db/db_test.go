package db

import (
	"database/sql"
	"os"
	"path/filepath"
	"testing"

	_ "modernc.org/sqlite"
)

// newNodeSchema 用 Node 版的原始 SQL 建一个库，验证 Go 实现能读它。
func newNodeSchema(t *testing.T, path string) {
	t.Helper()
	d, err := sql.Open("sqlite", path)
	if err != nil {
		t.Fatal(err)
	}
	defer d.Close()

	stmts := []string{
		`CREATE TABLE workspaces (
			id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE,
			name TEXT NOT NULL, created_at TEXT NOT NULL)`,
		`CREATE TABLE sessions (
			id TEXT PRIMARY KEY,
			workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
			title TEXT NOT NULL DEFAULT '', auto_title INTEGER NOT NULL DEFAULT 1,
			created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT)`,
		`CREATE TABLE messages (
			id TEXT PRIMARY KEY,
			session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
			role TEXT NOT NULL, content TEXT NOT NULL, tools TEXT,
			created_at TEXT NOT NULL)`,
		`CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)`,
		// 迁移 2
		`ALTER TABLE sessions ADD COLUMN claude_session_id TEXT`,
		`ALTER TABLE sessions ADD COLUMN context_tokens INTEGER NOT NULL DEFAULT 0`,
		`ALTER TABLE sessions ADD COLUMN context_max INTEGER NOT NULL DEFAULT 0`,
		`ALTER TABLE sessions ADD COLUMN context_percent REAL NOT NULL DEFAULT 0`,
		// 迁移 3
		`ALTER TABLE workspaces ADD COLUMN pinned_at TEXT`,
		`ALTER TABLE sessions ADD COLUMN pinned_at TEXT`,
		// 迁移 4
		`ALTER TABLE messages ADD COLUMN stopped INTEGER NOT NULL DEFAULT 0`,
		`INSERT INTO meta(key,value) VALUES('schema_version','4')`,
	}
	for _, s := range stmts {
		if _, err := d.Exec(s); err != nil {
			t.Fatalf("建表失败 %q: %v", s[:40], err)
		}
	}
}

func openAt(t *testing.T, path string) (*sql.DB, error) {
	t.Helper()
	d, err := sql.Open("sqlite", path)
	if err != nil {
		return nil, err
	}
	d.SetMaxOpenConns(1)
	for _, p := range []string{
		"PRAGMA journal_mode = WAL",
		"PRAGMA foreign_keys = ON",
		"PRAGMA busy_timeout = 5000",
	} {
		if _, err := d.Exec(p); err != nil {
			d.Close()
			return nil, err
		}
	}
	var integrity string
	if err := d.QueryRow("PRAGMA integrity_check").Scan(&integrity); err != nil {
		d.Close()
		return nil, err
	}
	if integrity != "ok" {
		d.Close()
		return nil, errIntegrity(integrity)
	}
	v, err := schemaVersion(d)
	if err != nil {
		d.Close()
		return nil, err
	}
	if v < SchemaVersion {
		d.Close()
		return nil, errOldVersion(v)
	}
	return d, nil
}

type errIntegrity string

func (e errIntegrity) Error() string { return "integrity: " + string(e) }

type errOldVersion int

func (e errOldVersion) Error() string { return "old version" }

func TestReadsNodeSchema(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "workbench.db")
	newNodeSchema(t, p)

	d, err := openAt(t, p)
	if err != nil {
		t.Fatalf("应能读取 Node 建的库，实际报错: %v", err)
	}
	defer d.Close()

	// 三张表的列必须齐全
	want := map[string][]string{
		"workspaces": {"id", "path", "name", "created_at", "pinned_at"},
		"sessions": {"id", "workspace_id", "title", "auto_title", "created_at", "updated_at",
			"deleted_at", "claude_session_id", "context_tokens", "context_max",
			"context_percent", "pinned_at"},
		"messages": {"id", "session_id", "role", "content", "tools", "created_at", "stopped"},
	}
	for table, cols := range want {
		got := columnsOf(t, d, table)
		for _, c := range cols {
			if !got[c] {
				t.Errorf("表 %s 缺列 %s（实际有 %v）", table, c, keys(got))
			}
		}
	}
}

func TestRejectsOldVersion(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "old.db")
	newNodeSchema(t, p)

	d, _ := sql.Open("sqlite", p)
	d.Exec("UPDATE meta SET value='2' WHERE key='schema_version'")
	d.Close()

	if _, err := openAt(t, p); err == nil {
		t.Fatal("版本过旧时应报错，实际通过了")
	}
}

func TestRejectsCorrupt(t *testing.T) {
	dir := t.TempDir()
	p := filepath.Join(dir, "corrupt.db")
	os.WriteFile(p, []byte("this is not a sqlite file at all, just garbage bytes"), 0o644)

	if _, err := openAt(t, p); err == nil {
		t.Fatal("损坏文件应报错，实际通过了")
	}
}

func columnsOf(t *testing.T, d *sql.DB, table string) map[string]bool {
	t.Helper()
	rows, err := d.Query("PRAGMA table_info(" + table + ")")
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	out := map[string]bool{}
	for rows.Next() {
		var cid int
		var name, ctype string
		var notnull, pk int
		var dflt any
		if err := rows.Scan(&cid, &name, &ctype, &notnull, &dflt, &pk); err != nil {
			t.Fatal(err)
		}
		out[name] = true
	}
	return out
}

func keys(m map[string]bool) []string {
	out := make([]string, 0, len(m))
	for k := range m {
		out = append(out, k)
	}
	return out
}
