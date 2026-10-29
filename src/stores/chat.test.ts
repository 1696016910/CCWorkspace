import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

// 用假的后端替换 IPC，专门测状态机本身
vi.mock('@/api/chat', () => ({
  send: vi.fn(async (_sid: string, _text: string) => 1),
  interrupt: vi.fn(async () => {}),
  onEvent: vi.fn(async () => () => {}),
  onCompact: vi.fn(async () => () => {}),
  compact: vi.fn(async () => {}),
  meta: vi.fn(async () => null)
}))

import { useChatStore } from '@/stores/chat'

describe('chat store 按会话分桶', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('两个会话同时生成时互不覆盖', async () => {
    const chat = useChatStore()
    await chat.send('a', '你好')
    await chat.send('b', '世界')

    chat.handle({ type: 'text', sessionId: 'a', runId: 1, delta: 'AAA' })
    chat.handle({ type: 'text', sessionId: 'b', runId: 1, delta: 'BBB' })

    expect(chat.streamText('a')).toBe('AAA')
    expect(chat.streamText('b')).toBe('BBB')
    expect(chat.isStreaming('a')).toBe(true)
    expect(chat.isStreaming('b')).toBe(true)
    expect(chat.activeCount).toBe(2)
  })

  it('停一个会话不影响另一个', async () => {
    const chat = useChatStore()
    await chat.send('a', 'x')
    await chat.send('b', 'y')

    await chat.stop('a')

    expect(chat.isStopping('a')).toBe(true)
    expect(chat.isStopping('b')).toBe(false)
    expect(chat.isStreaming('b')).toBe(true)
  })

  it('done 只清对应会话', async () => {
    const chat = useChatStore()
    await chat.send('a', 'x')
    await chat.send('b', 'y')

    chat.handle({
      type: 'done',
      sessionId: 'a',
      runId: 1,
      message: { id: 'm1', role: 'assistant', text: 'ok' }
    })

    expect(chat.isStreaming('a')).toBe(false)
    expect(chat.isStreaming('b')).toBe(true)
  })

  it('未查看标记在切到该会话后清除', async () => {
    const chat = useChatStore()
    await chat.send('a', 'x')
    chat.handle({
      type: 'done',
      sessionId: 'a',
      runId: 1,
      message: { id: 'm1', role: 'assistant', text: 'ok' }
    })

    expect(chat.unseen.has('a')).toBe(true)
    chat.markSeen('a')
    expect(chat.unseen.has('a')).toBe(false)
  })

  it('按钮状态按会话独立：一个在跑时另一个不算在跑', async () => {
    const chat = useChatStore()
    await chat.send('a', 'x')

    expect(chat.isStreaming('a')).toBe(true)
    expect(chat.isStreaming('b')).toBe(false)
    expect(chat.isStreaming(null)).toBe(false)
  })

  it('send 失败时回滚占位', async () => {
    const chatApi = await import('@/api/chat')
    vi.mocked(chatApi.send).mockRejectedValueOnce(new Error('后端拒绝'))

    const chat = useChatStore()
    await expect(chat.send('a', 'x')).rejects.toThrow('后端拒绝')

    expect(chat.isStreaming('a')).toBe(false)
    expect(chat.error).toBe('后端拒绝')
  })

  it('forget 清掉会话的残留状态', async () => {
    const chat = useChatStore()
    await chat.send('a', 'x')
    chat.handle({
      type: 'done',
      sessionId: 'a',
      runId: 1,
      message: { id: 'm1', role: 'assistant', text: 'ok' }
    })

    chat.forget('a')

    expect(chat.isStreaming('a')).toBe(false)
    expect(chat.unseen.has('a')).toBe(false)
  })
})
