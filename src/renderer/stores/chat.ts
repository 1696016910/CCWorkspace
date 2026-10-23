import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { ChatMessage, ToolCall } from '@shared/types'

export interface ChatEvent {
  type: 'start' | 'text' | 'tool' | 'done' | 'error' | 'interrupted'
  sessionId: string
  delta?: string
  tool?: ToolCall
  message?: ChatMessage
  usage?: { inputTokens: number; outputTokens: number; costUsd: number }
}

export const useChatStore = defineStore('chat', () => {
  const streaming = ref(false)
  const streamText = ref('')
  const streamTools = ref<ToolCall[]>([])
  const error = ref<string | null>(null)
  const usage = ref({ inputTokens: 0, outputTokens: 0, costUsd: 0 })
  const activeSessionId = ref<string | null>(null)

  let unsubscribe: (() => void) | null = null

  function listen(onDone: (msg: ChatMessage) => void): void {
    if (unsubscribe) return
    unsubscribe = window.api.chat.onEvent((payload) => {
      const e = payload as ChatEvent
      if (e.sessionId !== activeSessionId.value) return

      switch (e.type) {
        case 'start':
          streaming.value = true
          streamText.value = ''
          streamTools.value = []
          error.value = null
          break
        case 'text':
          streamText.value += e.delta ?? ''
          break
        case 'tool':
          if (e.tool) streamTools.value = [...streamTools.value, e.tool]
          break
        case 'done':
          streaming.value = false
          if (e.usage) usage.value = e.usage
          if (e.message) onDone(e.message)
          streamText.value = ''
          streamTools.value = []
          break
        case 'error':
          streaming.value = false
          error.value = typeof e.message === 'string' ? e.message : '发生未知错误'
          streamText.value = ''
          streamTools.value = []
          break
        case 'interrupted':
          streaming.value = false
          break
      }
    })
  }

  async function send(sessionId: string, text: string): Promise<void> {
    activeSessionId.value = sessionId
    streaming.value = true
    error.value = null
    try {
      await window.api.chat.send(sessionId, text)
    } catch (e) {
      streaming.value = false
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function stop(): Promise<void> {
    await window.api.chat.interrupt()
    streaming.value = false
  }

  return {
    streaming,
    streamText,
    streamTools,
    error,
    usage,
    activeSessionId,
    listen,
    send,
    stop
  }
})
