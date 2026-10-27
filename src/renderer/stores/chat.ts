import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ChatMessage, ToolCall } from '@shared/types'

export interface ChatEvent {
  type: 'start' | 'text' | 'tool' | 'done' | 'error' | 'interrupted' | 'compacted' | 'commands'
  sessionId: string
  /** 哪一轮；停止后立刻重发时，用来丢弃旧 run 的迟到事件 */
  runId: number
  delta?: string
  tool?: ToolCall
  message?: ChatMessage
  usage?: { inputTokens: number; outputTokens: number; costUsd: number }
  interrupted?: boolean
}

export const useChatStore = defineStore('chat', () => {
  const streaming = ref(false)
  /** 正在生成的会话 id；界面据此判断是否显示流式内容 */
  const streamingSessionId = ref<string | null>(null)
  const streamText = ref('')
  const streamTools = ref<ToolCall[]>([])
  const error = ref<string | null>(null)
  const usage = ref({ inputTokens: 0, outputTokens: 0, costUsd: 0 })
  const activeSessionId = ref<string | null>(null)
  /** 当前这一轮的 runId；事件带的 runId 与之不符即为过期事件 */
  /**
   * 当前期望的 runId；0 表示"没有在等任何一轮"。
   *
   * 停止后要保留它：主进程的 done 是异步到达的，
   * 提前清零会让这条 done 被当成过期事件丢弃，
   * 表现为"消息发出去了但界面什么都没显示"。
   */
  const activeRunId = ref(0)
  /**
   * 仍在等待收尾的旧 run。停止后立刻重发时，
   * 新 run 会占用 activeRunId，旧 run 的 done 靠这个集合识别，
   * 保证它的内容不会被误丢。
   */
  const pendingRuns = new Set<number>()

  let unsubscribe: (() => void) | null = null

  interface Listeners {
    /** 本轮结束，追加助手消息（带 sessionId，由调用方判断是否属于当前会话） */
    onDone: (msg: ChatMessage, sessionId: string) => void
    /** 任意结束（完成 / 出错 / 中断），用于清理运行态 */
    onFinish: (sessionId: string) => void
    /** 命令清单就绪 */
    onCommands?: (sessionId: string) => void
  }

  function listen(listeners: Listeners): void {
    if (unsubscribe) return
    unsubscribe = window.api.chat.onEvent((payload) => {
      const e = payload as ChatEvent

      // start 是"认领新一轮"，必须在过滤之前处理，
      // 否则第一次 start 会因为 activeRunId 还是 0 而被自己挡掉
      if (e.type === 'start') {
        // 上一个 run 若还在收尾，记下来，避免它的 done 被当成过期
        if (activeRunId.value !== 0) pendingRuns.add(activeRunId.value)
        if (e.sessionId !== activeSessionId.value) {
          // 不是我在等的那个会话，仍要记下它，避免它的收尾被后续过滤误吞
          activeSessionId.value = e.sessionId
        }
        activeRunId.value = e.runId
        streaming.value = true
        streamingSessionId.value = e.sessionId
        streamText.value = ''
        streamTools.value = []
        error.value = null
        return
      }

      // 过期 run 的事件一律丢弃。
      // 停止后立刻重发时，旧 run 的 done 会比新 run 的 text 更晚到达，
      // 不拦的话它会把新回复的流式显示清掉，界面看着像"没回复"。
      const isCurrent = e.runId === activeRunId.value
      const isPending = pendingRuns.has(e.runId)
      if (!isCurrent && !isPending) {
        if (e.type === 'done' || e.type === 'error' || e.type === 'interrupted' || e.type === 'compacted') {
          listeners.onFinish(e.sessionId)
        }
        return
      }

      // 2) 不是当前活跃会话：增量无从展示，但收尾信号必须处理，
      //    否则该会话的绿点永远不复位
      if (e.sessionId !== activeSessionId.value) {
        if (e.type === 'done' || e.type === 'error' || e.type === 'interrupted' || e.type === 'compacted') {
          listeners.onFinish(e.sessionId)
        }
        return
      }

      switch (e.type) {
        case 'text':
          streamText.value += e.delta ?? ''
          break
        case 'tool':
          if (e.tool) streamTools.value = [...streamTools.value, e.tool]
          break
        case 'done':
          pendingRuns.delete(e.runId)
          // 只有当前 run 才复位这些界面状态；旧 run 的收尾不打断新一轮
          if (isCurrent) {
            streaming.value = false
            streamingSessionId.value = null
            activeRunId.value = 0
          }
          if (isCurrent && !e.message) {
            streamText.value = ''
            streamTools.value = []
          }
          if (e.usage && isCurrent) usage.value = e.usage
          // 消息一律追加，由界面按 sessionId 判断归属
          if (e.message) {
            listeners.onDone({ ...e.message, stopped: e.interrupted === true }, e.sessionId)
          }
          if (isCurrent) {
            streamText.value = ''
            streamTools.value = []
          }
          listeners.onFinish(e.sessionId)
          break
        case 'error':
          streaming.value = false
          streamingSessionId.value = null
          activeRunId.value = 0
          error.value = typeof e.message === 'string' ? e.message : '发生未知错误'
          streamText.value = ''
          streamTools.value = []
          listeners.onFinish(e.sessionId)
          break
        case 'interrupted':
        case 'compacted':
          // 压缩结束时主进程只发 compacted 不发 done，
          // 不在这里复位就会一直显示"生成中"，输入框永久禁用
          streaming.value = false
          streamingSessionId.value = null
          activeRunId.value = 0
          streamText.value = ''
          streamTools.value = []
          listeners.onFinish(e.sessionId)
          break
        case 'commands':
          listeners.onCommands?.(e.sessionId)
          break
      }
    })
  }

  /** 发送一条消息；返回是否成功发起（生成中的失败通过事件回传） */
  async function send(sessionId: string, text: string): Promise<boolean> {
    // 记下发送前的状态：失败时必须恢复成"原样"，
    // 而不是无脑清空——否则在 B 里发送失败会顺手把 A 正在跑的运行态抹掉，
    // A 后续的流式内容和最终回复就全被当成过期事件丢弃了。
    const prevStreaming = streaming.value
    const prevSession = streamingSessionId.value
    const prevRunId = activeRunId.value

    streaming.value = true
    error.value = null
    try {
      await window.api.chat.send(sessionId, text)
      // 只有真正发出去之后才认领这个会话。
      // 提前认领会把失败的那次也算成"活跃会话"，从而丢掉上一个会话的收尾事件。
      activeSessionId.value = sessionId
      return true
    } catch (e) {
      streaming.value = prevStreaming
      streamingSessionId.value = prevSession
      activeRunId.value = prevRunId
      error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  /** 释放事件订阅；组件卸载时调用，否则重建后会闭包着旧实例 */
  function unlisten(): void {
    unsubscribe?.()
    unsubscribe = null
  }

  async function stop(): Promise<void> {
    await window.api.chat.interrupt()
    // 只复位"正在生成"的界面状态，但保留 activeRunId。
    // 清掉它会让随后到达的 done 被当成过期事件丢弃，
    // 结果是消息写进了库、界面却什么都不显示——用户看到的就是
    // "发了消息没有任何回应，也没有已停止提示"。
    streaming.value = false
    streamingSessionId.value = null
  }

  /** 压缩当前会话的上下文 */
  async function compact(sessionId: string): Promise<void> {
    await window.api.context.compact(sessionId)
  }

  return {
    streaming,
    streamingSessionId,
    streamText,
    streamTools,
    error,
    usage,
    activeSessionId,
    listen,
    unlisten,
    send,
    stop,
    compact
  }
})
