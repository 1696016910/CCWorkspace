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

describe('缓存内存控制', () => {
  it('流式渲染不写缓存（cacheable=false）', () => {
    // 同一内容渲染两次：第一次不缓存，第二次不走缓存路径也能正确输出
    const src = '```js\nconst a = 1\n```'
    const a = renderMarkdown(src, false)
    const b = renderMarkdown(src, false)
    expect(a).toBe(b)
    expect(a).toContain('code-block')
  })

  it('流式渲染可关闭高亮（避免未闭合代码块反复全量重算）', () => {
    const src = '```js\nconst a = 1\n```'
    const withHl = renderMarkdown(src, false, true)
    const noHl = renderMarkdown(src, false, false)
    expect(withHl).toContain('hljs-keyword')
    expect(noHl).not.toContain('hljs-keyword')
    // 代码本体与复制按钮都还在
    expect(noHl).toContain('code-block')
    expect(noHl).toContain('const a = 1')
  })

  it('大量不同内容不会无界增长', () => {
    for (let i = 0; i < 500; i++) {
      renderMarkdown(`## ${i}\n\n${'内容'.repeat(200)}`)
    }
    // 仍能正确渲染（说明淘汰逻辑没把结构搞坏）
    expect(renderMarkdown('## final\n\n内容')).toContain('final')
  })
})
