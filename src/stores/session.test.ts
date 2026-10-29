import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const created: string[] = []

vi.mock('@/api/session', () => ({
  list: vi.fn(async () => [
    {
      id: 'real-1',
      workspaceId: 'w1',
      title: '已有会话',
      updatedAt: new Date().toISOString(),
      messageCount: 2,
      running: false,
      renamed: true,
      contextPercent: 5,
      contextTokens: 50000,
      contextMax: 1000000,
      pinnedAt: null
    }
  ]),
  listDeleted: vi.fn(async () => []),
  create: vi.fn(async (workspaceId: string) => {
    const id = `new-${created.length + 1}`
    created.push(id)
    return {
      id,
      workspaceId,
      title: '',
      updatedAt: new Date().toISOString(),
      messageCount: 0,
      running: false,
      renamed: false,
      contextPercent: 0,
      contextTokens: 0,
      contextMax: 0,
      pinnedAt: null
    }
  }),
  messages: vi.fn(async () => []),
  remove: vi.fn(async () => {}),
  rename: vi.fn(async () => {}),
  pin: vi.fn(async () => {}),
  restore: vi.fn(async () => {}),
  purge: vi.fn(async () => {}),
  duplicate: vi.fn(async () => ({})),
  messageCount: vi.fn(async () => 0)
}))

import { useSessionStore } from '@/stores/session'

describe('草稿会话', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    created.length = 0
  })

  it('点新建不落库，只开一个空白对话页', async () => {
    const store = useSessionStore()
    await store.create('w1')
    const api = await import('@/api/session')

    expect(api.create).not.toHaveBeenCalled()
    expect(store.isDraft).toBe(true)
    // 草稿也应有 current，界面才能显示标题栏与输入框
    expect(store.current).toBeTruthy()
    expect(store.current?.title).toBe('')
    expect(store.messages).toEqual([])
  })

  it('草稿不在会话列表里 —— 左侧树不该出现空条目', async () => {
    const store = useSessionStore()
    await store.load()
    const before = store.list.length

    await store.create('w1')

    expect(store.list.length).toBe(before)
  })

  it('发第一条消息时才真正建会话', async () => {
    const store = useSessionStore()
    await store.create('w1')
    const api = await import('@/api/session')

    const id = await store.commitDraft()

    expect(api.create).toHaveBeenCalledWith('w1')
    expect(id).toBe('new-1')
    expect(store.isDraft).toBe(false)
    expect(store.activeId).toBe('new-1')
  })

  it('切到别的会话会丢弃草稿', async () => {
    const store = useSessionStore()
    await store.load()
    await store.create('w1')
    expect(store.isDraft).toBe(true)

    await store.select('real-1')

    expect(store.isDraft).toBe(false)
    expect(store.activeId).toBe('real-1')
  })

  it('草稿会话必须有可用的 id —— 输入框靠它存草稿', async () => {
    const store = useSessionStore()
    await store.create('w1')

    // 曾经的 bug：草稿的 activeId 是 null，输入框用 activeId 做 key，
    // setter 里 `if (!id) return` 把每个字都丢了 —— 表现为「打不进字、发不出去」。
    expect(store.activeId).toBeNull()
    expect(store.current?.id).toBeTruthy()
    expect(store.current?.id).toBe('__draft__')
  })

  it('草稿的上下文用量是未知（max 为 0），界面据此显示「—」', async () => {
    const store = useSessionStore()
    await store.create('w1')

    expect(store.current?.contextMax).toBe(0)
    expect(store.current?.contextTokens).toBe(0)
  })
})
