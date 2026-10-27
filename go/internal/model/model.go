// Package model 定义与 Node 版一致的共享类型。
package model

// Workspace 与磁盘上的一个项目目录一一对应。
type Workspace struct {
	ID       string  `json:"id"`
	Path     string  `json:"path"`
	Name     string  `json:"name"`
	AddedAt  string  `json:"addedAt"`
	PinnedAt *string `json:"pinnedAt"`
}

// SessionSummary 会话在列表中的展示形态。
type SessionSummary struct {
	ID             string  `json:"id"`
	WorkspaceID    string  `json:"workspaceId"`
	Title          string  `json:"title"`
	UpdatedAt      string  `json:"updatedAt"`
	MessageCount   int     `json:"messageCount"`
	Running        bool    `json:"running"`
	Renamed        bool    `json:"renamed"`
	ContextPercent float64 `json:"contextPercent"`
	ContextTokens  int     `json:"contextTokens"`
	PinnedAt       *string `json:"pinnedAt"`
}

// ChatMessage 对话流中的一条消息。
type ChatMessage struct {
	ID        string     `json:"id"`
	Role      string     `json:"role"` // user | assistant
	Text      string     `json:"text"`
	Tools     []ToolCall `json:"tools,omitempty"`
	Timestamp string     `json:"timestamp,omitempty"`
	Stopped   bool       `json:"stopped,omitempty"`
}

// ToolCall 一次工具调用。
type ToolCall struct {
	Name   string `json:"name"`
	Target string `json:"target"`
}

// EnvEntry 环境变量的一条键值。
type EnvEntry struct {
	Key   string `json:"key"`
	Value string `json:"value"`
}

// SkillInfo 一个可用 Skill。
type SkillInfo struct {
	Name        string `json:"name"`
	Description string `json:"description"`
	Source      string `json:"source"` // user | project | plugin
	Enabled     bool   `json:"enabled"`
}

// PluginInfo 一个已安装插件。
type PluginInfo struct {
	ID          string `json:"id"`
	Name        string `json:"name"`
	Version     string `json:"version"`
	Marketplace string `json:"marketplace"`
	Enabled     bool   `json:"enabled"`
}

// MarketplaceInfo 一个插件仓库。
type MarketplaceInfo struct {
	Name    string `json:"name"`
	Source  string `json:"source"`
	Builtin bool   `json:"builtin"`
}
