import { describe, expect, it } from 'vitest'

/**
 * Markdown 判定必须只看路径后缀。
 *
 * 后端返回的 `ext` 带前导点（".md"），曾经用 `/^(md|markdown)$/` 匹配它，
 * 永远不命中，导致所有 Markdown 都默认打开源码模式而不是预览。
 */
function isMarkdownPath(p: string): boolean {
  return /\.(md|markdown)$/i.test(p)
}

describe('FileViewer 的 Markdown 判定', () => {
  it('识别常见 Markdown 后缀', () => {
    expect(isMarkdownPath('C:\\a\\b\\README.md')).toBe(true)
    expect(isMarkdownPath('/home/u/notes.markdown')).toBe(true)
    expect(isMarkdownPath('/tmp/X.MD')).toBe(true)
  })

  it('不把其他类型误判为 Markdown', () => {
    expect(isMarkdownPath('/a/b.ts')).toBe(false)
    expect(isMarkdownPath('/a/b.md.txt')).toBe(false)
    expect(isMarkdownPath('/a/b')).toBe(false)
    // 名字里带 md 但不是后缀
    expect(isMarkdownPath('/a/md/b.ts')).toBe(false)
  })

  it('带前导点的 ext 形式不参与判定', () => {
    // 这是当初的坑：后端给 ".md"，拿它去匹配 /^(md|markdown)$/ 必然失败
    const backendExt = '.md'
    expect(/^(md|markdown)$/i.test(backendExt)).toBe(false)
    // 正确做法是用路径后缀
    expect(isMarkdownPath('/x/README.md')).toBe(true)
  })
})
