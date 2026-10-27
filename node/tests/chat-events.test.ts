import { describe, expect, it } from 'vitest'

/**
 * 复刻 chat store 的事件归属逻辑。
 *
 * 关键约束：事件必须按自带的 sessionId 判定归属，不能用"当前看着哪个会话"过滤。
 * 用后者会让切走的会话的 done 被整体丢弃 —— 消息不进界面、绿点不复位。
 */
interface Ev {
  type: string
  sessionId: string
  delta?: string
  message?: unknown
}

function makeStore(activeSession: string) {
  const state = {
    streaming: false,
    streamText: '',
    finished: [] as string[]
  }
  return {
    state,
    handle(e: Ev): void {
      if (e.sessionId !== activeSession) {
        if (e.type === 'done' || e.type === 'error' || e.type === 'interrupted') {
          state.finished.push(e.sessionId)
        }
        return
      }
      switch (e.type) {
        case 'start':
          state.streaming = true
          state.streamText = ''
          break
        case 'text':
          state.streamText += e.delta ?? ''
          break
        case 'done':
          state.streaming = false
          state.streamText = ''
          state.finished.push(e.sessionId)
          break
        case 'interrupted':
        case 'compacted':
          state.streaming = false
          state.streamText = ''
          state.finished.push(e.sessionId)
          break
      }
    }
  }
}

describe('chat 事件归属', () => {
  it('切走的会话结束时仍会触发收尾（绿点复位）', () => {
    const s = makeStore('B')
    s.handle({ type: 'done', sessionId: 'A' })
    expect(s.state.finished).toContain('A')
  })

  it('切走的会话的增量文本不会写进当前会话', () => {
    const s = makeStore('B')
    s.handle({ type: 'text', sessionId: 'A', delta: '不该出现' })
    expect(s.state.streamText).toBe('')
  })

  it('compacted 事件会复位 streaming（否则输入框永久禁用）', () => {
    const s = makeStore('A')
    s.handle({ type: 'start', sessionId: 'A' })
    expect(s.state.streaming).toBe(true)
    s.handle({ type: 'compacted', sessionId: 'A' })
    expect(s.state.streaming).toBe(false)
    expect(s.state.finished).toContain('A')
  })

  it('interrupted 会清掉残留的流式文本', () => {
    const s = makeStore('A')
    s.handle({ type: 'start', sessionId: 'A' })
    s.handle({ type: 'text', sessionId: 'A', delta: '半截话' })
    s.handle({ type: 'interrupted', sessionId: 'A' })
    expect(s.state.streamText).toBe('')
    expect(s.state.streaming).toBe(false)
  })

  it('同一会话的完整流程', () => {
    const s = makeStore('A')
    s.handle({ type: 'start', sessionId: 'A' })
    s.handle({ type: 'text', sessionId: 'A', delta: '你' })
    s.handle({ type: 'text', sessionId: 'A', delta: '好' })
    expect(s.state.streamText).toBe('你好')
    s.handle({ type: 'done', sessionId: 'A' })
    expect(s.state.streaming).toBe(false)
    expect(s.state.finished).toEqual(['A'])
  })
})
