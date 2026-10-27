// Package claude 管理 claude 子进程，按 docs/claude-cli-protocol.md 解析输出。
//
// 不依赖任何 SDK：拼参数、spawn、写 stdin、逐行读 stdout。
package claude

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"
	"time"
)

// 单轮安全上限，与 Node 版一致
const (
	MaxOutputChars = 400_000
	MaxTools       = 500
)

// Event 是发给 UI 的统一事件。
type Event struct {
	Type      string
	SessionID string
	Delta     string
	Tool      *ToolCall
	Text      string
	Ctx       *ContextUsage
	Usage     *Usage
	Error     string
	ClaudeSID string
	Commands  []string
}

// ToolCall 一次工具调用。
type ToolCall struct {
	Name   string `json:"name"`
	Target string `json:"target"`
}

// Usage 本轮 token 与费用。
type Usage struct {
	InputTokens  int
	OutputTokens int
	CostUSD      float64
	Percentage   float64
}

// ContextUsage 上下文明细。
type ContextUsage struct {
	Model       string
	TotalTokens int
	MaxTokens   int
	Percentage  float64
	Categories  []Category
}

// Category /context 的分类明细。
type Category struct {
	Name   string
	Tokens int
	Kind   string
}

// Options 启动一轮对话的参数。
type Options struct {
	Cwd   string
	Text  string
	Model string
	// Resume 非空时续接该 CLI 会话
	Resume string
	// Mode 权限模式，空则用 acceptEdits
	Mode string
	// Env 追加的环境变量（来自 settings.json 的 env）
	Env map[string]string
}

// Run 启动一轮对话，把事件逐个推给 emit。
// 返回时表示这一轮已结束（正常、出错或中断）。
func Run(opts Options, emit func(Event)) error {
	bin := "claude"
	if p := lookupClaude(); p != "" {
		bin = p
	}

	args := []string{
		"--print",
		"--input-format", "stream-json",
		"--output-format", "stream-json",
		"--verbose",
		"--include-partial-messages",
	}
	mode := opts.Mode
	if mode == "" {
		mode = "acceptEdits"
	}
	args = append(args, "--permission-mode", mode)
	if opts.Resume != "" {
		args = append(args, "--resume", opts.Resume)
	}
	if opts.Model != "" {
		args = append(args, "--model", opts.Model)
	}

	cmd := exec.Command(bin, args...)
	cmd.Dir = opts.Cwd
	cmd.Env = mergeEnv(opts.Env)

	stdin, err := cmd.StdinPipe()
	if err != nil {
		return fmt.Errorf("打开 stdin 失败: %w", err)
	}
	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return fmt.Errorf("打开 stdout 失败: %w", err)
	}
	cmd.Stderr = io.Discard

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("启动 claude 失败（确认已在 PATH 中）: %w", err)
	}

	// 写一行 NDJSON。注意不能立刻关 stdin：
	// 实测管道关闭过快时只能收到 hook 事件，拿不到真正回复。
	payload := map[string]any{
		"type": "user",
		"message": map[string]any{
			"role": "user",
			"content": []any{
				map[string]any{"type": "text", "text": opts.Text},
			},
		},
	}
	buf, _ := json.Marshal(payload)
	if _, err := stdin.Write(append(buf, '\n')); err != nil {
		_ = cmd.Process.Kill()
		return fmt.Errorf("写入 stdin 失败: %w", err)
	}

	stop := make(chan struct{})
	go func() {
		select {
		case <-stop:
		case <-time.After(10 * time.Minute):
			_ = cmd.Process.Kill()
		}
	}()
	defer close(stop)

	s := &state{emit: emit, acc: strings.Builder{}}
	scanErr := s.consume(stdout)
	_ = stdin.Close()
	_ = cmd.Wait()

	if scanErr != nil {
		return scanErr
	}
	return s.finish()
}

// state 单轮解析状态。
type state struct {
	emit     func(Event)
	acc      strings.Builder
	tools    []ToolCall
	sawEvent bool
	runErr   string
	lastAt   time.Time
}

// consume 逐行读取并分派事件。
func (s *state) consume(r io.Reader) error {
	sc := bufio.NewScanner(r)
	sc.Buffer(make([]byte, 0, 64*1024), 8*1024*1024) // hook 输出可能很大

	for sc.Scan() {
		line := sc.Bytes()
		if len(line) == 0 || line[0] != '{' {
			continue // 非 JSON 行直接跳过
		}
		var msg map[string]any
		if err := json.Unmarshal(line, &msg); err != nil {
			continue // 协议异常，跳过该行
		}
		s.handle(msg)
	}
	return sc.Err()
}

func (s *state) handle(msg map[string]any) {
	typ, _ := msg["type"].(string)
	sub, _ := msg["subtype"].(string)

	// hook 事件必须整类丢掉：实测 SessionStart hook 可吐出数万字符，
	// 不过滤会刷屏，还会让"是否收到有效响应"的判定永远成立。
	if typ == "system" && strings.HasPrefix(sub, "hook_") {
		return
	}

	// 子代理的事件不能混进主回复
	if pt, ok := msg["parent_tool_use_id"]; ok && pt != nil {
		return
	}

	switch typ {
	case "system":
		if sub == "init" {
			s.sawEvent = true
			s.lastAt = time.Now()
			if sid, ok := msg["session_id"].(string); ok {
				s.emit(Event{Type: "claudeSession", ClaudeSID: sid})
			}
			if cmds, ok := msg["slash_commands"].([]any); ok {
				names := make([]string, 0, len(cmds))
				for _, c := range cmds {
					if cs, ok := c.(string); ok {
						names = append(names, cs)
					}
				}
				if len(names) > 0 {
					s.emit(Event{Type: "commands", Commands: names})
				}
			}
		}
	case "stream_event":
		s.sawEvent = true
		s.lastAt = time.Now()
		if d := deltaText(msg); d != "" && s.acc.Len() < MaxOutputChars {
			s.acc.WriteString(d)
			s.emit(Event{Type: "text", Delta: d})
		}
	case "assistant":
		s.sawEvent = true
		s.lastAt = time.Now()
		s.handleAssistant(msg)
	case "result":
		s.sawEvent = true
		s.lastAt = time.Now()
		s.handleResult(msg)
	}
}

func (s *state) handleAssistant(msg map[string]any) {
	inner, _ := msg["message"].(map[string]any)
	blocks, _ := inner["content"].([]any)
	for _, b := range blocks {
		block, ok := b.(map[string]any)
		if !ok {
			continue
		}
		switch block["type"] {
		case "text":
			t, _ := block["text"].(string)
			// stream_event 已经累积过就不重复加，避免整段变两倍
			if t != "" && s.acc.Len() == 0 {
				s.acc.WriteString(t)
				s.emit(Event{Type: "text", Delta: t})
			}
		case "tool_use":
			if len(s.tools) >= MaxTools {
				continue
			}
			name, _ := block["name"].(string)
			input, _ := block["input"].(map[string]any)
			target := pickTarget(input)
			tc := ToolCall{Name: name, Target: target}
			s.tools = append(s.tools, tc)
			s.emit(Event{Type: "tool", Tool: &tc})
		}
	}
}

func (s *state) handleResult(msg map[string]any) {
	if isErr, _ := msg["is_error"].(bool); isErr {
		if errs, ok := msg["errors"].([]any); ok && len(errs) > 0 {
			parts := make([]string, 0, len(errs))
			for _, e := range errs {
				if es, ok := e.(string); ok && es != "" {
					parts = append(parts, es)
				}
			}
			s.runErr = strings.Join(parts, "；")
		}
		if s.runErr == "" {
			if sub, _ := msg["subtype"].(string); sub == "error_max_turns" {
				s.runErr = "已达到本轮最大轮次限制"
			} else {
				s.runErr = "模型返回了错误结果"
			}
		}
	}

	u, _ := msg["usage"].(map[string]any)
	usage := &Usage{
		InputTokens:  intOf(u, "input_tokens"),
		OutputTokens: intOf(u, "output_tokens"),
		CostUSD:      floatOf(msg, "total_cost_usd"),
	}
	cached := intOf(u, "cache_read_input_tokens") + intOf(u, "cache_creation_input_tokens")
	used := usage.InputTokens + cached + usage.OutputTokens

	maxTokens := 0
	model := ""
	if mu, ok := msg["modelUsage"].(map[string]any); ok {
		for name, info := range mu {
			m, ok := info.(map[string]any)
			if !ok {
				continue
			}
			if cw := intOf(m, "contextWindow"); cw > maxTokens {
				maxTokens = cw
				model = name
			}
		}
	}
	if maxTokens > 0 {
		usage.Percentage = round1(float64(used) / float64(maxTokens) * 100)
	}

	s.emit(Event{Type: "usage", Usage: usage})
	if maxTokens > 0 {
		s.emit(Event{Type: "context", Ctx: &ContextUsage{
			Model:       model,
			TotalTokens: used,
			MaxTokens:   maxTokens,
			Percentage:  usage.Percentage,
		}})
	}
}

// finish 收尾：把累积结果或错误交给上层。
func (s *state) finish() error {
	if s.runErr != "" {
		s.emit(Event{Type: "error", Error: s.runErr})
		return nil
	}
	if s.acc.Len() == 0 && len(s.tools) == 0 {
		s.emit(Event{Type: "error", Error: "模型没有返回任何内容，请重试"})
		return nil
	}
	s.emit(Event{Type: "done", Text: s.acc.String(), Tool: nil})
	return nil
}

// deltaText 从 stream_event 里取出 text_delta。
func deltaText(msg map[string]any) string {
	ev, ok := msg["event"].(map[string]any)
	if !ok {
		return ""
	}
	if t, _ := ev["type"].(string); t != "content_block_delta" {
		return ""
	}
	d, ok := ev["delta"].(map[string]any)
	if !ok {
		return ""
	}
	if t, _ := d["type"].(string); t != "text_delta" {
		return ""
	}
	txt, _ := d["text"].(string)
	return txt
}

func pickTarget(input map[string]any) string {
	for _, k := range []string{"file_path", "command", "pattern", "path", "url"} {
		if v, ok := input[k].(string); ok && v != "" {
			if len(v) > 500 {
				v = v[:500]
			}
			return v
		}
	}
	return ""
}

// lookupClaude 在 PATH 里找 claude 可执行文件。
func lookupClaude() string {
	p, err := exec.LookPath("claude")
	if err != nil {
		return ""
	}
	return p
}

// mergeEnv 把额外环境变量并到当前进程环境里。
func mergeEnv(extra map[string]string) []string {
	env := os.Environ()
	for k, v := range extra {
		env = append(env, k+"="+v)
	}
	return env
}

func intOf(m map[string]any, key string) int {
	if m == nil {
		return 0
	}
	if v, ok := m[key].(float64); ok {
		return int(v)
	}
	return 0
}

func floatOf(m map[string]any, key string) float64 {
	if m == nil {
		return 0
	}
	if v, ok := m[key].(float64); ok {
		return v
	}
	return 0
}

func round1(f float64) float64 {
	return float64(int(f*10+0.5)) / 10
}
