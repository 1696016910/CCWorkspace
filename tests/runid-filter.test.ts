import { describe, expect, it } from 'vitest'

/**
 * 与 stores/chat.ts 的事件归属逻辑等价。
 *
 * 关键：停止后立刻重发时，旧 run 的 done 会晚于新 run 的 text 到达。
 * 只按 sessionId 判断归属会让旧 done 把新回复的流式显示清掉，
 * 用户看到的是"我发的消息没有回复"。
 */
interface Ev {
  type: string
  sessionId: string
  runId: number
  delta?: string
}

function makeStore() {
  const s = {
    activeSessionId: '' as string,
    activeRunId: 0,
    streaming: false,
    streamText: '',
    finished: [] as string[],
    errors: [] as string[]
  }

  function handle(e: Ev): void {
    if (e.type === 'start') {
      s.activeSessionId = e.sessionId
      s.activeRunId = e.runId
      s.streaming = true
      s.streamText = ''
      return
    }
    if (e.runId !== s.activeRunId) {
      if (['done', 'error', 'interrupted', 'compacted'].includes(e.type)) {
        s.finished.push(e.sessionId)
      }
      return
    }
    if (e.sessionId !== s.activeSessionId) {
      if (['done', 'error', 'interrupted', 'compacted'].includes(e.type)) {
        s.finished.push(e.sessionId)
      }
      return
    }
    switch (e.type) {
      case 'text':
        s.streamText += e.delta ?? ''
        break
      case 'done':
        s.streaming = false
        s.activeRunId = 0
        s.streamText = ''
        s.finished.push(e.sessionId)
        break
    }
  }
  return { s, handle }
}

describe('runId 事件过滤', () => {
  it('旧 run 的迟到 done 不会清掉新 run 的流式内容', () => {
    const { s, handle } = makeStore()
    // run 1 启动后用户点了停止
    handle({ type: 'start', sessionId: 'A', runId: 1 })
    // 立刻重发，run 2 启动
    handle({ type: 'start', sessionId: 'A', runId: 2 })
    handle({ type: 'text', sessionId: 'A', runId: 2, delta: '新回复' })
    // run 1 的 done 迟到
    handle({ type: 'done', sessionId: 'A', runId: 1 })

    expect(s.streamText).toBe('新回复')
    expect(s.streaming).toBe(true)
  })

  it('当前 run 的 done 正常收尾', () => {
    const { s, handle } = makeStore()
    handle({ type: 'start', sessionId: 'A', runId: 5 })
    handle({ type: 'text', sessionId: 'A', runId: 5, delta: 'x' })
    handle({ type: 'done', sessionId: 'A', runId: 5 })

    expect(s.streaming).toBe(false)
    expect(s.streamText).toBe('')
    expect(s.finished).toContain('A')
  })

  it('旧 run 的结束信号仍会让该会话的绿点复位', () => {
    const { s, handle } = makeStore()
    handle({ type: 'start', sessionId: 'A', runId: 2 })
    handle({ type: 'done', sessionId: 'A', runId: 1 })
    expect(s.finished).toContain('A')
  })

  it('切走的会话的增量不会串进当前会话', () => {
    const { s, handle } = makeStore()
    handle({ type: 'start', sessionId: 'B', runId: 3 })
    handle({ type: 'text', sessionId: 'A', runId: 3, delta: '不该出现' })
    expect(s.streamText).toBe('')
  })
})
