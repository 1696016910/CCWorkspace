package main

import (
	"claude-workbench/internal/claude"
	"claude-workbench/internal/service"
	"fmt"
)

func main() {
	skills := service.EnabledSkillNames()
	fmt.Println("启用的 Skill 数:", len(skills))
	opts := claude.Options{Cwd: "D:/Project/AI/CCWorkspace", Text: "只回复两个字：收到", Mode: "acceptEdits", Skills: skills}
	var acc string
	err := claude.Run(opts, func(e claude.Event) {
		switch e.Type {
		case "text":
			acc += e.Delta
		case "claudeSession":
			fmt.Println("CLI 会话 id:", e.ClaudeSID[:8])
		case "context":
			fmt.Printf("上下文: %d/%d = %.1f%%\n", e.Ctx.TotalTokens, e.Ctx.MaxTokens, e.Ctx.Percentage)
		case "error":
			fmt.Println("错误:", e.Error)
		}
	})
	fmt.Println("失败:", err)
	fmt.Printf("累积文本: %q\n", acc)
}
