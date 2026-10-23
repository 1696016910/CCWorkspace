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
  | { type: 'commands'; sessionId: string }

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

/** 发送前的同步校验，供 IPC 层即时返回错误 */
export function assertCanSend(text: string): void {
  if (!text.trim()) throw new Error('消息不能为空')
  if (active) throw new Error('已有回复正在生成，请先停止')
}

export async function send(
  sessionId: string,
  text: string,
  cwd: string,
  skills: string[]
): Promise<void> {
  assertCanSend(text)

  const resumeId = getClaudeSessionId(sessionId) ?? undefined

  appendMessage(sessionId, 'user', text)
  emit({ type: 'start', sessionId })

  let acc = ''
  const tools: ToolCall[] = []
  let aborted = false
  let sawAnyEvent = false
  let timedOut = false
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
        includePartialMessages: true,
        pathToClaudeCodeExecutable: resolveClaudeBinary(),
        skills: skills.length > 0 ? skills : undefined
      }
    })

    active = {
      abort: () => {
        aborted = true
        void q.interrupt().catch(() => {})
      }
    }

    const iterator = q[Symbol.asyncIterator]()

    while (!aborted) {
      const next = await Promise.race([
        iterator.next(),
        new Promise<{ done: true; value: undefined; timedOut: true }>((resolve) =>
          setTimeout(() => resolve({ done: true, value: undefined, timedOut: true }), 30_000)
        )
      ])
      if (next.done) {
        timedOut = 'timedOut' in next
        break
      }

      const message = next.value
      // hook_started / hook_response 等系统事件不代表模型已响应
      if (
        message.type === 'assistant' ||
        message.type === 'stream_event' ||
        message.type === 'result' ||
        (message.type === 'system' && message.subtype === 'init')
      ) {
        sawAnyEvent = true
      }

      if (message.type === 'stream_event') {
        const delta = deltaText((message as unknown as { event: unknown }).event)
        if (delta) {
          acc += delta
          emit({ type: 'text', sessionId, delta })
        }
        continue
      }

      if (message.type === 'assistant') {
        const withCtx = message as unknown as {
          context_usage?: {
            model?: string
            total_tokens?: number
            raw_max_tokens?: number
            percentage?: number
            categories?: { name?: string; tokens?: number; kind?: string }[]
          }
        }
        if (withCtx.context_usage) {
          const c = withCtx.context_usage
          ctxSnapshots.set(sessionId, {
            model: c.model ?? '',
            total_tokens: c.total_tokens ?? 0,
            raw_max_tokens: c.raw_max_tokens ?? 0,
            percentage: c.percentage ?? 0,
            categories: c.categories ?? []
          })
          setContextUsage(sessionId, c.total_tokens ?? 0, c.raw_max_tokens ?? 0, c.percentage ?? 0)
        }

        const blocks = (
          message as unknown as { message?: { content?: Array<Record<string, unknown>> } }
        ).message?.content
        if (!Array.isArray(blocks)) continue

        for (const block of blocks) {
          if (block['type'] === 'text' && typeof block['text'] === 'string') {
            if (!acc) {
              acc = block['text']
              emit({ type: 'text', sessionId, delta: acc })
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
        continue
      }

      if (message.type === 'system' && message.subtype === 'init') {
        const init = message as unknown as { session_id?: string; slash_commands?: string[] }
        if (init.session_id) setClaudeSessionId(sessionId, init.session_id)
        if (init.slash_commands?.length) {
          const live = q as unknown as QueryHandle
          let described: SlashCommandInfo[] = []
          try {
            if (typeof live.supportedCommands === 'function') {
              const list = await live.supportedCommands()
              described = list.map((c) => ({
                name: c.name,
                description: c.description,
                argumentHint: c.argumentHint,
                builtin: c.builtin === true
              }))
            }
          } catch {
            // 拿不到描述时退回只有名字
          }
          commandsCache =
            described.length > 0
              ? described
              : init.slash_commands.map((name) => ({
                  name,
                  description: '',
                  argumentHint: '',
                  builtin: true
                }))
          emit({ type: 'commands', sessionId })
        }
        continue
      }

      if (message.type === 'result') {
        const r = message as unknown as {
          session_id?: string
          usage?: { input_tokens?: number; output_tokens?: number }
          total_cost_usd?: number
          modelUsage?: Record<string, { contextWindow?: number; inputTokens?: number }>
        }
        if (r.session_id) setClaudeSessionId(sessionId, r.session_id)

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
  } catch (e) {
    active = null
    emit({ type: 'error', sessionId, message: e instanceof Error ? e.message : String(e) })
    return
  }

  active = null

  // 超时或从未收到有效响应：多半是服务不可达
  if ((timedOut || !sawAnyEvent) && !aborted) {
    emit({
      type: 'error',
      sessionId,
      message: '连接超时，请检查网络与 ANTHROPIC_BASE_URL 配置'
    })
    return
  }

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
 * 主动跑一次 /context 以取得分类明细。
 *
 * UI 打开上下文面板时用它取数据：不经过聊天流，
 * 因此不会把界面置为「生成中」，也不产生用户可见的消息。
 */
export async function refreshContext(
  sessionId: string,
  cwd: string,
  skills: string[]
): Promise<ContextUsageDetail | null> {
  if (active) return getContextUsage(sessionId)

  const existing = ctxSnapshots.get(sessionId)
  if (existing && existing.categories && existing.categories.length > 0) {
    return getContextUsage(sessionId)
  }

  try {
    const { query } = await import('@anthropic-ai/claude-agent-sdk')
    const raw = await readRaw()
    const envOverrides = raw.env && typeof raw.env === 'object' ? raw.env : {}
    const models = await readModels()

    const q = query({
      prompt: oneShot('/context'),
      options: {
        cwd,
        resume: getClaudeSessionId(sessionId) ?? undefined,
        env: { ...process.env, ...envOverrides } as Record<string, string>,
        model: models['ANTHROPIC_MODEL'] || undefined,
        permissionMode: 'plan',
        pathToClaudeCodeExecutable: resolveClaudeBinary(),
        skills: skills.length > 0 ? skills : undefined
      }
    })

    for await (const m of q) {
      if (m.type === 'system' && m.subtype === 'init') {
        const init = m as unknown as { session_id?: string; slash_commands?: string[] }
        if (init.session_id) setClaudeSessionId(sessionId, init.session_id)
        if (init.slash_commands?.length) {
          const live = q as unknown as QueryHandle
          try {
            if (typeof live.supportedCommands === 'function') {
              const list = await live.supportedCommands()
              commandsCache = list.map((c) => ({
                name: c.name,
                description: c.description,
                argumentHint: c.argumentHint,
                builtin: c.builtin === true
              }))
            }
          } catch {
            commandsCache = init.slash_commands.map((name) => ({
              name,
              description: '',
              argumentHint: '',
              builtin: true
            }))
          }
        }
      }
      if (m.type === 'assistant') {
        const withCtx = m as unknown as {
          context_usage?: {
            model?: string
            total_tokens?: number
            raw_max_tokens?: number
            percentage?: number
            categories?: { name?: string; tokens?: number; kind?: string }[]
          }
        }
        if (withCtx.context_usage) {
          const c = withCtx.context_usage
          ctxSnapshots.set(sessionId, {
            model: c.model ?? '',
            total_tokens: c.total_tokens ?? 0,
            raw_max_tokens: c.raw_max_tokens ?? 0,
            percentage: c.percentage ?? 0,
            categories: c.categories ?? []
          })
          setContextUsage(sessionId, c.total_tokens ?? 0, c.raw_max_tokens ?? 0, c.percentage ?? 0)
        }
      }
    }
  } catch (e) {
    console.error('[context] refresh 失败:', e)
  }

  return getContextUsage(sessionId)
}

/**
 * 压缩上下文。
 * SDK 没有独立的 compact 接口，用官方约定的 /compact 命令触发，
 * 由 CLI 自身完成摘要并产生 compact_boundary 消息。
 */
export async function compact(sessionId: string, cwd: string, skills: string[]): Promise<void> {
  if (active) throw new Error('请先等待当前回复结束')

  emit({ type: 'start', sessionId })
  try {
    const { query } = await import('@anthropic-ai/claude-agent-sdk')
    const raw = await readRaw()
    const envOverrides = raw.env && typeof raw.env === 'object' ? raw.env : {}
    const models = await readModels()

    const q = query({
      prompt: oneShot('/compact'),
      options: {
        cwd,
        resume: getClaudeSessionId(sessionId) ?? undefined,
        env: { ...process.env, ...envOverrides } as Record<string, string>,
        model: models['ANTHROPIC_MODEL'] || undefined,
        permissionMode: 'acceptEdits',
        pathToClaudeCodeExecutable: resolveClaudeBinary(),
        skills: skills.length > 0 ? skills : undefined
      }
    })

    // 压缩不写入用户消息，只把摘要结果落库
    let summary = ''
    for await (const m of q) {
      if (m.type === 'assistant') {
        const blocks = (m as unknown as { message?: { content?: Array<Record<string, unknown>> } })
          .message?.content
        if (!Array.isArray(blocks)) continue
        for (const b of blocks) {
          if (b['type'] === 'text' && typeof b['text'] === 'string') summary += b['text']
        }
      }
    }

    if (summary.trim()) {
      appendMessage(sessionId, 'assistant', `（上下文已压缩）

${summary.trim()}`)
    }
  } catch (e) {
    emit({
      type: 'error',
      sessionId,
      message: `压缩失败：${e instanceof Error ? e.message : String(e)}`
    })
    return
  } finally {
    active = null
  }

  emit({ type: 'compacted', sessionId })
}
