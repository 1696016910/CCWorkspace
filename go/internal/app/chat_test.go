package app

import (
	"claude-workbench/internal/claude"
	"fmt"
	"strings"
	"testing"
)

// 真实调用 claude 验证端到端链路。
// 用 -short 跳过：需要网络与已登录的 CLI。
func TestRealChat(t *testing.T) {
	if testing.Short() {
		t.Skip("跳过真实对话测试")
	}

	var acc strings.Builder
	var gotSession bool
	err := claude.Run(claude.Options{
		Cwd:  "D:/Project/AI/CCWorkspace",
		Text: "只回复两个字：收到",
		Mode: "acceptEdits",
	}, func(e claude.Event) {
		switch e.Type {
		case "text":
			acc.WriteString(e.Delta)
		case "claudeSession":
			gotSession = true
		}
	})
	if err != nil {
		t.Fatalf("对话失败: %v", err)
	}
	fmt.Printf("回复: %q\n", acc.String())
	if acc.Len() == 0 {
		t.Error("没有收到任何文本")
	}
	if !gotSession {
		t.Error("没有收到 CLI 会话 id")
	}
}
