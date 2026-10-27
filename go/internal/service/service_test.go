package service

import (
	"claude-workbench/internal/db"
	"database/sql"
	"os"
	"path/filepath"
	"strings"
	"testing"

	_ "modernc.org/sqlite"
)

// setup 建一个临时库并把 db.Get() 指向它。
func setup(t *testing.T) {
	t.Helper()
	dir := t.TempDir()
	p := filepath.Join(dir, "test.db")

	d, err := sql.Open("sqlite", p)
	if err != nil {
		t.Fatal(err)
	}
	d.SetMaxOpenConns(1)
	d.Exec("PRAGMA foreign_keys = ON")

	stmts := []string{
		`CREATE TABLE workspaces (id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE,
			name TEXT NOT NULL, created_at TEXT NOT NULL)`,
		`CREATE TABLE sessions (id TEXT PRIMARY KEY,
			workspace_id TEXT NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
			title TEXT NOT NULL DEFAULT '', auto_title INTEGER NOT NULL DEFAULT 1,
			created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT)`,
		`CREATE TABLE messages (id TEXT PRIMARY KEY,
			session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
			role TEXT NOT NULL, content TEXT NOT NULL, tools TEXT, created_at TEXT NOT NULL)`,
		`CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)`,
		`ALTER TABLE sessions ADD COLUMN claude_session_id TEXT`,
		`ALTER TABLE sessions ADD COLUMN context_tokens INTEGER NOT NULL DEFAULT 0`,
		`ALTER TABLE sessions ADD COLUMN context_max INTEGER NOT NULL DEFAULT 0`,
		`ALTER TABLE sessions ADD COLUMN context_percent REAL NOT NULL DEFAULT 0`,
		`ALTER TABLE workspaces ADD COLUMN pinned_at TEXT`,
		`ALTER TABLE sessions ADD COLUMN pinned_at TEXT`,
		`ALTER TABLE messages ADD COLUMN stopped INTEGER NOT NULL DEFAULT 0`,
	}
	for _, s := range stmts {
		if _, err := d.Exec(s); err != nil {
			t.Fatalf("建表失败: %v", err)
		}
	}
	db.SetForTest(d)
	t.Cleanup(func() { d.Close() })
}

func mustWorkspace(t *testing.T, name string) string {
	t.Helper()
	dir := filepath.Join(t.TempDir(), name)
	os.MkdirAll(dir, 0o755)
	list, err := AddWorkspace(dir, name)
	if err != nil {
		t.Fatal(err)
	}
	return list[len(list)-1].ID
}

func TestWorkspaceCRUD(t *testing.T) {
	setup(t)
	id := mustWorkspace(t, "proj-a")

	list, _ := ListWorkspaces()
	if len(list) != 1 || list[0].ID != id {
		t.Fatalf("应有 1 个工作区，实际 %d", len(list))
	}

	list, err := RenameWorkspace(id, "改名后")
	if err != nil {
		t.Fatal(err)
	}
	if list[0].Name != "改名后" {
		t.Errorf("重命名失败: %s", list[0].Name)
	}

	list, _ = PinWorkspace(id, true)
	if list[0].PinnedAt == nil {
		t.Error("置顶后 PinnedAt 应非空")
	}
	list, _ = PinWorkspace(id, false)
	if list[0].PinnedAt != nil {
		t.Error("取消置顶后 PinnedAt 应为空")
	}

	if _, err := RemoveWorkspace(id); err != nil {
		t.Fatal(err)
	}
	list, _ = ListWorkspaces()
	if len(list) != 0 {
		t.Errorf("删除后应为空，实际 %d", len(list))
	}
}

func TestDuplicateWorkspacePathRejected(t *testing.T) {
	setup(t)
	dir := filepath.Join(t.TempDir(), "same")
	os.MkdirAll(dir, 0o755)
	if _, err := AddWorkspace(dir, ""); err != nil {
		t.Fatal(err)
	}
	if _, err := AddWorkspace(dir, ""); err == nil {
		t.Fatal("重复路径应被拒绝")
	} else if !strings.Contains(err.Error(), "已存在") {
		t.Errorf("错误信息应说明已存在: %v", err)
	}
}

func TestSessionLifecycle(t *testing.T) {
	setup(t)
	wsID := mustWorkspace(t, "p")

	s, err := CreateSession(wsID)
	if err != nil {
		t.Fatal(err)
	}

	// 软删除 → 列表里消失，回收站里有
	if err := DeleteSession(s.ID); err != nil {
		t.Fatal(err)
	}
	live, _ := ListSessions()
	if len(live) != 0 {
		t.Errorf("删除后列表应为空，实际 %d", len(live))
	}
	trash, _ := ListDeletedSessions()
	if len(trash) != 1 {
		t.Errorf("回收站应有 1 条，实际 %d", len(trash))
	}

	// 恢复
	if err := RestoreSession(s.ID); err != nil {
		t.Fatal(err)
	}
	live, _ = ListSessions()
	if len(live) != 1 {
		t.Errorf("恢复后应有 1 条，实际 %d", len(live))
	}

	// 彻底删除
	if err := PurgeSession(s.ID); err != nil {
		t.Fatal(err)
	}
	trash, _ = ListDeletedSessions()
	if len(trash) != 0 {
		t.Errorf("彻底删除后回收站应为空")
	}
}

func TestCreateSessionRequiresWorkspace(t *testing.T) {
	setup(t)
	if _, err := CreateSession(""); err == nil {
		t.Fatal("未指定工作区应报错")
	} else if !strings.Contains(err.Error(), "工作区") {
		t.Errorf("错误应提到工作区: %v", err)
	}
	if _, err := CreateSession("不存在的id"); err == nil {
		t.Fatal("不存在的工作区应报错")
	}
}

func TestDeriveTitle(t *testing.T) {
	cases := []struct{ in, want string }{
		{"把订单接口改成幂等", "把订单接口改成幂等"},
		{"第一行\n\n第二行", "第一行 第二行"},
		{"   ", "(新会话)"},
	}
	for _, c := range cases {
		if got := deriveTitle(c.in); got != c.want {
			t.Errorf("deriveTitle(%q) = %q，期望 %q", c.in, got, c.want)
		}
	}

	// 超长中文按码点截断，不切出半个字符
	long := deriveTitle(strings.Repeat("一", 60))
	if len([]rune(long)) != titleMax+1 { // 40 + 省略号
		t.Errorf("超长标题应为 %d 个码点，实际 %d", titleMax+1, len([]rune(long)))
	}
	if !strings.HasSuffix(long, "…") {
		t.Error("超长标题应以省略号结尾")
	}

	// emoji 不能被切一半
	emoji := deriveTitle(strings.Repeat("a", 39) + "😀😀😀")
	for _, r := range emoji {
		if r == 0xFFFD {
			t.Error("标题出现替换字符，说明代理对被切开")
		}
	}
}

func TestDuplicateSession(t *testing.T) {
	setup(t)
	wsID := mustWorkspace(t, "p")
	s, _ := CreateSession(wsID)
	AppendMessage(s.ID, "user", "原始消息", nil, false)

	dup, err := DuplicateSession(s.ID)
	if err != nil {
		t.Fatal(err)
	}
	if dup.ID == s.ID {
		t.Fatal("副本 id 应不同")
	}
	msgs, _ := ListMessages(dup.ID, 0)
	if len(msgs) != 1 || msgs[0].Text != "原始消息" {
		t.Errorf("副本应含原消息，实际 %d 条", len(msgs))
	}
}

func TestAppendMessageRejectsDeletedSession(t *testing.T) {
	setup(t)
	wsID := mustWorkspace(t, "p")
	s, _ := CreateSession(wsID)
	DeleteSession(s.ID)

	_, err := AppendMessage(s.ID, "user", "x", nil, false)
	if err == nil {
		t.Fatal("已删除会话写入消息应报错")
	}
	if !strings.Contains(err.Error(), "已被删除") {
		t.Errorf("错误信息应说明会话已删除: %v", err)
	}
}

func TestStoppedFlagPersisted(t *testing.T) {
	setup(t)
	wsID := mustWorkspace(t, "p")
	s, _ := CreateSession(wsID)

	AppendMessage(s.ID, "user", "问", nil, false)
	AppendMessage(s.ID, "assistant", "半截回复", nil, true)

	msgs, _ := ListMessages(s.ID, 0)
	if len(msgs) != 2 {
		t.Fatalf("应有 2 条消息，实际 %d", len(msgs))
	}
	if msgs[1].Stopped != true {
		t.Error("助手消息的 stopped 标记未持久化")
	}
	if msgs[0].Stopped != false {
		t.Error("用户消息不应有 stopped 标记")
	}
}

func TestPagination(t *testing.T) {
	setup(t)
	wsID := mustWorkspace(t, "p")
	s, _ := CreateSession(wsID)
	for i := 0; i < 100; i++ {
		AppendMessage(s.ID, "user", "消息", nil, false)
	}
	page, _ := ListMessages(s.ID, pageSize)
	if len(page) != pageSize {
		t.Errorf("分页应取 %d 条，实际 %d", pageSize, len(page))
	}
	all, _ := ListMessages(s.ID, 0)
	if len(all) != 100 {
		t.Errorf("全量应为 100 条，实际 %d", len(all))
	}
	// 分页取的是最近的，应与全量末尾一致
	if page[len(page)-1].ID != all[len(all)-1].ID {
		t.Error("分页应取最近的消息")
	}
}
