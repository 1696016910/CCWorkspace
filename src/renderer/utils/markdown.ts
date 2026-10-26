import { marked } from 'marked'
import createDOMPurify from 'dompurify'
import hljs from 'highlight.js/lib/common'

/**
 * dompurify 的默认导出是一个工厂函数：浏览器里需要传入 window
 * 才能拿到带 sanitize 的实例。项目固定运行在 Electron 渲染进程，
 * 因此这里直接用全局 window 构造一次并复用。
 */
const purifier = createDOMPurify(window as unknown as Parameters<typeof createDOMPurify>[0])

/** 提取代码块语言标签，用于展示在复制栏左侧 */
function languageOf(className: string | undefined): string {
  if (!className) return ''
  const m = /language-([\w+#-]+)/.exec(className)
  return m?.[1] ?? ''
}

/**
 * 给每个 <pre> 包一层带语言标签与复制按钮的容器。
 *
 * Markdown 渲染是纯 HTML 字符串，无法直接挂 Vue 事件，因此复制按钮
 * 通过 `data-copy` 标记 + 事件委托实现（见 MessageItem）。
 */
function decorateCodeBlocks(html: string): string {
  return html.replace(/<pre><code([^>]*)>([\s\S]*?)<\/code><\/pre>/g, (_all, attrs, body) => {
    const langMatch = /class="([^"]*)"/.exec(attrs)
    const lang = languageOf(langMatch?.[1]) || 'text'

    // 高亮：已知语言才着色，未知语言保持原文，避免报错
    let highlighted = body
    try {
      const decoded = body
        .replace(/&lt;/g, '<')
        .replace(/&gt;/g, '>')
        .replace(/&amp;/g, '&')
        .replace(/&quot;/g, '"')
      const language = hljs.getLanguage(lang) ? lang : null
      // highlight.js 返回的就是安全 HTML（内容已转义、只含 span 标签），
      // 不能再整体转义，否则会把标签本身显示成文本
      const out = language ? hljs.highlight(decoded, { language }).value : null
      if (out) highlighted = out
    } catch {
      // 高亮失败就用原始转义内容
    }

    return (
      '<div class="code-block">' +
      '<div class="code-head">' +
      `<span class="code-lang">${lang}</span>` +
      '<button class="code-copy" data-copy type="button">复制</button>' +
      '</div>' +
      `<pre><code${attrs}>${highlighted}</code></pre>` +
      '</div>'
    )
  })
}

/**
 * 把模型输出的 Markdown 渲染成安全 HTML。
 *
 * 模型输出属于不可信内容（可能夹带脚本），必须经 DOMPurify 净化。
 */
/**
 * 渲染结果缓存。
 *
 * 切会话时会把列表里的消息全部重渲染一次（实测 80 条约 200ms），
 * 而消息内容一旦入库就不再变化。按原文做 key 缓存后，
 * 来回切换几乎零成本。上限 300 条，避免长会话把内存吃满。
 */
const cache = new Map<string, string>()
const CACHE_MAX = 300

export function renderMarkdown(source: string | null | undefined): string {
  if (typeof source !== 'string' || source === '') return ''

  const hit = cache.get(source)
  if (hit !== undefined) return hit

  let raw: string
  try {
    raw = marked.parse(source, {
      breaks: true,
      gfm: true,
      async: false
    }) as string
  } catch {
    // 极端畸形输入不该让整条消息消失，降级为纯文本
    raw = source.replace(/[<>&]/g, (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' })[c] ?? c)
  }

  const decorated = decorateCodeBlocks(raw)

  const result = purifier.sanitize(decorated, {
    ADD_ATTR: ['target', 'data-file', 'data-copy', 'type', 'class'],
    ADD_TAGS: ['button', 'span'],
    // map/area 必须禁：<area href> 是链接，但 closest('a') 抓不到它，
    // 点击后窗口会直接导航到目标地址，绕过全部链接拦截
    FORBID_TAGS: [
      'style',
      'script',
      'iframe',
      'object',
      'embed',
      'form',
      'input',
      'meta',
      'link',
      'map',
      'area',
      'base'
    ],
    // style 属性可被用来做 CSS 注入与外部请求外带（background:url(https://evil/...)），
    // 聊天内容没有任何场景需要它，直接禁掉
    // hidden 也要禁：可以藏一段内容，让"复制"拿到与用户所见不同的文本
    FORBID_ATTR: [
      'onerror',
      'onload',
      'onclick',
      'style',
      'formaction',
      'action',
      'srcdoc',
      'hidden',
      'usemap'
    ]
  })

  // 简单的 FIFO 淘汰：Map 保持插入顺序，超限时删最早的一条
  if (cache.size >= CACHE_MAX) {
    const oldest = cache.keys().next().value
    if (oldest !== undefined) cache.delete(oldest)
  }
  cache.set(source, result)
  return result
}

/**
 * 从文本里识别出像文件路径的片段，供点击打开。
 *
 * 同时支持 Windows（C:\a\b.txt）与 POSIX（/a/b.txt）。
 * 结尾标点会被剥掉，避免把句号当成文件名的一部分。
 */
export function extractFilePaths(text: string): string[] {
  // 至少两级：盘符路径要求带扩展名；POSIX 路径允许无扩展名（如 /etc/hosts）
  const re = /(?:[A-Za-z]:[\\/][^\s"'`<>|]*\.[A-Za-z0-9]{1,8}|\/[^\s"'`<>|]*\/[^\s"'`<>|]*)/g
  const out = new Set<string>()
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    const raw = m[0].replace(/[.,;:)\]，。；：]+$/, '')
    if (raw.length > 3) out.add(raw)
  }
  return [...out]
}
