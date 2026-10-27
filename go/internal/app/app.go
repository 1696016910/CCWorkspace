// Package app 组装业务层与 UI。
//
// UI 通过 API 接口反向调用这里，避免 import 循环。
package app

import (
	"claude-workbench/internal/claude"
	"claude-workbench/internal/db"
	"claude-workbench/internal/model"
	"claude-workbench/internal/service"
	"claude-workbench/internal/ui"
	"fmt"
	"log"
	"strings"
	"sync"
)

// Version 由 main 从 version.json 注入。
var Version = "0.0.0"

// bridge 实现 ui.API。
type bridge struct{}

// Run 启动应用。
func Run() {
	if _, err := db.Open(); err != nil {
		ui.Fatal("无法打开数据库", err.Error())
		return
	}
	defer db.Close()

	ui.SetVersion(Version)
	w := ui.NewMainWindow(bridge{}, chatOnce)
	w.Run()
}

func (bridge) LoadWorkspaces() []model.Workspace {
	list, err := service.ListWorkspaces()
	if err != nil {
		log.Printf("加载工作区失败: %v", err)
		return nil
	}
	return list
}

func (bridge) LoadSessions() []model.SessionSummary {
	list, err := service.ListSessions()
	if err != nil {
		log.Printf("加载会话失败: %v", err)
		return nil
	}
	return list
}

func (bridge) LoadMessages(sessionID string) []model.ChatMessage {
	msgs, err := service.ListMessages(sessionID, 80)
	if err != nil {
		log.Printf("加载消息失败: %v", err)
		return nil
	}
	return msgs
}

func (bridge) AddWorkspace(dir, name string) ([]model.Workspace, error) {
	return service.AddWorkspace(dir, name)
}

func (bridge) RemoveWorkspace(id string) ([]model.Workspace, error) {
	return service.RemoveWorkspace(id)
}

func (bridge) RenameWorkspace(id, name string) ([]model.Workspace, error) {
	return service.RenameWorkspace(id, name)
}

func (bridge) PinWorkspace(id string, pinned bool) ([]model.Workspace, error) {
	return service.PinWorkspace(id, pinned)
}

func (bridge) NewSession(workspaceID string) (model.SessionSummary, error) {
	return service.CreateSession(workspaceID)
}

func (bridge) DeleteSession(id string) error { return service.DeleteSession(id) }
func (bridge) RenameSession(id, title string) error { return service.RenameSession(id, title) }
func (bridge) PinSession(id string, pinned bool) error {
	return service.PinSession(id, pinned)
}
func (bridge) DuplicateSession(id string) (model.SessionSummary, error) {
	return service.DuplicateSession(id)
}

func (bridge) DeletedSessions() []model.SessionSummary {
	list, err := service.ListDeletedSessions()
	if err != nil {
		log.Printf("加载回收站失败: %v", err)
		return nil
	}
	return list
}

func (bridge) RestoreSession(id string) error { return service.RestoreSession(id) }
func (bridge) PurgeSession(id string) error   { return service.PurgeSession(id) }

// ReadFile 读取工作区内的文本文件用于预览。
//
// 预览入口来自模型输出，属于不可信内容：必须限制在工作区内，
// 否则一个指向 ~/.ssh/id_rsa 的链接点一下就把私钥显示出来了。
func (bridge) ReadFile(workspacePath, relPath string) (string, error) {
	return service.ReadWorkspaceFile(workspacePath, relPath)
}

func (bridge) Summary() string {
	p, err := db.Path()
	if err != nil {
		p = "(未知)"
	}
	return fmt.Sprintf("Claude 工作台 · Go 版 v%s\n数据库：%s", Version, p)
}

// 同一时间只允许一轮对话。
var (
	runMu   sync.Mutex
	running bool
)

// chatOnce 跑一轮对话：spawn claude、把事件转成 UI 回调、写库。
func chatOnce(sessionID, text string, onDelta func(string), onDone func(string, bool)) {
	runMu.Lock()
	if running {
		runMu.Unlock()
		onDone("", false)
		return
	}
	running = true
	runMu.Unlock()

	defer func() {
		runMu.Lock()
		running = false
		runMu.Unlock()
	}()

	// 找会话所属工作区作为 cwd
	var cwd string
	for _, s := range mustSessions() {
		if s.ID == sessionID {
			for _, w := range mustWorkspaces() {
				if w.ID == s.WorkspaceID {
					cwd = w.Path
				}
			}
		}
	}
	if cwd == "" {
		onDone("会话所属工作区不存在", false)
		return
	}

	if _, err := service.AppendMessage(sessionID, "user", text, nil, false); err != nil {
		onDone(err.Error(), false)
		return
	}

	opts := claude.Options{
		Cwd:    cwd,
		Text:   text,
		Model:  service.ReadModels()["ANTHROPIC_MODEL"],
		Resume: mustClaudeSID(sessionID),
		Mode:   service.ReadPermissions().DefaultMode,
		Env:    envMap(),
	}

	var (
		acc     strings.Builder
		tools   []model.ToolCall
		runErr  string
		stopped bool
	)
	err := claude.Run(opts, func(e claude.Event) {
		switch e.Type {
		case "text":
			acc.WriteString(e.Delta)
			onDelta(e.Delta)
		case "claudeSession":
			service.SetClaudeSessionID(sessionID, e.ClaudeSID)
		case "tool":
			if e.Tool != nil {
				tools = append(tools, model.ToolCall{Name: e.Tool.Name, Target: e.Tool.Target})
			}
		case "context":
			if e.Ctx != nil {
				service.SetContextUsage(sessionID, e.Ctx.TotalTokens, e.Ctx.MaxTokens, e.Ctx.Percentage)
			}
		case "error":
			runErr = e.Error
		}
	})
	if err != nil {
		runErr = err.Error()
	}
	if runErr != "" {
		onDone(runErr, false)
		return
	}
	if acc.Len() == 0 && len(tools) == 0 {
		onDone("模型没有返回任何内容", false)
		return
	}

	// 助手回复必须落库，否则刷新后消失
	if _, err := service.AppendMessage(sessionID, "assistant", acc.String(), tools, stopped); err != nil {
		onDone(err.Error(), false)
		return
	}
	onDone(acc.String(), stopped)
}

func mustWorkspaces() []model.Workspace {
	l, _ := service.ListWorkspaces()
	return l
}

func mustSessions() []model.SessionSummary {
	l, _ := service.ListSessions()
	return l
}

func mustClaudeSID(sessionID string) string {
	s, _ := service.GetClaudeSessionID(sessionID)
	return s
}

// envMap 从 settings.json 的 env 区段取环境变量。
func envMap() map[string]string {
	entries, _ := service.ReadEnv()
	out := make(map[string]string, len(entries))
	for _, e := range entries {
		out[e.Key] = e.Value
	}
	return out
}
