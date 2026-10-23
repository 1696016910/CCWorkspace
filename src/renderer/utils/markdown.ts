import { marked } from 'marked'
import DOMPurify from 'dompurify'

/**
 * 把模型输出的 Markdown 渲染成安全 HTML。
 *
 * 模型输出属于不可信内容（可能夹带脚本），必须经 DOMPurify 净化。
 */
export function renderMarkdown(source: string): string {
  const raw = marked.parse(source, {
    breaks: true,
    gfm: true,
    async: false
  }) as string

  return DOMPurify.sanitize(raw, {
    ADD_ATTR: ['target', 'data-file'],
    FORBID_TAGS: ['style', 'script', 'iframe', 'object', 'embed'],
    FORBID_ATTR: ['onerror', 'onload', 'onclick']
  })
}

/** 从文本里识别出像文件路径的片段，供点击打开 */
export function extractFilePaths(text: string): string[] {
  const re = /(?:^|\s)((?:[A-Za-z]:[\\/]|\/)[^\s"'`<>|]+?\.[A-Za-z0-9]{1,8})(?=\s|$|[.,;:])/g
  const out = new Set<string>()
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    if (m[1]) out.add(m[1])
  }
  return [...out]
}
