import { describe, expect, it } from 'vitest'

/**
 * 分页切片逻辑（与 stores/session.ts 的 loadEarlier 等价）。
 *
 * 关键约束：切片基准必须是"发起请求时界面已有多少条"，
 * 不能用 await 之后的长度——请求期间新消息到达、库里比界面多
 * （压缩摘要、失败轮次）都会让切片错位，表现为消息重复或永久漏载。
 */
const PAGE_SIZE = 80

function loadEarlier(
  all: { id: string }[],
  messages: { id: string }[],
  messageTotal: number
): { messages: { id: string }[]; total: number } {
  const already = messages.length
  const extra = Math.max(0, all.length - messageTotal)
  const loadedFromEnd = already + extra
  const start = Math.max(0, all.length - loadedFromEnd - PAGE_SIZE)
  const end = Math.max(0, all.length - loadedFromEnd)
  const page = all.slice(start, end)
  const seen = new Set(messages.map((m) => m.id))
  return {
    messages: page.filter((m) => !seen.has(m.id)).concat(messages),
    total: all.length
  }
}

const make = (n: number) => Array.from({ length: n }, (_, i) => ({ id: `m${i}` }))
const tail = (all: { id: string }[], n: number) => all.slice(-n)

describe('消息分页', () => {
  it('常规情况：160 条已加载最后 80 条，加载到前 80 条', () => {
    const all = make(160)
    const msgs = tail(all, PAGE_SIZE)
    const r = loadEarlier(all, msgs, 160)
    expect(r.messages).toHaveLength(160)
    expect(r.messages[0]!.id).toBe('m0')
    expect(new Set(r.messages.map((m) => m.id)).size).toBe(160)
  })

  it('消息重复不会出现（库里比界面多 1 条的压缩摘要场景）', () => {
    const all = make(161)
    // 界面只加载了最后 80 条，但 messageTotal 还是 160（摘要没回显）
    const msgs = tail(all, PAGE_SIZE)
    const r = loadEarlier(all, msgs, 160)
    const ids = r.messages.map((m) => m.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('不遗漏任何消息（连续翻页到底）', () => {
    const all = make(200)
    let msgs = tail(all, PAGE_SIZE)
    let total = 200
    for (let i = 0; i < 5 && msgs.length < total; i++) {
      const r = loadEarlier(all, msgs, total)
      msgs = r.messages
      total = r.total
    }
    expect(msgs).toHaveLength(200)
    expect(msgs[0]!.id).toBe('m0')
    expect(new Set(msgs.map((m) => m.id)).size).toBe(200)
  })

  it('正好 80 条：不触发加载', () => {
    const all = make(80)
    expect(all.length <= PAGE_SIZE).toBe(true)
  })

  it('81 条：第一次加载拿到全部', () => {
    const all = make(81)
    const msgs = tail(all, PAGE_SIZE)
    const r = loadEarlier(all, msgs, 81)
    expect(r.messages).toHaveLength(81)
  })

  it('单条边界：1 条消息时不越界', () => {
    const all = make(1)
    const r = loadEarlier(all, all, 1)
    expect(r.messages).toHaveLength(1)
  })
})
