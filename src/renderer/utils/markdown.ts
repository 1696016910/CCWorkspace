import { marked } from 'marked'
import createDOMPurify from 'dompurify'

/**
 * dompurify 的默认导出是一个工厂函数：浏览器里需要传入 window
 * 才能拿到带 sanitize 的实例。项目固定运行在 Electron 渲染进程，
 * 因此这里直接用全局 window 构造一次并复用。
 */
const purifier = createDOMPurify(window as unknown as Parameters<typeof createDOMPurify>[0])

/**
 * 把模型输出的 Markdown 渲染成安全 HTML。
 *
 * 模型输出属于不可信内容（可能夹带脚本），必须经 DOMPurify 净化。
 */
export function renderMarkdown(source: string | null | undefined): string {
  if (typeof source !== 'string' || source === '') return ''

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

  return purifier.sanitize(raw, {
    ADD_ATTR: ['target', 'data-file'],
    FORBID_TAGS: ['style', 'script', 'iframe', 'object', 'embed'],
    FORBID_ATTR: ['onerror', 'onload', 'onclick']
  })
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
