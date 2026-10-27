// Claude 工作台 · Go 实现
//
// 与 Node 版读写同一个 SQLite 数据库、同一份 ~/.claude/settings.json，
// 通过 spawn claude 子进程走 stdio JSON 协议对话。
package main

import (
	"claude-workbench/internal/app"
	"encoding/json"
	"os"
)

func main() {
	// 版本号从 version.json 读，与 CI 保持同一来源
	if raw, err := os.ReadFile("version.json"); err == nil {
		var v struct {
			Version string `json:"version"`
		}
		if json.Unmarshal(raw, &v) == nil && v.Version != "" {
			app.Version = v.Version
		}
	}
	app.Run()
}
