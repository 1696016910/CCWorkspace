# claude CLI 进程协议

后端（Rust）不依赖任何 SDK，直接管理 claude 子进程并解析 stdout。

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

### 7.1 CLI 没有直接给出「当前上下文多大」

实测 10 轮对话，两种候选口径都不合格：

| 口径 | 实测序列 | 问题 |
|---|---|---|
| `usage` 三项之和 | 77246 → 85823 → 94302 → **47490** → 47769 | 单轮值；缓存命中状态一变就回落，实测跳 4.7 个百分点 |
| `modelUsage` 三项 | 7.7% → 15.6% → 27.5% → 36.3% → **41.1%** | 会话累计值；把每轮输入累加，第 5 轮就虚报 41% |
| `modelUsage.inputTokens` 单取 | 6975 → 13380 → 20685 → 24756 | 只是「未命中缓存的部分」，不含 cache_read |

**没有一个字段等于「当前上下文大小」。** 这是 CLI 的接口限制，不是实现缺陷。

### 7.2 采用的方案

**优先用 `/context` 的权威值，否则用本轮 usage 的输入三项之和。**

```
# 1. 若本轮输出是 /context 的结果，直接解析这一行
**Tokens:** 21.2k / 1m (2%)
   → used = 21200, max = 1000000, pct = 2.1%

# 2. 否则
used = usage.input_tokens + cache_read_input_tokens + cache_creation_input_tokens
pct  = clamp(used / contextWindow * 100, 0, 100)
```

不含 `output_tokens`：输出还没进上下文。

### 7.3 不要做「历史峰值」平滑

曾经为了消除缓存抖动导致的回落，做过「记住历史最大值、只增不减」。**这是错的。**

真实案例：某轮读 PDF 把上下文撑到 142k，之后 CLI 清理了内容实际只剩 21k，
而峰值让界面一直显示 14%（Claude 自己报 3%），差了 4 倍。

上下文是会被清理和压缩的，**回落是真实的**，必须如实反映。

### 7.4 `/compact` 也是本地命令

实测 `subtype = success`、`is_error = false`，但 `result` 是**空字符串**
（响应里有 `local_command: "compact"`）。它只做压缩，不产摘要文本。

把空文本当失败会导致每次都报「压缩没有返回摘要」。
压缩后上下文会真的变小（实测 142k → 45k），但新的准确值要等下一轮对话才能拿到。

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

## 9. 实测补充（0.2.2 修复期）

### 9.1 图片内容块可用

`content` 数组里除 `text` 之外，可以放标准 API 格式的 image 块：

```json
{"type":"image","source":{"type":"base64","media_type":"image/png","data":"<base64>"}}
```

实测：发一张 64×64 纯蓝 PNG，模型回答「蓝色」，`result.is_error = false`。
字段名与 Claude API 一致，不需要额外转换。

**注意**：整行 NDJSON 是一次性写入 stdin 的，图片越大这行越长。
`invoke` 传 base64 与 CLI 读 stdin 两处都有实际上限，超限时 CLI 会返回 `is_error`，
界面上的表现是「发送失败」——所以失败文案必须点明与附件有关。

### 9.2 init 事件带完整能力清单

`system/init` 里除 `session_id` 外还有：

| 字段 | 类型 | 实测数量 | 说明 |
|---|---|---|---|
| `slash_commands` | string[] | 143 | 全部可用 `/` 命令 |
| `skills` | string[] | 65 | Skill 名单（上面那个的子集） |
| `agents` | string[] | 20 | Agent 名单 |
| `plugins` | object[] | 21 | `{name, path, source, version}` |
| `terminal_slash_commands` | string[] | 4 | 只能在终端用 |
| `mcp_servers` | object[] | 4 | `{name, status, source}` |

**`slash_commands` 是超集**：用户的 Skill、用户命令、项目命令（以 `dir:name` 形式）、
插件命令与 CLI 内置命令全在里面。扫目录只能拿到子集，拿不到内置命令。

**但只有名字没有描述。** 描述要从本地文件补：Skill 取 frontmatter 的 `description`，
命令取首段文字，内置命令补不到就留空。

**时机**：init 只在发消息之后到达。首屏空会话时是空的。

### 9.3 停止必须杀整棵进程树

Windows 下命令链是 `cmd.exe /d /s /c claude` → `node` → 工具子进程。
`child.kill()` 只杀最外层 `cmd.exe`，`node` 会存活并继续往管道写。

实测对比：

```
裸 kill：  杀掉 cmd 后 node 仍存活  → 内容在几秒后一起涌出来
taskkill /T /F：cmd 与 node 都终止  → 停止立即生效
```

### 9.4 30 秒空闲超时

原文档第 5 节写了这条但代码里没有。现已实现：连续 30 秒没有
`init` / `stream_event` / `assistant` / `result` 中任意一类事件，就终止进程并报超时。

hook 事件**不计入**——挂住的 hook 会不断刷新计时器，让超时永远触发不了。

实现方式是读线程 + `recv_timeout(100ms)` 轮询：直接 `for line in reader.lines()`
是阻塞的，模型思考期间不产出任何行，停止标志永远读不到。
