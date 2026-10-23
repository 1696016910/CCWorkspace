import { describe, expect, it } from 'vitest'
import { deriveTitle } from '../src/main/services/session'

describe('deriveTitle', () => {
  it('普通文本原样作为标题', () => {
    expect(deriveTitle('把订单接口改成幂等')).toBe('把订单接口改成幂等')
  })

  it('超长标题截断到 40 字并加省略号', () => {
    const title = deriveTitle('一'.repeat(60))
    expect(title).toHaveLength(41)
    expect(title.endsWith('…')).toBe(true)
  })

  it('多行压成单行', () => {
    expect(deriveTitle('第一行\n\n第二行')).toBe('第一行 第二行')
  })

  it('空白内容给出占位标题', () => {
    expect(deriveTitle('   ')).toBe('(新会话)')
  })
})
