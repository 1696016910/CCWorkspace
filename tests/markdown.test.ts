// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { extractFilePaths, renderMarkdown } from '../src/renderer/utils/markdown'

describe('renderMarkdown 安全性', () => {
  it('普通 Markdown 正常渲染', () => {
    const html = renderMarkdown('**粗体** 和 `代码`')
    expect(html).toContain('<strong>粗体</strong>')
    expect(html).toContain('<code>代码</code>')
  })

  it('剥离 script 标签', () => {
    const html = renderMarkdown('正常<script>alert(1)</script>结尾')
    expect(html).not.toContain('<script')
    expect(html).not.toContain('alert(1)')
  })

  it('剥离内联事件处理器', () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)">')
    expect(html).not.toContain('onerror')
  })

  it('剥离 javascript: 链接', () => {
    const html = renderMarkdown('[点我](javascript:alert(1))')
    expect(html).not.toContain('javascript:')
  })

  it('iframe 被移除', () => {
    const html = renderMarkdown('<iframe src="https://evil"></iframe>')
    expect(html).not.toContain('<iframe')
  })

  it('空值与非法类型返回空串', () => {
    expect(renderMarkdown('')).toBe('')
    expect(renderMarkdown(null)).toBe('')
    expect(renderMarkdown(undefined)).toBe('')
  })

  it('代码块中的尖括号被转义而非当作标签', () => {
    const html = renderMarkdown('```\n<div>hi</div>\n```')
    expect(html).toContain('&lt;div&gt;')
    expect(html).not.toContain('<div>hi</div>')
  })
})

