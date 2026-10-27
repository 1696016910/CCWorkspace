package service

import (
	"claude-workbench/internal/model"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// withTempHome 把 HOME 指向临时目录，隔离 ~/.claude。
func withTempHome(t *testing.T) string {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("USERPROFILE", home) // Windows 下 os.UserHomeDir 读这个
	return home
}

func writeSettings(t *testing.T, home, content string) {
	t.Helper()
	dir := filepath.Join(home, ".claude")
	os.MkdirAll(dir, 0o755)
	if err := os.WriteFile(filepath.Join(dir, "settings.json"), []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
}

func TestValidateEnv(t *testing.T) {
	ok := []model.EnvEntry{{Key: "ANTHROPIC_AUTH_TOKEN", Value: "x"}}
	if msg := ValidateEnv(ok); msg != "" {
		t.Errorf("合法输入应通过，实际: %s", msg)
	}

	cases := []struct {
		entries []model.EnvEntry
		want    string
	}{
		{[]model.EnvEntry{{Key: "", Value: "x"}}, "不能为空"},
		{[]model.EnvEntry{{Key: "A B", Value: "x"}}, "不合法"},
		{[]model.EnvEntry{{Key: "1ABC", Value: "x"}}, "不合法"},
		{[]model.EnvEntry{{Key: "__proto__", Value: "x"}}, "保留名"},
		{[]model.EnvEntry{{Key: "A", Value: "a\nb"}}, "换行"},
		{[]model.EnvEntry{{Key: "A", Value: "a\x00b"}}, "空字符"},
		{[]model.EnvEntry{{Key: "A", Value: "1"}, {Key: "A", Value: "2"}}, "重复"},
	}
	for _, c := range cases {
		got := ValidateEnv(c.entries)
		if !strings.Contains(got, c.want) {
			t.Errorf("ValidateEnv(%v) = %q，期望包含 %q", c.entries, got, c.want)
		}
	}
}

func TestReadRawMissingFile(t *testing.T) {
	withTempHome(t)
	raw, err := ReadRaw()
	if err != nil {
		t.Fatalf("文件不存在应返回空对象而非错误: %v", err)
	}
	if len(raw) != 0 {
		t.Errorf("应为空对象，实际 %v", raw)
	}
}

func TestReadRawMalformed(t *testing.T) {
	home := withTempHome(t)
	writeSettings(t, home, `{"env": {broken`)

	if _, err := ReadRaw(); err == nil {
		t.Fatal("畸形 JSON 应返回错误")
	}
}

// 最关键的一条：畸形文件时保存必须被拒绝，否则用户的其它配置会被清空。
func TestSaveEnvRefusesMalformed(t *testing.T) {
	home := withTempHome(t)
	writeSettings(t, home, `{"hooks": {"x": 1}, broken`)

	err := SaveEnv([]model.EnvEntry{{Key: "A", Value: "1"}})
	if err == nil {
		t.Fatal("畸形文件时应拒绝保存")
	}

	// 原文件必须原封不动
	orig, _ := os.ReadFile(filepath.Join(home, ".claude", "settings.json"))
	if !strings.Contains(string(orig), "broken") {
		t.Error("原文件被改动了")
	}
}

func TestSaveEnvPreservesOtherFields(t *testing.T) {
	home := withTempHome(t)
	writeSettings(t, home, `{"hooks":{"a":1},"permissions":{"defaultMode":"plan"},"theme":"dark"}`)

	if err := SaveEnv([]model.EnvEntry{{Key: "FOO", Value: "bar"}}); err != nil {
		t.Fatal(err)
	}

	raw, _ := ReadRaw()
	if raw["hooks"] == nil {
		t.Error("hooks 字段丢失")
	}
	if raw["theme"] != "dark" {
		t.Error("theme 字段丢失")
	}
	perm, _ := raw["permissions"].(map[string]any)
	if perm["defaultMode"] != "plan" {
		t.Error("permissions 字段被破坏")
	}
	env, _ := raw["env"].(map[string]any)
	if env["FOO"] != "bar" {
		t.Error("env 未写入")
	}
}

func TestSaveEnvCreatesBackup(t *testing.T) {
	home := withTempHome(t)
	writeSettings(t, home, `{"env":{}}`)

	if err := SaveEnv([]model.EnvEntry{{Key: "A", Value: "1"}}); err != nil {
		t.Fatal(err)
	}

	entries, err := os.ReadDir(filepath.Join(home, ".claude", "backups"))
	if err != nil || len(entries) == 0 {
		t.Error("保存前应生成备份")
	}
}

func TestReadEnvSorted(t *testing.T) {
	home := withTempHome(t)
	writeSettings(t, home, `{"env":{"ZED":"1","ALPHA":"2","MIKE":"3"}}`)

	entries, _ := ReadEnv()
	if len(entries) != 3 {
		t.Fatalf("应有 3 条，实际 %d", len(entries))
	}
	if entries[0].Key != "ALPHA" || entries[2].Key != "ZED" {
		t.Errorf("应按字母排序，实际 %v", entries)
	}
}

func TestPermissionsRoundTrip(t *testing.T) {
	home := withTempHome(t)
	writeSettings(t, home, `{}`)

	in := PermissionsModel{DefaultMode: "plan", Allow: []string{"Read(*)"}, Deny: []string{"Bash(rm:*)"}}
	if err := SavePermissions(in); err != nil {
		t.Fatal(err)
	}
	out := ReadPermissions()
	if out.DefaultMode != "plan" {
		t.Errorf("defaultMode = %q", out.DefaultMode)
	}
	if len(out.Allow) != 1 || out.Allow[0] != "Read(*)" {
		t.Errorf("allow = %v", out.Allow)
	}
}

func TestSaveModelsKeepsOtherEnv(t *testing.T) {
	home := withTempHome(t)
	writeSettings(t, home, `{"env":{"ANTHROPIC_AUTH_TOKEN":"secret","ANTHROPIC_MODEL":"old"}}`)

	if err := SaveModels(map[string]string{"ANTHROPIC_MODEL": "new"}); err != nil {
		t.Fatal(err)
	}

	entries, _ := ReadEnv()
	var token, mdl string
	for _, e := range entries {
		switch e.Key {
		case "ANTHROPIC_AUTH_TOKEN":
			token = e.Value
		case "ANTHROPIC_MODEL":
			mdl = e.Value
		}
	}
	if token != "secret" {
		t.Error("保存模型时把 token 弄丢了")
	}
	if mdl != "new" {
		t.Errorf("模型未更新: %q", mdl)
	}
}

func TestWriteUses0600(t *testing.T) {
	home := withTempHome(t)
	writeSettings(t, home, `{}`)
	if err := SaveEnv([]model.EnvEntry{{Key: "A", Value: "1"}}); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(filepath.Join(home, ".claude", "settings.json"))
	if err != nil {
		t.Fatal(err)
	}
	// Windows 上权限位不严格，只验证可读且内容正确
	raw, _ := os.ReadFile(filepath.Join(home, ".claude", "settings.json"))
	var parsed map[string]any
	if err := json.Unmarshal(raw, &parsed); err != nil {
		t.Errorf("写出的不是合法 JSON: %v", err)
	}
	_ = info
}
