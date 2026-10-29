import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as chatApi from '@/api/chat'
import type { Attachment, ChatEvent, ToolCall } from '@/api/types'

/** 一个会话正在进行中的一轮 */
interface RunState {
  runId: number
  text: string
  tools: ToolCall[]
  stopping: boolean
  /** 点击停止的时刻，用于前端兜底复位 */
  stoppedAt: number
}

/**
 * 停止后等这么久还没收到 done，就本地复位，保证用户总有出路。
 *
 * 必须大于后端杀进程树的实际耗时，否则会在后端还在收尾时就先报"停止超时"，
 * 用户看到一条假错误。
 */
const STOP_TIMEOUT_MS = 30_000

export const useChatStore = defineStore('chat', () => {
  /**
   * 按会话分桶。
   *
   * 原来是单份全局状态，导致一个会话在跑时所有会话的按钮都变成停止、
   * 所有会话都发不出消息。多会话并行必须按会话隔离。
   */
  const runs = ref<Map<string, RunState>>(new Map())

  /** 生成完成但用户还没切过去看的会话 */
  const unseen = ref<Set<string>>(new Set())

  /**
   * 消息 id → 本轮耗时（毫秒）。
   *
   * 只在本次运行内有效：重开应用后历史消息不显示耗时，
   * 因为那是上一次运行的事，对当下没有参考价值。
   */
  const lastDuration = ref<Map<string, number>>(new Map())

  const error = ref<string | null>(null)

  /** 该会话是否正在生成 */
  function isStreaming(sessionId: string | null): boolean {
    return !!sessionId && runs.value.has(sessionId)
  }

  function streamText(sessionId: string | null): string {
    return (sessionId && runs.value.get(sessionId)?.text) || ''
  }

  function streamTools(sessionId: string | null): ToolCall[] {
    return (sessionId && runs.value.get(sessionId)?.tools) || []
  }

  function isStopping(sessionId: string | null): boolean {
    return !!sessionId && !!runs.value.get(sessionId)?.stopping
  }

  /** 正在生成的会话数，用于并发上限提示 */
  const activeCount = computed(() => runs.value.size)

  function ensure(sessionId: string, runId: number): RunState {
    let r = runs.value.get(sessionId)
    if (!r) {
      r = { runId, text: '', tools: [], stopping: false, stoppedAt: 0 }
      runs.value.set(sessionId, r)
    }
    return r
  }

  async function send(sessionId: string, text: string, attachments: Attachment[] = []): Promise<void> {
    error.value = null
    // 先占位：否则 runId 回来之前界面看不到"正在思考"
    runs.value.set(sessionId, {
      runId: 0,
      text: '',
      tools: [],
      stopping: false,
      stoppedAt: 0
    })
    unseen.value.delete(sessionId)
    try {
      const runId = await chatApi.send(sessionId, text, attachments)
      const r = runs.value.get(sessionId)
      if (r) r.runId = runId
    } catch (e) {
      runs.value.delete(sessionId)
      error.value = e instanceof Error ? e.message : String(e)
      throw e
    }
  }

  async function stop(sessionId: string): Promise<void> {
    const r = runs.value.get(sessionId)
    if (!r || r.stopping) return
    r.stopping = true
    r.stoppedAt = Date.now()
    try {
      await chatApi.interrupt(sessionId)
    } catch {
      // 中断失败不阻塞 UI：done 仍会到达，兜底计时器也会处理
    }
  }

  /** 停止超时的本地复位。由组件定时调用。 */
  function checkStopTimeout(sessionId: string): boolean {
    const r = runs.value.get(sessionId)
    if (!r || !r.stopping || !r.stoppedAt) return false
    if (Date.now() - r.stoppedAt < STOP_TIMEOUT_MS) return false
    runs.value.delete(sessionId)
    error.value = '停止超时，已断开显示；后台进程可能仍在运行'
    return true
  }

  /** 处理一条后端推送的事件。 */
  function handle(
    ev: ChatEvent,
    onDone?: (sessionId: string) => void,
    onContext?: (sessionId: string) => void
  ): void {
    const sid = ev.sessionId
    switch (ev.type) {
      case 'start': {
        ensure(sid, ev.runId)
        break
      }
      case 'text': {
        const r = ensure(sid, ev.runId)
        r.text += ev.delta
        break
      }
      case 'tool': {
        const r = ensure(sid, ev.runId)
        r.tools.push(ev.tool)
        break
      }
      case 'meta': {
        // 能力清单由 chatApi.meta() 拉，这里只是通知
        break
      }
      case 'context': {
        // 后端用 /context 校准过用量，库里已更新。只需重拉会话列表，
        // 不要复用 done 的回调——那里会重拉消息、抢焦点，对这个事件是多余的。
        onContext?.(sid)
        break
      }
      case 'done':
      case 'error': {
        if (ev.type === 'error') error.value = ev.error
        // 本轮耗时随 done 一起带回，挂到消息上供界面显示
        if (ev.type === 'done' && ev.usage?.durationMs && ev.message) {
          lastDuration.value = new Map(lastDuration.value).set(
            ev.message.id,
            ev.usage.durationMs as number
          )
        }
        runs.value.delete(sid)
        // 用户不在这个会话上，标成待查看
        unseen.value = new Set([...unseen.value, sid])
        onDone?.(sid)
        break
      }
      default:
        break
    }
  }

  /** 用户切到该会话：清掉待查看标记 */
  function markSeen(sessionId: string | null): void {
    if (!sessionId || !unseen.value.has(sessionId)) return
    const next = new Set(unseen.value)
    next.delete(sessionId)
    unseen.value = next
  }

  /** 会话被删除时清掉残留，否则 Map 与 Set 只涨不消 */
  function forget(sessionId: string): void {
    runs.value.delete(sessionId)
    if (unseen.value.has(sessionId)) {
      const next = new Set(unseen.value)
      next.delete(sessionId)
      unseen.value = next
    }
  }

  return {
    runs,
    unseen,
    lastDuration,
    error,
    activeCount,
    isStreaming,
    streamText,
    streamTools,
    isStopping,
    send,
    stop,
    checkStopTimeout,
    handle,
    markSeen,
    forget
  }
})
