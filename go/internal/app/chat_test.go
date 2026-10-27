package app

import (
	"claude-workbench/internal/claude"
	"fmt"
	"os"
	"strings"
	"testing"
)

// 真实调用 claude 验证端到端链路。
//
// 需要网络与已登录的 CLI，且实测可能一轮跑几分钟：
// 必须显式设置 CW_REAL_TEST=1 才跑，否则默认跳过，
// 避免 `go test ./...` 撞上 600 秒超时变成假失败。
func TestRealChat(t *testing.T) {
	if os.Getenv("CW_REAL_TEST") != "1" {
		t.Skip("跳过真实对话测试（CW_REAL_TEST=1 才运行）")
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
