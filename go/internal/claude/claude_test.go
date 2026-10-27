package claude

import (
	"strings"
	"testing"
)

// feed 把样本行喂进 state，返回收集到的事件。
func feed(lines []string) []Event {
	var out []Event
	s := &state{emit: func(e Event) { out = append(out, e) }}
	s.consume(strings.NewReader(strings.Join(lines, "\n")))
	s.finish()
	return out
}

func countType(evs []Event, typ string) int {
	n := 0
	for _, e := range evs {
		if e.Type == typ {
			n++
		}
	}
	return n
}

func TestFiltersHookEvents(t *testing.T) {
	evs := feed([]string{
		`{"type":"system","subtype":"hook_started","hook_name":"SessionStart"}`,
		`{"type":"system","subtype":"hook_response","output":"` + strings.Repeat("x", 50000) + `"}`,
		`{"type":"system","subtype":"hook_progress"}`,
		`{"type":"system","subtype":"init","session_id":"s1","slash_commands":["help"]}`,
		`{"type":"result","subtype":"success","is_error":false,"usage":{"input_tokens":10,"output_tokens":2}}`,
	})
	if n := countType(evs, "text"); n != 0 {
		t.Errorf("hook 事件不应产出 text，实际 %d 条", n)
	}
	if n := countType(evs, "claudeSession"); n != 1 {
		t.Errorf("init 应产出 1 条 claudeSession，实际 %d", n)
	}
	if n := countType(evs, "commands"); n != 1 {
		t.Errorf("init 应产出 1 条 commands，实际 %d", n)
	}
}

func TestFiltersSubagentEvents(t *testing.T) {
	evs := feed([]string{
		`{"type":"stream_event","parent_tool_use_id":"task-1","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"子代理文本"}}}`,
		`{"type":"assistant","parent_tool_use_id":"task-1","message":{"content":[{"type":"text","text":"子代理完整"}]}}`,
		`{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"主回复"}}}`,
		`{"type":"result","is_error":false,"usage":{"input_tokens":5,"output_tokens":1}}`,
	})
	var got strings.Builder
	for _, e := range evs {
		if e.Type == "text" {
			got.WriteString(e.Delta)
		}
	}
	if got.String() != "主回复" {
		t.Errorf("应只保留主回复，实际 %q", got.String())
	}
}

func TestStreamDelta(t *testing.T) {
	evs := feed([]string{
		`{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"你"}}}`,
		`{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"好"}}}`,
		`{"type":"result","is_error":false,"usage":{"input_tokens":5,"output_tokens":1}}`,
	})
	var got strings.Builder
	for _, e := range evs {
		if e.Type == "text" {
			got.WriteString(e.Delta)
		}
	}
	if got.String() != "你好" {
		t.Errorf("应累积为 你好，实际 %q", got.String())
	}
}

func TestIgnoresThinkingDelta(t *testing.T) {
	evs := feed([]string{
		`{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"thinking_delta","thinking":"内部思考"}}}`,
		`{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"可见"}}}`,
		`{"type":"result","is_error":false,"usage":{"input_tokens":5,"output_tokens":1}}`,
	})
	var got strings.Builder
	for _, e := range evs {
		if e.Type == "text" {
			got.WriteString(e.Delta)
		}
	}
	if got.String() != "可见" {
		t.Errorf("thinking_delta 不应产出文本，实际 %q", got.String())
	}
}

func TestToolCalls(t *testing.T) {
	evs := feed([]string{
		`{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/a/b.ts"}}]}}`,
		`{"type":"result","is_error":false,"usage":{"input_tokens":5,"output_tokens":1}}`,
	})
	var names []string
	for _, e := range evs {
		if e.Type == "tool" && e.Tool != nil {
			names = append(names, e.Tool.Name+"→"+e.Tool.Target)
		}
	}
	if len(names) != 1 || names[0] != "Read→/a/b.ts" {
		t.Errorf("工具调用解析错误: %v", names)
	}
}

func TestIsErrorResult(t *testing.T) {
	evs := feed([]string{
		`{"type":"result","is_error":true,"errors":["密钥无效"]}`,
	})
	if n := countType(evs, "error"); n != 1 {
		t.Fatalf("is_error 应产出一条 error")
	}
	for _, e := range evs {
		if e.Type == "error" && !strings.Contains(e.Error, "密钥无效") {
			t.Errorf("错误信息应包含原始 errors 内容，实际 %q", e.Error)
		}
	}
}

func TestEmptyResult(t *testing.T) {
	evs := feed([]string{
		`{"type":"result","is_error":false,"usage":{"input_tokens":5,"output_tokens":0}}`,
	})
	if n := countType(evs, "error"); n != 1 {
		t.Errorf("无内容时应报错提示重试，实际 error 数 %d", n)
	}
}

func TestContextPercentage(t *testing.T) {
	evs := feed([]string{
		`{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"x"}}}`,
		`{"type":"result","is_error":false,"usage":{"input_tokens":1000,"output_tokens":0,"cache_read_input_tokens":9000},"modelUsage":{"m1":{"contextWindow":100000}}}`,
	})
	var pct float64
	for _, e := range evs {
		if e.Type == "context" && e.Ctx != nil {
			pct = e.Ctx.Percentage
		}
	}
	// (1000 + 9000 + 0) / 100000 = 10%
	if pct != 10 {
		t.Errorf("上下文百分比应为 10，实际 %v（cache token 漏算会让它变成 1）", pct)
	}
}

func TestIgnoresNonJSON(t *testing.T) {
	evs := feed([]string{
		`这不是 JSON`,
		``,
		`{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"ok"}}}`,
		`{"type":"result","is_error":false,"usage":{"input_tokens":5,"output_tokens":1}}`,
	})
	if n := countType(evs, "text"); n != 1 {
		t.Errorf("非 JSON 行应被跳过，正常行仍要处理")
	}
}
