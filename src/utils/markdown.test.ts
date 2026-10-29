import { describe, expect, it } from 'vitest'
import { extractFilePaths, softenLongParagraphs } from '@/utils/markdown'

describe('extractFilePaths', () => {
  it('不把正文里的斜杠词组当路径', () => {
    // 真实踩过的坑：这句被识别成 /Excel/PPT/PDF，渲染成卡片，点了报错
    expect(extractFilePaths('- 处理 Word/Excel/PPT/PDF 文件')).toEqual([])
    expect(extractFilePaths('运行 npm run build && ls /a/b/c')).toEqual([])
  })

  it('识别 Unix 风格的代码文件路径', () => {
    expect(extractFilePaths('见 /src/utils/markdown.ts 的实现')).toEqual([
      '/src/utils/markdown.ts'
    ])
    expect(extractFilePaths('路径 /home/user/project/main.py 里')).toEqual([
      '/home/user/project/main.py'
    ])
  })

  it('识别 Windows 盘符路径', () => {
    const got = extractFilePaths('打开 C:\\Users\\a\\b.md 看看')
    expect(got).toHaveLength(1)
    expect(got[0]).toContain('b.md')
  })

  it('相对路径不算：没有绝对定位依据', () => {
    expect(extractFilePaths('改 src/api/chat.ts 就行')).toEqual([])
  })

  it('剥掉尾部的标点', () => {
    expect(extractFilePaths('文件是 /a/b.ts.')).toEqual(['/a/b.ts'])
  })
})

describe('超长段落断行', () => {
  const long = '这是一句测试内容。'.repeat(60) // 约 540 字，无换行

  it('超过阈值的长段落按句号断成多段', () => {
    const out = softenLongParagraphs(long)
    expect(out.split('\n\n').length).toBeGreaterThan(1)
  })

  it('短段落原样保留', () => {
    const short = '这是一句很短的话。'
    expect(softenLongParagraphs(short)).toBe(short)
  })

  it('代码块不碰', () => {
    const fence = '`'.repeat(3)
    const code = fence + '\n' + 'x'.repeat(400) + '\n' + fence
    expect(softenLongParagraphs(code)).toBe(code)
  })

  it('表格不碰', () => {
    const table = '| ' + 'a'.repeat(400) + ' |'
    expect(softenLongParagraphs(table)).toBe(table)
  })

  it('标题与列表不碰', () => {
    const h = '# ' + 'x'.repeat(400)
    const li = '- ' + 'x'.repeat(400)
    expect(softenLongParagraphs(h)).toBe(h)
    expect(softenLongParagraphs(li)).toBe(li)
  })
})
