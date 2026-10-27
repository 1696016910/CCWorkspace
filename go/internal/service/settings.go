package service

import (
	"claude-workbench/internal/model"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"
)

const maxBackups = 20

// ClaudeDir 返回 ~/.claude 目录。
func ClaudeDir() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(home, ".claude"), nil
}

// SettingsPath 返回 ~/.claude/settings.json。
func SettingsPath() (string, error) {
	dir, err := ClaudeDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(dir, "settings.json"), nil
}

// ReadRaw 读取 settings.json 的原始 JSON。文件不存在时返回空对象。
//
// 解析失败时返回一个非 nil 的错误，调用方必须据此**拒绝写入**：
// 若把空对象当作原始配置写回去，用户的所有其它配置会被清空。
func ReadRaw() (map[string]any, error) {
	p, err := SettingsPath()
	if err != nil {
		return nil, err
	}
	raw, err := os.ReadFile(p)
	if os.IsNotExist(err) {
		return map[string]any{}, nil
	}
	if err != nil {
		return nil, fmt.Errorf("读取 settings.json 失败: %w", err)
	}

	var out map[string]any
	if err := json.Unmarshal(raw, &out); err != nil {
		return nil, fmt.Errorf("settings.json 无法解析，请先修复该文件")
	}
	if out == nil {
		out = map[string]any{}
	}
	return out, nil
}

// WriteRaw 安全写入：备份 → 临时文件 → rename。
//
// expectedMtime 非空时做并发冲突检测；文件在别处被改过就拒绝写入。
func WriteRaw(next map[string]any, expectedMtime *time.Time) error {
	p, err := SettingsPath()
	if err != nil {
		return err
	}

	if expectedMtime != nil {
		if info, err := os.Stat(p); err == nil {
			if info.ModTime().Sub(*expectedMtime).Abs() > time.Second {
				return fmt.Errorf("文件已被其它进程修改，请重新加载后再保存")
			}
		}
	}

	// 先备份
	if _, err := os.Stat(p); err == nil {
		dir, _ := ClaudeDir()
		backupDir := filepath.Join(dir, "backups")
		if err := os.MkdirAll(backupDir, 0o755); err == nil {
			orig, readErr := os.ReadFile(p)
			if readErr == nil {
				stamp := time.Now().Format("2006-01-02T15-04-05.000Z")
				_ = os.WriteFile(filepath.Join(backupDir, "settings-"+stamp+".json"), orig, 0o600)
				pruneBackups(backupDir)
			}
		}
	}

	body, err := json.MarshalIndent(next, "", "  ")
	if err != nil {
		return fmt.Errorf("序列化失败: %w", err)
	}

	tmp := fmt.Sprintf("%s.tmp-%d", p, time.Now().UnixNano())
	// 0600：settings.json 里可能有 API Token，rename 会保留临时文件的权限
	if err := os.WriteFile(tmp, body, 0o600); err != nil {
		return fmt.Errorf("写入失败: %w", err)
	}
	if err := os.Rename(tmp, p); err != nil {
		os.Remove(tmp)
		return fmt.Errorf("替换文件失败: %w", err)
	}
	return nil
}

// pruneBackups 只保留最近 maxBackups 份。
func pruneBackups(dir string) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return
	}
	var names []string
	for _, e := range entries {
		n := e.Name()
		if strings.HasPrefix(n, "settings-") && strings.HasSuffix(n, ".json") {
			names = append(names, n)
		}
	}
	sort.Strings(names)
	excess := len(names) - maxBackups
	for i := 0; i < excess; i++ {
		os.Remove(filepath.Join(dir, names[i]))
	}
}

var envKeyRe = regexp.MustCompile(`^[A-Za-z_][A-Za-z0-9_]*$`)

// ValidateEnv 校验环境变量条目。返回空串表示通过。
func ValidateEnv(entries []model.EnvEntry) string {
	seen := map[string]bool{}
	for _, e := range entries {
		k := strings.TrimSpace(e.Key)
		if k == "" {
			return "键名不能为空"
		}
		if !envKeyRe.MatchString(k) {
			return fmt.Sprintf("键名不合法：%s（只允许字母、数字、下划线，且不能以数字开头）", k)
		}
		// 这些名字在普通对象上赋值会改到原型链
		if k == "__proto__" || k == "constructor" || k == "prototype" {
			return fmt.Sprintf("键名不允许使用保留名：%s", k)
		}
		if seen[k] {
			return fmt.Sprintf("键名重复：%s", k)
		}
		// NUL 会让 CLI 的 spawn 直接失败
		if strings.ContainsAny(e.Value, "\r\n\x00") {
			return fmt.Sprintf("值不能包含换行或空字符：%s", k)
		}
		seen[k] = true
	}
	return ""
}

// ReadEnv 读取 env 区段。
func ReadEnv() ([]model.EnvEntry, error) {
	raw, err := ReadRaw()
	if err != nil {
		// 读失败时返回空列表而不是错误：界面要能打开并显示提示
		return []model.EnvEntry{}, nil
	}
	envMap, _ := raw["env"].(map[string]any)
	out := []model.EnvEntry{}
	for k, v := range envMap {
		if s, ok := v.(string); ok {
			out = append(out, model.EnvEntry{Key: k, Value: s})
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Key < out[j].Key })
	return out, nil
}

// SaveEnv 保存 env 区段，其它字段原样保留。
func SaveEnv(entries []model.EnvEntry) error {
	if msg := ValidateEnv(entries); msg != "" {
		return fmt.Errorf("%s", msg)
	}
	p, _ := SettingsPath()
	var mtime *time.Time
	if info, err := os.Stat(p); err == nil {
		t := info.ModTime()
		mtime = &t
	}

	raw, err := ReadRaw()
	if err != nil {
		return err
	}

	env := map[string]any{}
	for _, e := range entries {
		env[strings.TrimSpace(e.Key)] = e.Value
	}
	raw["env"] = env
	return WriteRaw(raw, mtime)
}

// ReadPermissionsModel 读取权限配置。
type PermissionsModel struct {
	DefaultMode string
	Allow       []string
	Deny        []string
}

// ReadPermissions 读取 permissions 区段。
func ReadPermissions() PermissionsModel {
	out := PermissionsModel{DefaultMode: "acceptEdits", Allow: []string{}, Deny: []string{}}
	raw, err := ReadRaw()
	if err != nil {
		return out
	}
	perm, _ := raw["permissions"].(map[string]any)
	if mode, ok := perm["defaultMode"].(string); ok && mode != "" {
		out.DefaultMode = mode
	}
	out.Allow = stringSlice(perm["allow"])
	out.Deny = stringSlice(perm["deny"])
	return out
}

// SavePermissions 保存权限配置。
func SavePermissions(p PermissionsModel) error {
	path, _ := SettingsPath()
	var mtime *time.Time
	if info, err := os.Stat(path); err == nil {
		t := info.ModTime()
		mtime = &t
	}
	raw, err := ReadRaw()
	if err != nil {
		return err
	}
	raw["permissions"] = map[string]any{
		"defaultMode": p.DefaultMode,
		"allow":       p.Allow,
		"deny":        p.Deny,
	}
	return WriteRaw(raw, mtime)
}

// ReadModels 从 env 里取模型相关字段。
func ReadModels() map[string]string {
	out := map[string]string{}
	entries, _ := ReadEnv()
	for _, e := range entries {
		out[e.Key] = e.Value
	}
	return out
}

// SaveModels 把模型字段写回 env，保留其它环境变量。
func SaveModels(models map[string]string) error {
	entries, _ := ReadEnv()
	index := map[string]int{}
	for i, e := range entries {
		index[e.Key] = i
	}
	for k, v := range models {
		if i, ok := index[k]; ok {
			entries[i].Value = v
		} else {
			entries = append(entries, model.EnvEntry{Key: k, Value: v})
		}
	}
	return SaveEnv(entries)
}

func stringSlice(v any) []string {
	arr, ok := v.([]any)
	if !ok {
		return []string{}
	}
	out := make([]string, 0, len(arr))
	for _, x := range arr {
		if s, ok := x.(string); ok {
			out = append(out, s)
		}
	}
	return out
}
