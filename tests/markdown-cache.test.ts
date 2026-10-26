// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { renderMarkdown } from '../src/renderer/utils/markdown'

describe('renderMarkdown 缓存', () => {
  it('相同内容返回相同结果（走缓存）', () => {
    const src = '## 标题\n\n正文内容'
    const a = renderMarkdown(src)
    const b = renderMarkdown(src)
    expect(a).toBe(b)
    expect(a).toContain('标题')
  })

  it('缓存不会串内容', () => {
    expect(renderMarkdown('第一条')).toContain('第一条')
    expect(renderMarkdown('第二条')).toContain('第二条')
    expect(renderMarkdown('第一条')).not.toContain('第二条')
  })

  it('缓存命中后仍然安全（不会被缓存绕过净化）', () => {
    const evil = '<div style="background:url(https://evil.example/x)">x</div>'
    renderMarkdown(evil)
    const again = renderMarkdown(evil)
    expect(again).not.toContain('evil.example')
    expect(again).not.toContain('style')
  })

  it('空内容不进入缓存也不报错', () => {
    expect(renderMarkdown('')).toBe('')
    expect(renderMarkdown(null)).toBe('')
    expect(renderMarkdown(undefined)).toBe('')
  })

  it('大量不同内容不会无限增长（有上限）', () => {
    for (let i = 0; i < 400; i++) renderMarkdown(`内容 ${i}`)
    // 超出上限后最早的内容被淘汰，重新渲染仍正确
    const r = renderMarkdown('内容 0')
    expect(r).toContain('内容 0')
  })
})
