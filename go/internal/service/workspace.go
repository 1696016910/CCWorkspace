// Package service 实现工作区与会话的业务逻辑。
//
// 与 Node 版的 services/workspace.ts、services/session.ts 行为一致，
// 读写同一个数据库。
package service

import (
	"claude-workbench/internal/db"
	"claude-workbench/internal/model"
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/google/uuid"
)

// ListWorkspaces 按置顶优先、创建时间升序返回。
func ListWorkspaces() ([]model.Workspace, error) {
	rows, err := db.Get().Query(
		`SELECT id, path, name, created_at, pinned_at FROM workspaces
		  ORDER BY pinned_at IS NULL, pinned_at DESC, created_at ASC`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	out := []model.Workspace{}
	for rows.Next() {
		var w model.Workspace
		var pinned sql.NullString
		if err := rows.Scan(&w.ID, &w.Path, &w.Name, &w.AddedAt, &pinned); err != nil {
			return nil, err
		}
		if pinned.Valid {
			w.PinnedAt = &pinned.String
		}
		out = append(out, w)
	}
	return out, rows.Err()
}

// AddWorkspace 添加一个工作区。路径重复时拒绝。
func AddWorkspace(dir, customName string) ([]model.Workspace, error) {
	dir = strings.TrimSpace(dir)
	if dir == "" {
		return nil, fmt.Errorf("请选择一个目录")
	}
	abs, err := filepath.Abs(dir)
	if err != nil {
		return nil, fmt.Errorf("路径无效: %w", err)
	}
	info, err := os.Stat(abs)
	if err != nil || !info.IsDir() {
		return nil, fmt.Errorf("目录不存在或不可读：%s", abs)
	}

	// Windows 与 macOS 路径大小写不敏感，精确比较会让同一目录被加两次
	var existing string
	q := `SELECT id FROM workspaces WHERE path = ?`
	if runtime.GOOS == "windows" || runtime.GOOS == "darwin" {
		q = `SELECT id FROM workspaces WHERE path = ? COLLATE NOCASE`
	}
	if err := db.Get().QueryRow(q, abs).Scan(&existing); err == nil {
		return nil, fmt.Errorf("该工作区已存在")
	}

	name := strings.TrimSpace(customName)
	if name == "" {
		name = filepath.Base(abs)
	}
	_, err = db.Get().Exec(
		`INSERT INTO workspaces(id, path, name, created_at) VALUES(?, ?, ?, ?)`,
		uuid.NewString(), abs, name, db.Now())
	if err != nil {
		return nil, err
	}
	return ListWorkspaces()
}

// RemoveWorkspace 删除工作区，会话与消息级联删除。不触碰磁盘目录。
func RemoveWorkspace(id string) ([]model.Workspace, error) {
	if _, err := db.Get().Exec(`DELETE FROM workspaces WHERE id = ?`, id); err != nil {
		return nil, err
	}
	return ListWorkspaces()
}

// RenameWorkspace 只改展示名。
func RenameWorkspace(id, name string) ([]model.Workspace, error) {
	name = strings.TrimSpace(name)
	if name == "" {
		return nil, fmt.Errorf("名称不能为空")
	}
	if _, err := db.Get().Exec(`UPDATE workspaces SET name = ? WHERE id = ?`, name, id); err != nil {
		return nil, err
	}
	return ListWorkspaces()
}

// PinWorkspace 置顶或取消置顶。
func PinWorkspace(id string, pinned bool) ([]model.Workspace, error) {
	var exists string
	if err := db.Get().QueryRow(`SELECT id FROM workspaces WHERE id = ?`, id).Scan(&exists); err != nil {
		return nil, fmt.Errorf("工作区不存在")
	}
	var at any
	if pinned {
		at = db.Now()
	}
	if _, err := db.Get().Exec(`UPDATE workspaces SET pinned_at = ? WHERE id = ?`, at, id); err != nil {
		return nil, err
	}
	return ListWorkspaces()
}
