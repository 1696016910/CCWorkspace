// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { renderMarkdown } from '../src/renderer/utils/markdown'

describe('renderMarkdown 代码块增强', () => {
  it('代码块带语言标签与复制按钮', () => {
    const html = renderMarkdown('```js\nconst a = 1\n```')
    expect(html).toContain('code-block')
    expect(html).toContain('code-copy')
    expect(html).toContain('code-lang')
    expect(html).toContain('js')
  })

  it('无语言的代码块标记为 text', () => {
    const html = renderMarkdown('```\nplain\n```')
    expect(html).toContain('>text<')
  })

  it('代码块内文本可正确提取（未被重复转义）', () => {
    const html = renderMarkdown('```js\nconst a = 1\n```')
    // 不应出现被转义的 span 字面量
    expect(html).not.toContain('&lt;span')
    // 应包含高亮后的 span
    expect(html).toContain('hljs-keyword')
  })

  it('未闭合代码块不抛错', () => {
    expect(() => renderMarkdown('```js\nconst a = 1')).not.toThrow()
  })

  it('代码块中的 HTML 被安全处理', () => {
    const html = renderMarkdown('```html\n<script>alert(1)</script>\n```')
    expect(html).not.toContain('<script>alert')
  })
})

describe('renderMarkdown 其他格式', () => {
  it('表格渲染', () => {
    const html = renderMarkdown('| a | b |\n|---|---|\n| 1 | 2 |')
    expect(html).toContain('<table')
    expect(html).toContain('<th')
  })

  it('有序与无序列表', () => {
    expect(renderMarkdown('- a\n- b')).toContain('<ul')
    expect(renderMarkdown('1. a\n2. b')).toContain('<ol')
  })

  it('引用块', () => {
    expect(renderMarkdown('> quote')).toContain('<blockquote')
  })

  it('行内代码', () => {
    expect(renderMarkdown('`code`')).toContain('<code>code</code>')
  })

  it('链接保留但剥离 javascript 协议', () => {
    const ok = renderMarkdown('[x](https://a.com)')
    expect(ok).toContain('href="https://a.com"')
    const bad = renderMarkdown('[x](javascript:alert(1))')
    expect(bad).not.toContain('javascript:')
  })
})
