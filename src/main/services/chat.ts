import { BrowserWindow } from 'electron'
import type { ChatMessage, ToolCall } from '../../shared/types'
import { appendMessage } from './session'
import { readModels, readPermissions, readRaw } from './claudeSettings'
import { resolveClaudeBinary } from './binary'

export type ChatEvent =
  | { type: 'start'; sessionId: string }
  | { type: 'text'; sessionId: string; delta: string }
  | { type: 'tool'; sessionId: string; tool: ToolCall }
  | { type: 'done'; sessionId: string; message: ChatMessage; usage: Usage }
  | { type: 'error'; sessionId: string; message: string }
  | { type: 'interrupted'; sessionId: string }

export interface Usage {
  inputTokens: number
  outputTokens: number
  costUsd: number
}

interface ActiveRun {
  abort: () => void
}

let active: ActiveRun | null = null

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

/**
 * 把一条用户消息包成 SDK 的流式输入项。
 * SDK 的 interrupt() 与 setModel() 只在流式输入模式下可用，
 * 因此即便一次只发一条消息，也走 AsyncIterable 而非字符串 prompt。
 */
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

/**
 * 发送一条消息并流式回传。
 *
 * 用 Agent SDK 而非手写 stdio 协议——协议里有未文档化的
 * control_request / mcp_response 包装层，自己维护不划算。
 */
export async function send(
  sessionId: string,
  text: string,
  cwd: string,
  skills: string[]
): Promise<void> {
  if (active) throw new Error('已有回复正在生成，请先停止')

  appendMessage(sessionId, 'user', text)
  emit({ type: 'start', sessionId })

  let acc = ''
  const tools: ToolCall[] = []
  let aborted = false
  let usage: Usage = { inputTokens: 0, outputTokens: 0, costUsd: 0 }

  try {
    const { query } = await import('@anthropic-ai/claude-agent-sdk')
    const raw = await readRaw()
    const envOverrides = raw.env && typeof raw.env === 'object' ? raw.env : {}
    const models = await readModels()
    const model = models['ANTHROPIC_MODEL'] || undefined
    // 权限模式来自设置页；无法识别的值回落到 acceptEdits
    const perms = await readPermissions()
    const mode = (
      ['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions'] as const
    ).includes(perms.defaultMode as never)
      ? (perms.defaultMode as 'default' | 'acceptEdits' | 'plan' | 'dontAsk' | 'bypassPermissions')
      : 'acceptEdits'

    const q = query({
      prompt: oneShot(text),
      options: {
        cwd,
        env: { ...process.env, ...envOverrides } as Record<string, string>,
        model,
        permissionMode: mode,
        // 打包后 SDK 默认会从 app.asar 里找二进制，需显式指向解包后的路径
        pathToClaudeCodeExecutable: resolveClaudeBinary(),
        // 'all' 表示全部 Skill；禁用列表通过显式白名单表达
        skills: skills.length > 0 ? skills : undefined
      }
    })

    active = {
      abort: () => {
        aborted = true
        void q.interrupt()
      }
    }

    for await (const message of q) {
      if (aborted) break

      if (message.type === 'assistant') {
        const blocks = (
          message as unknown as {
            message?: { content?: Array<Record<string, unknown>> }
          }
        ).message?.content
        if (!Array.isArray(blocks)) continue

        for (const block of blocks) {
          if (block['type'] === 'text' && typeof block['text'] === 'string') {
            acc += block['text']
            emit({ type: 'text', sessionId, delta: block['text'] })
          } else if (block['type'] === 'tool_use') {
            const input = (block['input'] ?? {}) as Record<string, unknown>
            const target =
              (typeof input['file_path'] === 'string' && input['file_path']) ||
              (typeof input['command'] === 'string' && input['command']) ||
              (typeof input['pattern'] === 'string' && input['pattern']) ||
              ''
            const tool: ToolCall = {
              name: String(block['name'] ?? ''),
              target: String(target)
            }
            tools.push(tool)
            emit({ type: 'tool', sessionId, tool })
          }
        }
      } else if (message.type === 'result') {
        const r = message as unknown as {
          usage?: { input_tokens?: number; output_tokens?: number }
          total_cost_usd?: number
        }
        usage = {
          inputTokens: r.usage?.input_tokens ?? 0,
          outputTokens: r.usage?.output_tokens ?? 0,
          costUsd: r.total_cost_usd ?? 0
        }
      }
    }
  } catch (e) {
    active = null
    emit({
      type: 'error',
      sessionId,
      message: e instanceof Error ? e.message : String(e)
    })
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
