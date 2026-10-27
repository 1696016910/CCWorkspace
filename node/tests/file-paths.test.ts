// @vitest-environment jsdom
import { describe, expect, it } from 'vitest'
import { extractFilePaths } from '../src/renderer/utils/markdown'

const WIN = String.fromCharCode(67, 58, 92) + 'Windows' + String.fromCharCode(92) + 'win.ini'
const WIN_TXT = String.fromCharCode(67, 58, 92) + 'a' + String.fromCharCode(92) + 'b.txt'

describe('extractFilePaths', () => {
  it('识别 Windows 路径', () => {
    expect(extractFilePaths(`配置在 ${WIN} 里`)).toEqual([WIN])
  })

  it('识别 POSIX 路径', () => {
    expect(extractFilePaths('见 /etc/hosts 文件')).toEqual(['/etc/hosts'])
  })

  it('剥掉结尾标点', () => {
    expect(extractFilePaths(`打开 ${WIN_TXT}.`)).toEqual([WIN_TXT])
  })

  it('同一路径只出现一次', () => {
    expect(extractFilePaths(`${WIN_TXT} 和 ${WIN_TXT}`)).toHaveLength(1)
  })

  it('普通文本不误报', () => {
    expect(extractFilePaths('这是一段没有路径的文字')).toEqual([])
  })
})
