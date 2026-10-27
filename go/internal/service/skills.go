package service

import (
	"claude-workbench/internal/db"
	"claude-workbench/internal/model"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"
)

// ListSkills 扫描 ~/.claude/skills，返回全局 Skill。
//
// 只列全局：项目级 Skill 依赖某个具体工作区，而现在会话总是归属
// 明确的工作区，设置页不再需要选"看哪个工作区的 Skill"。
func ListSkills() ([]model.SkillInfo, error) {
	dir, err := ClaudeDir()
	if err != nil {
		return nil, err
	}
	skillsDir := filepath.Join(dir, "skills")
	entries, err := os.ReadDir(skillsDir)
	if os.IsNotExist(err) {
		return []model.SkillInfo{}, nil
	}
	if err != nil {
		return nil, err
	}

	disabled := disabledSkillSet()
	out := []model.SkillInfo{}
	for _, e := range entries {
		if !e.IsDir() {
			continue
		}
		mdPath := filepath.Join(skillsDir, e.Name(), "SKILL.md")
		content, err := os.ReadFile(mdPath)
		if err != nil {
			continue
		}
		name, desc := parseFrontmatter(string(content), e.Name())
		out = append(out, model.SkillInfo{
			Name:        name,
			Description: desc,
			Source:      "user",
			Enabled:     !disabled[name],
		})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Name < out[j].Name })
	return out, nil
}

// ToggleSkill 启用/禁用一个 Skill。
//
// 名单存在数据库 meta.disabled_skills，和 Node 版同一个位置：
// 三份实现共用一个库，存文件里会导致各版本互相看不到对方的开关。
func ToggleSkill(name string, enabled bool) error {
	conn := db.Get()
	if conn == nil {
		return fmt.Errorf("数据库未打开")
	}
	set := disabledSkillSet()
	if enabled {
		delete(set, name)
	} else {
		set[name] = true
	}
	list := make([]string, 0, len(set))
	for k := range set {
		list = append(list, k)
	}
	sort.Strings(list)

	body, err := json.Marshal(list)
	if err != nil {
		return err
	}
	_, err = conn.Exec(
		`INSERT INTO meta(key, value) VALUES('disabled_skills', ?)
		 ON CONFLICT(key) DO UPDATE SET value = excluded.value`, string(body))
	return err
}

// disabledSkillSet 读取禁用名单。
func disabledSkillSet() map[string]bool {
	out := map[string]bool{}
	conn := db.Get()
	if conn == nil {
		return out
	}
	var raw string
	if err := conn.QueryRow(`SELECT value FROM meta WHERE key = 'disabled_skills'`).Scan(&raw); err != nil {
		return out
	}
	var list []string
	if json.Unmarshal([]byte(raw), &list) == nil {
		for _, n := range list {
			out[n] = true
		}
	}
	return out
}

// parseFrontmatter 从 SKILL.md 的 frontmatter 里取 name 与 description。
//
// 只处理单行 `key: value` 形式；多行值（`description: >`）退化为取首行，
// 这是够用的最小实现，界面本来也只展示一行摘要。
func parseFrontmatter(content, fallbackName string) (name, desc string) {
	name = fallbackName
	lines := strings.Split(content, "\n")
	if len(lines) == 0 || strings.TrimSpace(lines[0]) != "---" {
		return
	}
	for _, l := range lines[1:] {
		t := strings.TrimSpace(l)
		if t == "---" {
			break
		}
		if v, ok := strings.CutPrefix(t, "name:"); ok {
			if s := strings.TrimSpace(v); s != "" && s != ">" && s != "|" {
				name = s
			}
		}
		if v, ok := strings.CutPrefix(t, "description:"); ok {
			s := strings.TrimSpace(v)
			if s != "" && s != ">" && s != "|" {
				desc = s
			}
		}
	}
	return
}

// ListPlugins 调用 claude plugin list --json；失败时退回空列表。
func ListPlugins() ([]model.PluginInfo, error) {
	out, err := runClaude([]string{"plugin", "list", "--json"})
	if err != nil {
		return []model.PluginInfo{}, nil
	}
	var items []struct {
		ID      string `json:"id"`
		Version string `json:"version"`
		Enabled bool   `json:"enabled"`
	}
	if err := json.Unmarshal([]byte(out), &items); err != nil {
		return []model.PluginInfo{}, nil
	}
	res := make([]model.PluginInfo, 0, len(items))
	for _, p := range items {
		name, market := splitPluginID(p.ID)
		res = append(res, model.PluginInfo{
			ID: p.ID, Name: name, Version: p.Version, Marketplace: market, Enabled: p.Enabled,
		})
	}
	return res, nil
}

// TogglePlugin 启用/禁用插件。
func TogglePlugin(id string, enabled bool) error {
	action := "disable"
	if enabled {
		action = "enable"
	}
	_, err := runClaude([]string{"plugin", action, id})
	if err != nil {
		return fmt.Errorf("调用 claude 命令失败: %w", err)
	}
	return nil
}

// InstallPlugin 安装插件。
func InstallPlugin(id string) error {
	if _, err := runClaude([]string{"plugin", "install", id}); err != nil {
		return fmt.Errorf("调用 claude 命令失败: %w", err)
	}
	return nil
}

// UninstallPlugin 卸载插件。
func UninstallPlugin(id string) error {
	if _, err := runClaude([]string{"plugin", "uninstall", id}); err != nil {
		return fmt.Errorf("调用 claude 命令失败: %w", err)
	}
	return nil
}

// ListMarketplaces 列出插件仓库。
func ListMarketplaces() ([]model.MarketplaceInfo, error) {
	raw, err := ReadRaw()
	if err != nil {
		return []model.MarketplaceInfo{}, nil
	}
	known, _ := raw["extraKnownMarketplaces"].(map[string]any)
	out := []model.MarketplaceInfo{{
		Name: "claude-plugins-official", Source: "github.com/anthropics/claude-plugins", Builtin: true,
	}}
	names := make([]string, 0, len(known))
	for n := range known {
		names = append(names, n)
	}
	sort.Strings(names)
	for _, n := range names {
		src := ""
		if m, ok := known[n].(map[string]any); ok {
			if s, ok := m["source"].(map[string]any); ok {
				if u, ok := s["url"].(string); ok {
					src = u
				} else if r, ok := s["repo"].(string); ok {
					src = r
				}
			}
		}
		out = append(out, model.MarketplaceInfo{Name: n, Source: src})
	}
	return out, nil
}

// AddMarketplace 添加插件仓库。
func AddMarketplace(name, repo string) error {
	name, repo = strings.TrimSpace(name), strings.TrimSpace(repo)
	if name == "" || repo == "" {
		return fmt.Errorf("仓库名称与地址都不能为空")
	}
	raw, err := ReadRaw()
	if err != nil {
		return err
	}
	known, _ := raw["extraKnownMarketplaces"].(map[string]any)
	if known == nil {
		known = map[string]any{}
	}
	known[name] = map[string]any{
		"source": map[string]any{"source": "github", "repo": repo},
	}
	raw["extraKnownMarketplaces"] = known
	return WriteRaw(raw, nil)
}

// RemoveMarketplace 移除插件仓库。内置仓库不可移除。
func RemoveMarketplace(name string) error {
	if name == "claude-plugins-official" {
		return fmt.Errorf("内置仓库不可移除")
	}
	raw, err := ReadRaw()
	if err != nil {
		return err
	}
	known, _ := raw["extraKnownMarketplaces"].(map[string]any)
	if known == nil {
		return nil
	}
	delete(known, name)
	raw["extraKnownMarketplaces"] = known
	return WriteRaw(raw, nil)
}

func splitPluginID(id string) (name, marketplace string) {
	at := strings.LastIndex(id, "@")
	if at < 0 {
		return id, "unknown"
	}
	return id[:at], id[at+1:]
}

// runClaude 调用 claude 子命令。
//
// Windows 上 npm 装的 claude 是 .cmd 垫片，exec.Command 不查 PATHEXT，
// 直接调 "claude" 会 ENOENT，必须经 cmd.exe。
func runClaude(args []string) (string, error) {
	var cmd *exec.Cmd
	if isWindows() {
		quoted := make([]string, len(args))
		for i, a := range args {
			quoted[i] = a
		}
		cmd = exec.Command("cmd.exe", append([]string{"/d", "/s", "/c", "claude"}, quoted...)...)
	} else {
		cmd = exec.Command("claude", args...)
	}
	out, err := cmd.Output()
	if err != nil {
		return "", err
	}
	return string(out), nil
}
