import { BrowserWindow } from 'electron'
import type { ChatMessage, ContextUsageDetail, SlashCommandInfo, ToolCall } from '../../shared/types'
import { appendMessage, getClaudeSessionId, setClaudeSessionId, setContextUsage } from './session'
import { readModels, readPermissions, readRaw } from './claudeSettings'
import { resolveClaudeBinary } from './binary'

/**
 * runId 标识"哪一轮"。
 *
 * 停止后立刻重发时，旧 run 的 done 会晚于新 run 的 start 到达；
 * 渲染层若只按 sessionId 判断归属，旧 done 会把新回复的流式显示清掉。
 */
export type ChatEvent =
  | { type: 'start'; sessionId: string; runId: number }
  | { type: 'text'; sessionId: string; runId: number; delta: string }
  | { type: 'tool'; sessionId: string; runId: number; tool: ToolCall }
  | { type: 'done'; sessionId: string; runId: number; message: ChatMessage; usage: Usage; interrupted?: boolean }
  | { type: 'error'; sessionId: string; runId: number; message: string }
  | { type: 'interrupted'; sessionId: string; runId: number }
  | { type: 'compacted'; sessionId: string; runId: number }
  | { type: 'commands'; sessionId: string; runId: number }

export interface Usage {
  inputTokens: number
  outputTokens: number
  costUsd: number
  /** 上下文窗口占用百分比 */
  percentage: number
}

/**
 * SDK 的 skills 字段语义是"白名单"，省略表示沿用 CLI 默认（即全部可用）。
 * 所以用户把 Skill 全关掉时必须显式传空数组，传 undefined 反而等于全开。
 */
function skillsOption(skills: string[]): string[] {
  return skills
}

/** 单轮输出的安全上限，防止异常模型行为撑爆内存与数据库 */
const MAX_OUTPUT_CHARS = 400_000
/** 单轮工具调用记录上限 */
const MAX_TOOLS = 500

interface ActiveRun {
  /** 每次运行唯一，收尾时只有 id 对得上才允许清空 active */
  id: number
  abort: () => void
  /** 立刻释放占位（用于 interrupt），后续收尾不会再误清别人的 active */
  release: () => void
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
/** 运行序号，用来识别"这一轮是否仍然是当前占用者" */
let runSeq = 0
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

/** 会话被彻底删除时释放它的上下文快照，否则这个 Map 只增不减 */
export function forgetContext(sessionId: string): void {
  ctxSnapshots.delete(sessionId)
}

export function interrupt(): void {
  const run = active
  if (!run) return
  run.abort()
  // 立刻让位，用户可以马上发下一条；
  // 旧 run 收尾时 release() 会发现 active 已不是自己，不会误清新的占位
  run.release()
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

  // 必须在这里同步占位：后面有 import/读配置等多个 await，
  // 中间空档里第二次 chat:send 会再次通过校验，起两个 run。
  const runId = ++runSeq
  /** 占位阶段就被用户点了停止（此时还没有真实 query 可中断） */
  let abortRequested = false
  const placeholder: ActiveRun = {
    id: runId,
    abort: () => {
      abortRequested = true
    },
    release: () => {
      if (active?.id === runId) active = null
    }
  }
  active = placeholder

  const resumeId = getClaudeSessionId(sessionId) ?? undefined

  // 会话可能在发送瞬间已被删除，这里失败要立刻告诉用户，而不是照常跑一轮
  try {
    appendMessage(sessionId, 'user', text)
  } catch (e) {
    placeholder.release()
    emit({ type: 'error', sessionId, runId, message: e instanceof Error ? e.message : String(e) })
    return
  }
  emit({ type: 'start', sessionId, runId })

  let acc = ''
  const tools: ToolCall[] = []
  let aborted = false
  let sawAnyEvent = false
  let timedOut = false
  let usage: Usage = { inputTokens: 0, outputTokens: 0, costUsd: 0, percentage: 0 }
  /** 本轮占位；query 创建后才赋值，收尾时统一 release */
  let run: ActiveRun | null = null
  /** result 事件里带回的错误，非空表示这一轮模型侧失败 */
  let runError: string | null = null

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
        skills: skillsOption(skills)
      }
    })

    // 复用外层已分配的 runId：在这里再 ++ 一次会让 start/done 用一个 id、
    // text/tool 用另一个 id，渲染层会把所有增量当成过期事件丢弃，
    // 表现为整轮只显示"正在思考"，结束后才整段冒出内容。
    const thisRun: ActiveRun = {
      id: runId,
      abort: () => {
        aborted = true
        void q.interrupt().catch(() => {})
      },
      release: () => {
        // 只有当前占用者才能释放，避免旧 run 的收尾把新 run 的占位抹掉
        if (active?.id === runId) active = null
      }
    }
    // 占位换成带真实 abort 的实现，但 id 保持不变
    run = thisRun
    if (abortRequested || active?.id !== runId) {
      // 建 query 期间用户已经点了停止：立刻中止，别让它白跑白计费
      aborted = true
      void q.interrupt().catch(() => {})
    } else {
      active = thisRun
    }

    const iterator = q[Symbol.asyncIterator]()

    while (!aborted) {
      let timer: NodeJS.Timeout | undefined
      const next = await Promise.race([
        iterator.next(),
        new Promise<{ done: true; value: undefined; timedOut: true }>((resolve) => {
          timer = setTimeout(
            () => resolve({ done: true, value: undefined, timedOut: true }),
            30_000
          )
        })
      ])
      // 不清理的话，一轮几百个事件会留下几百个悬挂定时器，
      // 结束后还要拖住事件循环最多 30 秒
      if (timer) clearTimeout(timer)
      if (next.done) {
        timedOut = 'timedOut' in next
        // break 不会触发 async generator 的 finally，
        // 不主动 return 的话 CLI 子进程会一直活着继续计费
        await iterator.return?.()
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
        // parent_tool_use_id 非空说明这段来自 Task 子代理。
        // 不区分的话，子代理的中间输出会拼进主回复、还实时显示给用户，
        // 保存下来的助手消息也会变成"子代理文本 + 主模型回答"的混合物。
        const fromSubagent =
          (message as unknown as { parent_tool_use_id?: string | null }).parent_tool_use_id != null
        if (fromSubagent) continue

        const delta = deltaText((message as unknown as { event: unknown }).event)
        if (delta && acc.length < MAX_OUTPUT_CHARS) {
          acc += delta
          emit({ type: 'text', sessionId, runId, delta })
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

        // 子代理的完整消息同样不能算进主回复
        if ((message as unknown as { parent_tool_use_id?: string | null }).parent_tool_use_id != null) {
          continue
        }

        const blocks = (
          message as unknown as { message?: { content?: Array<Record<string, unknown>> } }
        ).message?.content
        if (!Array.isArray(blocks)) continue

        for (const block of blocks) {
          if (block['type'] === 'text' && typeof block['text'] === 'string') {
            if (!acc) {
              acc = block['text']
              emit({ type: 'text', sessionId, runId, delta: acc })
            }
          } else if (block['type'] === 'tool_use') {
            const input = (block['input'] ?? {}) as Record<string, unknown>
            const target =
              (typeof input['file_path'] === 'string' && input['file_path']) ||
              (typeof input['command'] === 'string' && input['command']) ||
              (typeof input['pattern'] === 'string' && input['pattern']) ||
              ''
            const safeTarget = String(target).slice(0, 500)
            if (tools.length < MAX_TOOLS) {
              const tool: ToolCall = { name: String(block['name'] ?? ''), target: safeTarget }
              tools.push(tool)
              emit({ type: 'tool', sessionId, runId, tool })
            }
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
          emit({ type: 'commands', sessionId, runId })
        }
        continue
      }

      if (message.type === 'result') {
        const r = message as unknown as {
          session_id?: string
          usage?: {
            input_tokens?: number
            output_tokens?: number
            cache_read_input_tokens?: number
            cache_creation_input_tokens?: number
          }
          total_cost_usd?: number
          is_error?: boolean
          subtype?: string
          errors?: string[]
          modelUsage?: Record<string, { contextWindow?: number; inputTokens?: number }>
        }
        if (r.session_id) setClaudeSessionId(sessionId, r.session_id)

        // result 也可能是失败（密钥错误、额度用尽、超过轮次上限）。
        // 不识别的话会走成功分支，写入一条空的助手消息，用户只看到空气泡。
        if (r.is_error) {
          runError =
            r.errors?.filter(Boolean).join('；') ||
            (r.subtype === 'error_max_turns'
              ? '已达到本轮最大轮次限制'
              : '模型返回了错误结果')
        }

        let maxTokens = 0
        let windowModel = ''
        for (const [name, info] of Object.entries(r.modelUsage ?? {})) {
          if (info.contextWindow && info.contextWindow > maxTokens) {
            maxTokens = info.contextWindow
            windowModel = name
          }
        }
        // resume 场景下命中 prompt cache 的 token 记在 cache_read 里，
        // input_tokens 只剩未命中的增量。只算 input+output 会让
        // 15 万 token 的上下文显示成 0.5%，用户永远看不到该压缩了。
        const cached =
          (r.usage?.cache_read_input_tokens ?? 0) + (r.usage?.cache_creation_input_tokens ?? 0)
        const inputOnly = r.usage?.input_tokens ?? 0
        const outputOnly = r.usage?.output_tokens ?? 0
        const used = inputOnly + cached + outputOnly
        const pct = maxTokens > 0 ? Math.min(999, Math.round((used / maxTokens) * 1000) / 10) : 0

        usage = {
          inputTokens: inputOnly,
          outputTokens: outputOnly,
          costUsd: r.total_cost_usd ?? 0,
          percentage: pct
        }
        // 保留已有的分类明细：清空会让刷新判定（要求 categories 非空）
        // 永远失败，于是每轮对话后第一次打开面板都会重跑一次 /context，
        // 白烧 token。token 数本身每轮都更新，分类明细沿用即可。
        ctxSnapshots.set(sessionId, {
          model: windowModel || model || '',
          total_tokens: used,
          raw_max_tokens: maxTokens,
          percentage: pct,
          categories: ctxSnapshots.get(sessionId)?.categories ?? []
        })
        setContextUsage(sessionId, used, maxTokens, pct)
      }
    }
  } catch (e) {
    run?.release()
    emit({ type: 'error', sessionId, runId, message: e instanceof Error ? e.message : String(e) })
    return
  }

  run?.release()

  // 超时或从未收到有效响应：多半是服务不可达
  if ((timedOut || !sawAnyEvent) && !aborted) {
    emit({
      type: 'error',
      sessionId,
      runId,
      message: '连接超时，请检查网络与 ANTHROPIC_BASE_URL 配置'
    })
    return
  }

  // 模型侧返回错误：明确告诉用户，并且不写入空消息
  if (runError) {
    emit({ type: 'error', sessionId, runId, message: runError })
    return
  }

  // 没有任何文本且不是主动停止：说明这一轮什么也没产出，
  // 写一条空消息只会让对话流里多出一个空气泡
  if (!acc && !aborted) {
    emit({
      type: 'error',
      sessionId,
      runId,
      message: '模型没有返回任何内容，请重试'
    })
    return
  }

  let saved: ChatMessage
  try {
    saved = appendMessage(
      sessionId,
      'assistant',
      acc || '（已停止）',
      tools.length ? tools : undefined
    )
  } catch (e) {
    // 写到一半会话被删了：内容已在界面显示完，告诉用户没能保存
    emit({
      type: 'error',
      sessionId,
      runId,
      message: e instanceof Error ? e.message : String(e)
    })
    return
  }

  // 中断时不要再单独发 interrupted：渲染层收到它就会把 runId 清零，
  // 紧跟其后的 done 反而被当成过期事件丢弃，结果是"库里有消息、界面没有"。
  // 一个 done 带上 interrupted 标记就够了。
  emit({ type: 'done', sessionId, runId, message: saved, usage, interrupted: aborted })
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
/** 正在跑 /context 的会话，避免轮询把同一个会话的 CLI 反复拉起 */
const refreshing = new Set<string>()

export async function refreshContext(
  sessionId: string,
  cwd: string,
  skills: string[]
): Promise<ContextUsageDetail | null> {
  // 有回复在跑时不打扰：另起一个 CLI resume 同一会话会互相污染 transcript
  if (active) return getContextUsage(sessionId)
  // 已经在刷新了就直接返回现有数据，让轮询自然停下
  if (refreshing.has(sessionId)) return getContextUsage(sessionId)

  const existing = ctxSnapshots.get(sessionId)
  if (existing && existing.categories && existing.categories.length > 0) {
    return getContextUsage(sessionId)
  }

  refreshing.add(sessionId)
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
        skills: skillsOption(skills)
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
  } finally {
    refreshing.delete(sessionId)
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

  // 压缩同样要占位：不占的话压缩期间用户能再发消息，
  // 两个 CLI 会同时 resume 同一个会话；而且 finally 里的清理
  // 会把那次 send 的占位一起抹掉，导致中断失效。
  const runId = ++runSeq
  const placeholder: ActiveRun = {
    id: runId,
    abort: () => {},
    release: () => {
      if (active?.id === runId) active = null
    }
  }
  active = placeholder

  emit({ type: 'start', sessionId, runId })
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
        skills: skillsOption(skills)
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
      runId,
      message: `压缩失败：${e instanceof Error ? e.message : String(e)}`
    })
    return
  } finally {
    placeholder.release()
  }

  // 压缩后旧快照必然失效：不清掉的话面板还显示压缩前的百分比和分类，
  // 用户会以为没生效并反复点压缩
  ctxSnapshots.delete(sessionId)
  setContextUsage(sessionId, 0, 0, 0)

  emit({ type: 'compacted', sessionId, runId })
}
