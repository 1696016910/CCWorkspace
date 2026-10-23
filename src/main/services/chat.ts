import { BrowserWindow } from 'electron'
import type { ChatMessage, ContextUsageDetail, SlashCommandInfo, ToolCall } from '../../shared/types'
import { appendMessage, getClaudeSessionId, setClaudeSessionId, setContextUsage } from './session'
import { readModels, readPermissions, readRaw } from './claudeSettings'
import { resolveClaudeBinary } from './binary'

export type ChatEvent =
  | { type: 'start'; sessionId: string }
  | { type: 'text'; sessionId: string; delta: string }
  | { type: 'tool'; sessionId: string; tool: ToolCall }
  | { type: 'done'; sessionId: string; message: ChatMessage; usage: Usage }
  | { type: 'error'; sessionId: string; message: string }
  | { type: 'interrupted'; sessionId: string }
  | { type: 'compacted'; sessionId: string }

export interface Usage {
  inputTokens: number
  outputTokens: number
  costUsd: number
  /** 上下文窗口占用百分比 */
  percentage: number
}

interface ActiveRun {
  abort: () => void
}

/** SDK 的 Query 句柄，用于查询上下文与命令列表 */
interface QueryHandle {
  getContextUsage?: (opts?: { detail?: 'summary' | 'full' }) => Promise<{
    model: string
    total_tokens: number
    raw_max_tokens: number
    percentage: number
    categories?: { name?: string; tokens?: number; kind?: string }[]
  }>
  supportedCommands?: () => Promise<
    { name: string; description: string; argumentHint: string; builtin?: boolean }[]
  >
}

let active: ActiveRun | null = null
/** 最近一次运行留下的 Query，仅在迭代期间可查询 */
let lastQuery: QueryHandle | null = null
/** 最后结束那一轮抓到的上下文快照，按会话 id 隔离 */
const ctxSnapshots = new Map<
  string,
  {
    model: string
    total_tokens: number
    raw_max_tokens: number
    percentage: number
    categories?: { name?: string; tokens?: number; kind?: string }[]
  }
>()
/** 首轮抓到的命令与 Skill 列表 */
let commandsCache: SlashCommandInfo[] = []

function emit(event: ChatEvent): void {
  for (const win of BrowserWindow.getAllWindows()) {
    win.webContents.send('chat:event', event)
  }
}

export function isRunning(): boolean {
  return active !== null
}

export function interrupt(): void {
  active?.abort()
  active = null
}

/** 把一条用户消息包成 SDK 的流式输入项 */
async function* oneShot(text: string): AsyncGenerator<{
  type: 'user'
  message: { role: 'user'; content: string }
  parent_tool_use_id: null
}> {
  yield {
    type: 'user',
    message: { role: 'user', content: text },
    parent_tool_use_id: null
  }
}

/** 从流式增量事件中取出文本片段 */
function deltaText(event: unknown): string | null {
  const e = event as { type?: string; delta?: { type?: string; text?: string } }
  if (e?.type === 'content_block_delta' && e.delta?.type === 'text_delta') {
    return e.delta.text ?? null
  }
  return null
}

export async function send(
  sessionId: string,
  text: string,
  cwd: string,
  skills: string[]
): Promise<void> {
  if (active) throw new Error('已有回复正在生成，请先停止')

  // 后续轮次用 CLI 会话 id 续接，否则每轮都是全新上下文
  const resumeId = getClaudeSessionId(sessionId) ?? undefined

  appendMessage(sessionId, 'user', text)
  emit({ type: 'start', sessionId })

  let acc = ''
  const tools: ToolCall[] = []
  let aborted = false
  let usage: Usage = { inputTokens: 0, outputTokens: 0, costUsd: 0, percentage: 0 }

  try {
    const { query } = await import('@anthropic-ai/claude-agent-sdk')
    const raw = await readRaw()
    const envOverrides = raw.env && typeof raw.env === 'object' ? raw.env : {}
    const models = await readModels()
    const model = models['ANTHROPIC_MODEL'] || undefined
    const perms = await readPermissions()
    const allowed = ['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions'] as const
    const mode = (allowed as readonly string[]).includes(perms.defaultMode)
      ? (perms.defaultMode as (typeof allowed)[number])
      : 'acceptEdits'

    const q = query({
      prompt: oneShot(text),
      options: {
        cwd,
        resume: resumeId,
        env: { ...process.env, ...envOverrides } as Record<string, string>,
        model,
        permissionMode: mode,
        // 开启流式增量，否则只能等整段回复结束
        includePartialMessages: true,
        pathToClaudeCodeExecutable: resolveClaudeBinary(),
        skills: skills.length > 0 ? skills : undefined
      }
    })

    lastQuery = q as unknown as QueryHandle

    active = {
      abort: () => {
        aborted = true
        void q.interrupt()
      }
    }

    for await (const message of q) {
      if (aborted) break

      // 流式增量：逐字推给界面
      if (message.type === 'stream_event') {
        const delta = deltaText((message as unknown as { event: unknown }).event)
        if (delta) {
          acc += delta
          emit({ type: 'text', sessionId, delta })
        }
        continue
      }

      if (message.type === 'assistant') {
        const blocks = (
          message as unknown as { message?: { content?: Array<Record<string, unknown>> } }
        ).message?.content
        if (!Array.isArray(blocks)) continue

        for (const block of blocks) {
          // 文本已在 stream_event 中逐字推送过，这里只补漏（无流式时）
          if (block['type'] === 'text' && typeof block['text'] === 'string') {
            const full = block['text']
            if (!acc) {
              acc = full
              emit({ type: 'text', sessionId, delta: full })
            }
          } else if (block['type'] === 'tool_use') {
            const input = (block['input'] ?? {}) as Record<string, unknown>
            const target =
              (typeof input['file_path'] === 'string' && input['file_path']) ||
              (typeof input['command'] === 'string' && input['command']) ||
              (typeof input['pattern'] === 'string' && input['pattern']) ||
              ''
            const tool: ToolCall = { name: String(block['name'] ?? ''), target: String(target) }
            tools.push(tool)
            emit({ type: 'tool', sessionId, tool })
          }
        }
      } else if (message.type === 'system' && message.subtype === 'init') {
        // 首次轮次记住 CLI 会话 id
        const init = message as unknown as {
          session_id?: string
          slash_commands?: string[]
          skills?: string[]
        }
        if (init.session_id) setClaudeSessionId(sessionId, init.session_id)
        // init 消息自带命令与 Skill 清单，无需额外查询
        if (init.slash_commands?.length) {
          commandsCache = init.slash_commands.map((name) => ({
            name,
            description: '',
            argumentHint: '',
            builtin: true
          }))
        }
      } else if (message.type === 'result') {
        const r = message as unknown as {
          session_id?: string
          usage?: { input_tokens?: number; output_tokens?: number }
          total_cost_usd?: number
          modelUsage?: Record<string, { contextWindow?: number; inputTokens?: number }>
        }
        if (r.session_id) setClaudeSessionId(sessionId, r.session_id)

        // 上下文窗口从 modelUsage 里取；input_tokens 即本轮结束时占用的上下文量
        let maxTokens = 0
        let windowModel = ''
        for (const [name, info] of Object.entries(r.modelUsage ?? {})) {
          if (info.contextWindow && info.contextWindow > maxTokens) {
            maxTokens = info.contextWindow
            windowModel = name
          }
        }
        const used = (r.usage?.input_tokens ?? 0) + (r.usage?.output_tokens ?? 0)
        const pct = maxTokens > 0 ? Math.min(999, Math.round((used / maxTokens) * 1000) / 10) : 0

        usage = {
          inputTokens: r.usage?.input_tokens ?? 0,
          outputTokens: r.usage?.output_tokens ?? 0,
          costUsd: r.total_cost_usd ?? 0,
          percentage: pct
        }

        ctxSnapshots.set(sessionId, {
          model: windowModel || model || '',
          total_tokens: used,
          raw_max_tokens: maxTokens,
          percentage: pct,
          categories: []
        })
        setContextUsage(sessionId, used, maxTokens, pct)
      }
    }

    // 一轮结束后补一次上下文百分比，并持久化到会话
    try {
      if (lastQuery?.getContextUsage) {
        const ctx = await lastQuery.getContextUsage({ detail: 'summary' })
        usage.percentage = ctx.percentage
        setContextUsage(sessionId, ctx.total_tokens, ctx.raw_max_tokens, ctx.percentage)
      }
    } catch {
      // 用量查询失败不影响本轮结果
    }
  } catch (e) {
    active = null
    emit({ type: 'error', sessionId, message: e instanceof Error ? e.message : String(e) })
    return
  }

  active = null

  const saved = appendMessage(
    sessionId,
    'assistant',
    acc || (aborted ? '（已停止）' : ''),
    tools.length ? tools : undefined
  )

  if (aborted) emit({ type: 'interrupted', sessionId })
  emit({ type: 'done', sessionId, message: saved, usage })
}

/**
 * 读取上下文用量明细。
 *
 * query 在迭代结束后会关闭，此时再调用 getContextUsage 会报
 * "Query closed before response received"，因此这里优先返回
 * 上一轮结束时抓到的快照。
 */
export async function getContextUsage(sessionId: string): Promise<ContextUsageDetail | null> {
  const ctx = ctxSnapshots.get(sessionId)
  if (!ctx) return null
  return {
    model: ctx.model,
    totalTokens: ctx.total_tokens,
    maxTokens: ctx.raw_max_tokens,
    percentage: ctx.percentage,
    categories: (ctx.categories ?? []).map(
      (c: { name?: string; tokens?: number; kind?: string }) => ({
        name: c.name ?? '',
        tokens: c.tokens ?? 0,
        kind: c.kind ?? 'used'
      })
    )
  }
}

/** 列出当前会话可用的斜杠命令与 Skill；同样只能用迭代期间抓到的缓存 */
export async function listCommands(_sessionId?: string): Promise<SlashCommandInfo[]> {
  return commandsCache
}

/**
 * 压缩上下文。
 * SDK 没有独立的 compact 接口，用官方约定的 /compact 命令触发，
 * 由 CLI 自身完成摘要并产生 compact_boundary 消息。
 */
export async function compact(sessionId: string, cwd: string, skills: string[]): Promise<void> {
  await send(sessionId, '/compact', cwd, skills)
  emit({ type: 'compacted', sessionId })
}
