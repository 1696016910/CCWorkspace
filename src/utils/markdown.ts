import { marked } from 'marked'
import createDOMPurify from 'dompurify'
import hljs from 'highlight.js/lib/common'

/**
 * dompurify 默认导出是工厂函数：浏览器里要传 window 才能拿到带 sanitize 的实例。
 * Tauri 跑在 WebView 里，构造一次复用即可。
 */
const purifier = createDOMPurify(window as unknown as Parameters<typeof createDOMPurify>[0])

/** 提取代码块语言标签，展示在复制栏左侧 */
function languageOf(className: string | undefined): string {
  if (!className) return ''
  const m = /language-([\w+#-]+)/.exec(className)
  return m?.[1] ?? ''
}

/**
 * 给每个 <pre> 包一层带语言标签与复制按钮的容器。
 *
 * Markdown 渲染结果是 HTML 字符串，挂不了 Vue 事件，
 * 复制按钮通过 `data-copy` 标记 + 事件委托实现（见 MessageItem）。
 */
function decorateCodeBlocks(html: string, highlight: boolean): string {
  return html.replace(
    /<pre><code([^>]*)>([\s\S]*?)<\/code><\/pre>/g,
    (_all, attrs: string, body: string) => {
      const langMatch = /class="([^"]*)"/.exec(attrs)
      const lang = languageOf(langMatch?.[1]) || 'text'

      let highlighted = body
      try {
        const decoded = body
          .replace(/&lt;/g, '<')
          .replace(/&gt;/g, '>')
          .replace(/&amp;/g, '&')
          .replace(/&quot;/g, '"')
        // 流式期间不高亮：未闭合围栏每帧都要全量重高亮，
        // 4 万字符的代码流能把 CPU 打满，永远追不上输出速度
        const language = highlight && hljs.getLanguage(lang) ? lang : null
        // highlight.js 返回的是安全 HTML（内容已转义、只含 span），
        // 不能再整体转义，否则标签本身会被当文本显示
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
    }
  )
}

/**
 * 渲染缓存。
 *
 * 切会话时会把消息全部重渲染（80 条约 200ms），而入库内容不再变化，
 * 按原文做 key 缓存后来回切换几乎零成本。
 *
 * 容量按"总字符数"限，不按条数：流式每帧全文都在涨，
 * 300 条大内容能吃到 200MB+，按条数限流兜不住。
 */
const cache = new Map<string, string>()
const CACHE_MAX_CHARS = 2_000_000
let cacheChars = 0

/**
 * 把模型输出的 Markdown 渲染成安全 HTML。
 *
 * 模型输出属于不可信内容（可能夹带脚本），必须经 DOMPurify 净化。
 *
 * @param cacheable 流式输出必须传 false：每帧都是新字符串，缓存无复用价值，
 *   还会把历史消息的缓存全挤掉。
 */
export function renderMarkdown(
  source: string | null | undefined,
  cacheable = true,
  highlight = true
): string {
  if (typeof source !== 'string' || source === '') return ''

  if (cacheable) {
    const hit = cache.get(source)
    if (hit !== undefined) return hit
  }

  let raw: string
  try {
    raw = marked.parse(source, { breaks: true, gfm: true, async: false }) as string
  } catch {
    // 畸形输入不该让整条消息消失，降级为纯文本
    raw = source.replace(/[<>&]/g, (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;' })[c] ?? c)
  }

  const decorated = decorateCodeBlocks(raw, highlight)

  const result = purifier.sanitize(decorated, {
    ADD_ATTR: ['target', 'data-file', 'data-copy', 'type', 'class'],
    ADD_TAGS: ['button', 'span'],
    // map/area 必须禁：<area href> 是链接但 closest('a') 抓不到，
    // 点击会让窗口直接导航走，绕过全部链接拦截
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
    // style 属性可用于 CSS 注入与外部请求外带，聊天内容不需要它
    // hidden 也要禁：能藏内容和用户所见不一致
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

  if (!cacheable) return result

  while (cacheChars + source.length > CACHE_MAX_CHARS && cache.size > 0) {
    const oldest = cache.keys().next().value
    if (oldest === undefined) break
    const oldValue = cache.get(oldest) ?? ''
    cacheChars -= oldest.length + oldValue.length
    cache.delete(oldest)
  }
  cache.set(source, result)
  cacheChars += source.length + result.length
  return result
}

/**
 * 从文本里识别像文件路径的片段，供点击打开。
 *
 * 同时支持 Windows（C:\a\b.txt）与 POSIX（/a/b.txt）。
 * 结尾标点会剥掉，避免把句号当成文件名的一部分。
 */
/**
 * 已知的文本/代码扩展名。
 *
 * 只认这些：原来的正则把「以斜杠开头、中间还有斜杠的任意串」都当路径，
 * 于是正文里「处理 Word/Excel/PPT/PDF 文件」会被当成 `/Excel/PPT/PDF`，
 * 渲染成可点卡片，点了报「只能预览当前工作区内的文件」。
 */
const FILE_EXTS =
  'md|markdown|txt|log|json|jsonc|ya?ml|toml|ini|conf|cfg|env|' +
  'ts|tsx|js|jsx|mjs|cjs|vue|svelte|css|scss|sass|less|html|htm|xml|svg|' +
  'rs|go|py|java|kt|cs|c|cc|cpp|h|hpp|rb|php|swift|sh|bash|zsh|ps1|bat|cmd|' +
  'sql|graphql|proto|lua|dart|ex|exs|scala|clj|hs|pl'

const FILE_PATH_RE = new RegExp(
  // 盘符路径，或 Unix 风格绝对路径，都强制要求扩展名
  `(?:[A-Za-z]:[\\\\/][^\\s"'\`<>|]*?\\.(?:${FILE_EXTS})\\b` +
    `|(?<![\\w/])/(?:[^\\s"'\`<>|/]+/)*[^\\s"'\`<>|/]+\\.(?:${FILE_EXTS})\\b)`,
  'gi'
)

export function extractFilePaths(text: string): string[] {
  const out = new Set<string>()
  let m: RegExpExecArray | null
  FILE_PATH_RE.lastIndex = 0
  while ((m = FILE_PATH_RE.exec(text)) !== null) {
    const raw = m[0].replace(/[.,;:)\]，。；：]+$/, '')
    if (raw.length > 3) out.add(raw)
  }
  return [...out]
}
