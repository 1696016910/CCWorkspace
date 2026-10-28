import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as chatApi from '@/api/chat'
import type { ChatEvent, ToolCall } from '@/api/types'

export const useChatStore = defineStore('chat', () => {
  const streaming = ref(false)
  /** 正在生成回复的会话 id；界面据此判断要不要显示流式气泡 */
  const streamingSessionId = ref<string | null>(null)
  const streamText = ref('')
  const streamTools = ref<ToolCall[]>([])
  const stopping = ref(false)
  const error = ref<string | null>(null)

  /**
   * 当前期望的 runId；0 表示"没有在等任何一轮"。
   *
   * 停止后必须保留它：主进程的 done 是异步到达的，
   * 提前清零会让这条 done 被当成过期事件丢掉，
   * 表现为"消息发出去了但界面什么都没显示"。
   */
  const activeRunId = ref(0)
  /**
   * 仍在等待收尾的旧 run。停止后立刻重发时新 run 会占用 activeRunId，
   * 旧 run 的 done 靠这个集合识别，保证它的内容不会被误丢。
   */
  const pendingRuns = new Set<number>()

  /** 事件是否属于当前这轮（含正在收尾的旧轮） */
  function isCurrent(runId: number): boolean {
    return runId === activeRunId.value || pendingRuns.has(runId)
  }

  async function send(sessionId: string, text: string): Promise<void> {
    error.value = null
    streamText.value = ''
    streamTools.value = []
    streaming.value = true
    streamingSessionId.value = sessionId
    stopping.value = false
    try {
      const runId = await chatApi.send(sessionId, text)
      activeRunId.value = runId
    } catch (e) {
      streaming.value = false
      streamingSessionId.value = null
      error.value = e instanceof Error ? e.message : String(e)
      throw e
    }
  }

  async function stop(): Promise<void> {
    if (!streaming.value) return
    stopping.value = true
    // 记进 pendingRuns：这一轮的 done 稍后仍会到达，不能当成过期事件
    if (activeRunId.value) pendingRuns.add(activeRunId.value)
    try {
      await chatApi.interrupt()
    } catch {
      // 中断失败不阻塞 UI，done 事件仍会来
    }
  }

  /** 处理一条后端推送的事件。 */
  function handle(ev: ChatEvent, onDone?: () => void): void {
    if (!isCurrent(ev.runId)) return

    switch (ev.type) {
      case 'text':
        streamText.value += ev.delta
        break
      case 'tool':
        streamTools.value.push(ev.tool)
        break
      case 'done':
        finish(ev.runId)
        onDone?.()
        break
      case 'error':
        error.value = ev.error
        finish(ev.runId)
        onDone?.()
        break
      default:
        break
    }
  }

  function finish(runId: number): void {
    pendingRuns.delete(runId)
    if (runId !== activeRunId.value) return
    streaming.value = false
    streamingSessionId.value = null
    stopping.value = false
    streamText.value = ''
    streamTools.value = []
    activeRunId.value = 0
  }

  const hasStreamText = computed(() => streamText.value.length > 0)

  return {
    streaming,
    streamingSessionId,
    streamText,
    streamTools,
    stopping,
    error,
    hasStreamText,
    send,
    stop,
    handle
  }
})
