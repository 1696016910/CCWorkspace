import { describe, expect, it } from 'vitest'
import { extractFilePaths } from '@/utils/markdown'

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
