package ui

import "claude-workbench/internal/model"

// API 是 UI 依赖的业务接口。
//
// 用接口而不是直接 import app 包，避免 app → ui → app 的循环依赖，
// 也让 UI 可以在没有数据库的情况下单独构造（便于验证布局）。
type API interface {
	LoadWorkspaces() []model.Workspace
	LoadSessions() []model.SessionSummary
	LoadMessages(sessionID string) []model.ChatMessage

	AddWorkspace(dir, name string) ([]model.Workspace, error)
	RemoveWorkspace(id string) ([]model.Workspace, error)
	RenameWorkspace(id, name string) ([]model.Workspace, error)
	PinWorkspace(id string, pinned bool) ([]model.Workspace, error)

	NewSession(workspaceID string) (model.SessionSummary, error)
	DeleteSession(id string) error
	RenameSession(id, title string) error
	PinSession(id string, pinned bool) error
	DuplicateSession(id string) (model.SessionSummary, error)

	Summary() string
}

// ChatFn 一轮对话的实现，由 app 层注入。
//
// onDelta 每收到一段增量文本调用一次；onDone 在结束时调用，
// stopped 表示这一轮被用户中断。
type ChatFn func(sessionID, text string, onDelta func(string), onDone func(full string, stopped bool))
