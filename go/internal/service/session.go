package service

import (
	"claude-workbench/internal/db"
	"claude-workbench/internal/model"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/google/uuid"
)

const (
	titleMax = 40
	pageSize = 80
)

// deriveTitle 从首条消息生成标题。
//
// 按码点截断而不是按字节，否则会把一个 emoji 从代理对中间切开，
// 留下孤立半字符渲染成方块。
func deriveTitle(text string) string {
	oneLine := strings.Join(strings.Fields(text), " ")
	if oneLine == "" {
		return "(新会话)"
	}
	runes := []rune(oneLine)
	if len(runes) <= titleMax {
		return oneLine
	}
	return string(runes[:titleMax]) + "…"
}

const sessionCols = `s.id, s.workspace_id, s.title, s.auto_title, s.created_at, s.updated_at,
	s.deleted_at, s.claude_session_id, s.context_tokens, s.context_max, s.context_percent,
	s.pinned_at`

func scanSession(rows *sql.Rows, withCount bool) (model.SessionSummary, error) {
	var s model.SessionSummary
	var autoTitle, ctxMax int
	var deleted, claudeSID, pinned sql.NullString
	dest := []any{&s.ID, &s.WorkspaceID, &s.Title, &autoTitle, &s.UpdatedAt, &s.UpdatedAt,
		&deleted, &claudeSID, &s.ContextTokens, &ctxMax, &s.ContextPercent, &pinned}
	if withCount {
		dest = append(dest, &s.MessageCount)
	}
	if err := rows.Scan(dest...); err != nil {
		return s, err
	}
	s.Renamed = autoTitle == 0
	if pinned.Valid {
		s.PinnedAt = &pinned.String
	}
	return s, nil
}

// ListSessions 返回未删除的会话，置顶优先、更新时间倒序。
func ListSessions() ([]model.SessionSummary, error) {
	rows, err := db.Get().Query(
		`SELECT ` + sessionCols + `, COUNT(m.id) AS message_count
		   FROM sessions s
		   LEFT JOIN messages m ON m.session_id = s.id
		  WHERE s.deleted_at IS NULL
		  GROUP BY s.id
		  ORDER BY s.pinned_at IS NULL, s.pinned_at DESC, s.updated_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := []model.SessionSummary{}
	for rows.Next() {
		s, err := scanSession(rows, true)
		if err != nil {
			return nil, err
		}
		out = append(out, s)
	}
	return out, rows.Err()
}

// ListDeletedSessions 回收站里的会话。
func ListDeletedSessions() ([]model.SessionSummary, error) {
	rows, err := db.Get().Query(
		`SELECT ` + sessionCols + `, COUNT(m.id) AS message_count
		   FROM sessions s
		   LEFT JOIN messages m ON m.session_id = s.id
		  WHERE s.deleted_at IS NOT NULL
		  GROUP BY s.id
		  ORDER BY s.deleted_at DESC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	out := []model.SessionSummary{}
	for rows.Next() {
		s, err := scanSession(rows, true)
		if err != nil {
			return nil, err
		}
		out = append(out, s)
	}
	return out, rows.Err()
}

// CreateSession 新建会话。工作区必须存在。
func CreateSession(workspaceID string) (model.SessionSummary, error) {
	var s model.SessionSummary
	if strings.TrimSpace(workspaceID) == "" {
		return s, fmt.Errorf("请先选择一个工作区")
	}
	var exists string
	if err := db.Get().QueryRow(`SELECT id FROM workspaces WHERE id = ?`, workspaceID).Scan(&exists); err != nil {
		return s, fmt.Errorf("指定的工作区不存在")
	}
	now := db.Now()
	id := uuid.NewString()
	if _, err := db.Get().Exec(
		`INSERT INTO sessions(id, workspace_id, title, auto_title, created_at, updated_at)
		 VALUES(?, ?, '', 1, ?, ?)`, id, workspaceID, now, now); err != nil {
		return s, err
	}
	s = model.SessionSummary{
		ID: id, WorkspaceID: workspaceID, Title: "", UpdatedAt: now, Renamed: false,
	}
	return s, nil
}

// RenameSession 重命名，同时关闭自动标题。
func RenameSession(id, title string) error {
	title = strings.TrimSpace(title)
	if title == "" {
		return fmt.Errorf("标题不能为空")
	}
	res, err := db.Get().Exec(`UPDATE sessions SET title = ?, auto_title = 0 WHERE id = ?`, title, id)
	if err != nil {
		return err
	}
	if n, _ := res.RowsAffected(); n == 0 {
		return fmt.Errorf("会话不存在")
	}
	return nil
}

// DeleteSession 软删除，进回收站。
func DeleteSession(id string) error {
	res, err := db.Get().Exec(`UPDATE sessions SET deleted_at = ? WHERE id = ?`, db.Now(), id)
	if err != nil {
		return err
	}
	if n, _ := res.RowsAffected(); n == 0 {
		return fmt.Errorf("会话不存在")
	}
	return nil
}

// RestoreSession 从回收站恢复。
func RestoreSession(id string) error {
	res, err := db.Get().Exec(`UPDATE sessions SET deleted_at = NULL WHERE id = ?`, id)
	if err != nil {
		return err
	}
	if n, _ := res.RowsAffected(); n == 0 {
		return fmt.Errorf("会话不存在")
	}
	return nil
}

// PurgeSession 彻底删除会话与消息。
func PurgeSession(id string) error {
	var exists string
	if err := db.Get().QueryRow(`SELECT id FROM sessions WHERE id = ?`, id).Scan(&exists); err != nil {
		return fmt.Errorf("会话不存在")
	}
	tx, err := db.Get().Begin()
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err := tx.Exec(`DELETE FROM messages WHERE session_id = ?`, id); err != nil {
		return err
	}
	if _, err := tx.Exec(`DELETE FROM sessions WHERE id = ?`, id); err != nil {
		return err
	}
	return tx.Commit()
}

// DuplicateSession 复制会话（含消息），新会话不继承 CLI 会话 id。
func DuplicateSession(id string) (model.SessionSummary, error) {
	var out model.SessionSummary
	var src model.SessionSummary
	var autoTitle int
	err := db.Get().QueryRow(
		`SELECT id, workspace_id, title, auto_title FROM sessions WHERE id = ?`, id).
		Scan(&src.ID, &src.WorkspaceID, &src.Title, &autoTitle)
	if err == sql.ErrNoRows {
		return out, fmt.Errorf("会话不存在")
	}
	if err != nil {
		return out, err
	}

	now := db.Now()
	newID := uuid.NewString()
	title := ""
	if src.Title != "" {
		title = src.Title + " 副本"
	}
	// 源会话还没有标题时，副本也保持 auto_title=1，否则它永远等不到自动命名
	newAuto := 1
	if src.Title != "" {
		newAuto = 0
	}

	tx, err := db.Get().Begin()
	if err != nil {
		return out, err
	}
	defer tx.Rollback()

	if _, err := tx.Exec(
		`INSERT INTO sessions(id, workspace_id, title, auto_title, created_at, updated_at)
		 VALUES(?, ?, ?, ?, ?, ?)`, newID, src.WorkspaceID, title, newAuto, now, now); err != nil {
		return out, err
	}

	rows, err := tx.Query(
		`SELECT role, content, tools, created_at FROM messages
		  WHERE session_id = ? ORDER BY created_at ASC`, id)
	if err != nil {
		return out, err
	}
	type row struct{ role, content, created, tools string }
	var msgs []row
	for rows.Next() {
		var r row
		var tools sql.NullString
		if err := rows.Scan(&r.role, &r.content, &tools, &r.created); err != nil {
			rows.Close()
			return out, err
		}
		r.tools = tools.String
		msgs = append(msgs, r)
	}
	rows.Close()

	for _, m := range msgs {
		var toolsVal any
		if m.tools != "" {
			toolsVal = m.tools
		}
		if _, err := tx.Exec(
			`INSERT INTO messages(id, session_id, role, content, tools, created_at)
			 VALUES(?, ?, ?, ?, ?, ?)`, uuid.NewString(), newID, m.role, m.content, toolsVal, m.created); err != nil {
			return out, err
		}
	}
	if err := tx.Commit(); err != nil {
		return out, err
	}
	out = model.SessionSummary{ID: newID, WorkspaceID: src.WorkspaceID, Title: title, UpdatedAt: now}
	return out, nil
}

// PinSession 置顶或取消置顶。
func PinSession(id string, pinned bool) error {
	var exists string
	if err := db.Get().QueryRow(`SELECT id FROM sessions WHERE id = ?`, id).Scan(&exists); err != nil {
		return fmt.Errorf("会话不存在")
	}
	var at any
	if pinned {
		at = db.Now()
	}
	_, err := db.Get().Exec(`UPDATE sessions SET pinned_at = ? WHERE id = ?`, at, id)
	return err
}

// AppendMessage 追加一条消息；会话已被删除时返回错误。
//
// 这里必须报错而不是静默丢弃：静默丢弃会让上层以为写入成功，
// 界面上消息看着发出去了，刷新后却消失。
func AppendMessage(sessionID, role, content string, tools []model.ToolCall, stopped bool) (model.ChatMessage, error) {
	var msg model.ChatMessage
	var deleted sql.NullString
	err := db.Get().QueryRow(`SELECT deleted_at FROM sessions WHERE id = ?`, sessionID).Scan(&deleted)
	if err == sql.ErrNoRows {
		return msg, fmt.Errorf("会话不存在")
	}
	if err != nil {
		return msg, err
	}
	if deleted.Valid {
		return msg, fmt.Errorf("会话已被删除，消息未保存")
	}

	now := db.Now()
	id := uuid.NewString()
	var toolsJSON any
	if len(tools) > 0 {
		b, _ := json.Marshal(tools)
		toolsJSON = string(b)
	}
	stoppedInt := 0
	if stopped {
		stoppedInt = 1
	}
	if _, err := db.Get().Exec(
		`INSERT INTO messages(id, session_id, role, content, tools, created_at, stopped)
		 VALUES(?, ?, ?, ?, ?, ?, ?)`,
		id, sessionID, role, content, toolsJSON, now, stoppedInt); err != nil {
		return msg, err
	}
	if _, err := db.Get().Exec(`UPDATE sessions SET updated_at = ? WHERE id = ?`, now, sessionID); err != nil {
		return msg, err
	}

	// 首条用户消息补自动标题
	if role == "user" {
		var autoTitle int
		var title string
		if err := db.Get().QueryRow(
			`SELECT auto_title, title FROM sessions WHERE id = ?`, sessionID).Scan(&autoTitle, &title); err == nil {
			if autoTitle == 1 && title == "" {
				db.Get().Exec(`UPDATE sessions SET title = ? WHERE id = ?`, deriveTitle(content), sessionID)
			}
		}
	}

	return model.ChatMessage{ID: id, Role: role, Text: content, Tools: tools, Timestamp: now, Stopped: stopped}, nil
}

// ListMessages 取最近 limit 条（0 表示全部），按时间正序返回。
func ListMessages(sessionID string, limit int) ([]model.ChatMessage, error) {
	q := `SELECT id, role, content, tools, created_at, stopped FROM messages
	       WHERE session_id = ? ORDER BY created_at ASC`
	args := []any{sessionID}
	if limit > 0 {
		q = `SELECT * FROM (
		       SELECT id, role, content, tools, created_at, stopped FROM messages
		        WHERE session_id = ? ORDER BY created_at DESC LIMIT ?
		     ) ORDER BY created_at ASC`
		args = append(args, limit)
	}
	rows, err := db.Get().Query(q, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	out := []model.ChatMessage{}
	for rows.Next() {
		var m model.ChatMessage
		var tools sql.NullString
		var stopped int
		if err := rows.Scan(&m.ID, &m.Role, &m.Text, &tools, &m.Timestamp, &stopped); err != nil {
			return nil, err
		}
		m.Stopped = stopped == 1
		if tools.Valid && tools.String != "" {
			var tc []model.ToolCall
			if json.Unmarshal([]byte(tools.String), &tc) == nil {
				m.Tools = tc
			}
		}
		out = append(out, m)
	}
	return out, rows.Err()
}

// CountMessages 会话消息总数。
func CountMessages(sessionID string) (int, error) {
	var n int
	err := db.Get().QueryRow(
		`SELECT COUNT(*) FROM messages WHERE session_id = ?`, sessionID).Scan(&n)
	return n, err
}

// GetClaudeSessionID 取该会话对应的 CLI 会话 id，用于 resume。
func GetClaudeSessionID(sessionID string) (string, error) {
	var sid sql.NullString
	err := db.Get().QueryRow(
		`SELECT claude_session_id FROM sessions WHERE id = ?`, sessionID).Scan(&sid)
	if err != nil {
		return "", err
	}
	return sid.String, nil
}

// SetClaudeSessionID 记录 CLI 会话 id。
func SetClaudeSessionID(sessionID, claudeSID string) error {
	_, err := db.Get().Exec(
		`UPDATE sessions SET claude_session_id = ? WHERE id = ?`, claudeSID, sessionID)
	return err
}

// SetContextUsage 写入上下文用量快照。
func SetContextUsage(sessionID string, tokens, max int, pct float64) error {
	_, err := db.Get().Exec(
		`UPDATE sessions SET context_tokens = ?, context_max = ?, context_percent = ?
		  WHERE id = ?`, tokens, max, pct, sessionID)
	return err
}
