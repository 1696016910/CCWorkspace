// Claude 工作台 · Go 实现
//
// 与 Node 版读写同一个 SQLite 数据库、同一份 ~/.claude/settings.json，
// 通过 spawn claude 子进程走 stdio JSON 协议对话。
package main

import (
	"claude-workbench/internal/app"
	_ "embed"
	"encoding/json"
)

// 用 go:embed 而不是运行时读文件：双击启动时 cwd 不是程序所在目录，
// 读 version.json 必然失败，界面上会一直显示 v0.0.0。
//
//go:embed version.json
var versionJSON []byte

func main() {
	var v struct {
		Version string `json:"version"`
	}
	if json.Unmarshal(versionJSON, &v) == nil && v.Version != "" {
		app.Version = v.Version
	}
	app.Run()
}
