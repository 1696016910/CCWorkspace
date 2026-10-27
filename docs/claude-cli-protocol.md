# claude CLI 进程协议

三份实现（Node / Go / Rust）共用这套协议。Go 与 Rust 不依赖任何 SDK，直接管理子进程并解析 stdout。

> 本文档的每一条都来自实测，不是推测。样本见文末。

## 1. 启动

```bash
claude --print \
       --input-format stream-json \
       --output-format stream-json \
       --verbose \
       --include-partial-messages \
       --permission-mode <mode> \
       [--resume <session-id>] \
       [--model <model-name>]
```

| 参数 | 作用 |
|---|---|
| `--print` | 非交互模式，处理完一条输入就退出 |
| `--input-format stream-json` | stdin 接受 NDJSON，每行一个 JSON |
| `--output-format stream-json` | stdout 输出 NDJSON |
| `--verbose` | 输出完整事件（不加会丢失中间事件） |
| `--include-partial-messages` | 输出 `stream_event` 增量，这是流式输出的来源 |
| `--permission-mode` | `default` / `acceptEdits` / `plan` / `dontAsk` / `bypassPermissions` |
| `--resume` | 续接已有 CLI 会话，实现多轮上下文 |

**工作目录**设为会话所属工作区的路径。
**环境变量**为 `process.env` 叠加 `~/.claude/settings.json` 的 `env` 字段。

## 2. 输入格式

stdin 写入单个 NDJSON 行：

```json
{"type":"user","message":{"role":"user","content":[{"type":"text","text":"用户输入"}]}}
```

写完这一行后**不要立即关闭 stdin**，否则可能错过输出（实测：管道关闭过快时只能收到 hook 事件）。

## 3. 输出事件

stdout 逐行输出 JSON。实测一次完整对话产生 19 个事件：

| `type` | `subtype` | 数量 | 含义 | 处理方式 |
|---|---|---|---|---|
| `system` | `hook_started` | 4 | 会话启动 hook 开始 | **忽略** |
| `system` | `hook_response` | 4 | hook 输出 | **忽略**（内容可达数万字符） |
| `system` | `hook_progress` | 1 | hook 进度 | **忽略** |
| `system` | `init` | 1 | 会话初始化 | 记录 `session_id`、`slash_commands`、`model` |
| `system` | `status` | 1 | 状态变更 | 忽略或显示为"处理中" |
| `stream_event` | — | 6 | 逐字符增量 | 提取 `event.delta.text`（仅 `content_block_delta`） |
| `assistant` | — | 1 | 完整消息块 | 提取 `message.content` 里的 `text` 与 `tool_use` |
| `result` | `success` | 1 | 本轮结束 | 取 `usage`、`total_cost_usd`、`modelUsage` |

### 3.1 stream_event 的结构

```json
{
  "type": "stream_event",
  "event": {
    "type": "content_block_delta",
    "delta": { "type": "text_delta", "text": "O" }
  }
}
```

**只有 `event.type === "content_block_delta"` 且 `event.delta.type === "text_delta"` 时才取 `event.delta.text`。** 其他 delta 类型（`thinking_delta`、`input_json_delta`）不产出可见文本。

### 3.2 assistant 的结构

```json
{
  "type": "assistant",
  "message": {
    "model": "deepseek-v4.1-flash",
    "content": [
      { "type": "text", "text": "..." },
      { "type": "tool_use", "name": "Read", "input": { "file_path": "/a/b.ts" } }
    ]
  },
  "parent_tool_use_id": null
}
```

`parent_tool_use_id` 非空表示这条消息来自 Task 子代理。

### 3.3 result 的结构

```json
{
  "type": "result",
  "subtype": "success",
  "is_error": false,
  "usage": { "input_tokens": 37866, "output_tokens": 2,
             "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0 },
  "total_cost_usd": 0.0123,
  "modelUsage": { "deepseek-v4.1-flash": { "contextWindow": 1000000 } }
}
```

## 4. 必须过滤的两类事件

### 4.1 hook 事件

`system/hook_started`、`system/hook_response`、`system/hook_progress`。

实测某些机器上 SessionStart hook 会输出数万字符（Skill 内容注入），如果不过滤会：
- 把界面刷屏
- 拖慢 JSON 解析
- 让"是否收到有效响应"的判定失效

**过滤条件**：`type === "system" && subtype.startsWith("hook_")`。

### 4.2 子代理事件

`stream_event` 与 `assistant` 都带 `parent_tool_use_id`。非空表示该事件由 Task 工具启动的子代理产生。

**过滤条件**：`parent_tool_use_id != null` 时跳过。

不过滤的后果：子代理的中间输出混进主回复，保存的消息变成"子代理文本 + 主模型回答"的混合物。

## 5. 错误分类

| 现象 | 判定 | 提示文案 |
|---|---|---|
| `result.is_error === true` | 模型侧失败 | 取 `result.errors[]`；空则按 `subtype` 翻译 |
| `subtype === "error_max_turns"` | 超过轮次上限 | 已达到本轮最大轮次限制 |
| 30 秒无有效事件 | 连接超时 | 检查网络与 ANTHROPIC_BASE_URL 配置 |
| 迭代结束但未收到 `result` | CLI 异常退出 | 模型没有返回任何内容，请重试 |
| 某行不是合法 JSON | 协议异常 | 跳过该行，继续读 |

**"有效事件"的定义**：`assistant` / `stream_event` / `result` / `system:init`。
hook 事件**不算**——否则挂住的 hook 会一直刷新超时计时器，让超时永远不触发。

## 6. 中断

向子进程发送终止信号（Go：`cmd.Process.Kill()`；Rust：`child.kill()`）。

中断后：
- 已收到的增量文本**保留**，写入数据库并标记 `stopped = 1`
- 界面显示「已停止生成」
- 不要因为中断就丢弃这一轮的部分内容

## 7. 上下文用量

两个来源，优先级不同：

| 来源 | 字段 | 性质 |
|---|---|---|
| `assistant.context_usage` | `total_tokens` / `raw_max_tokens` / `percentage` | **权威值**，CLI 自己算的 |
| `result.usage` | `input_tokens` + `output_tokens` + cache 字段 | 推算值，仅作兜底 |

**两套口径不能混写同一个字段**，否则百分比会在 50% 与 5% 之间跳变。

推算值为：
```
used = input_tokens + cache_read_input_tokens + cache_creation_input_tokens + output_tokens
pct  = used / modelUsage[model].contextWindow * 100
```

分母取 `modelUsage` 中 `contextWindow` 最大的那条（多模型时选主模型的窗口）。

## 8. 实测样本

命令：

```bash
printf '%s\n' '{"type":"user","message":{"role":"user","content":[{"type":"text","text":"reply with exactly: OK"}]}}' \
  | claude --print --input-format stream-json --output-format stream-json \
           --verbose --include-partial-messages --permission-mode acceptEdits
```

输出 19 行，事件序列：

```
system/hook_started    × 4
system/hook_response   × 4
system/hook_progress   × 1
system/init            × 1
system/status          × 1
stream_event           × 6   (其中一条 delta = "OK")
assistant              × 1   (model = deepseek-v4.1-flash)
result/success         × 1   (is_error = false, input_tokens = 37866, output_tokens = 2)
```

注意 `input_tokens = 37866` 是**首次会话的系统提示开销**，包含所有 Skill 定义与工具 schema。
这解释了为什么新会话一开就占用数万 token。
