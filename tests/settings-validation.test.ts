import { describe, expect, it } from 'vitest'
import { validateEnv } from '../src/main/services/claudeSettings'

describe('validateEnv', () => {
  it('合法输入通过', () => {
    expect(
      validateEnv([
        { key: 'ANTHROPIC_BASE_URL', value: 'https://example.com/v1' },
        { key: 'MAX_THINKING_TOKENS', value: '8000' }
      ])
    ).toBeNull()
  })

  it('空键名被拒绝', () => {
    expect(validateEnv([{ key: '  ', value: 'x' }])).toBe('键名不能为空')
  })

  it('重复键名被拒绝', () => {
    expect(
      validateEnv([
        { key: 'A', value: '1' },
        { key: 'A', value: '2' }
      ])
    ).toBe('键名重复：A')
  })

  it('含换行的值被拒绝', () => {
    expect(validateEnv([{ key: 'A', value: 'line1\nline2' }])).toContain('换行')
  })

  it('含 NUL 的值被拒绝（会让 CLI spawn 失败）', () => {
    expect(validateEnv([{ key: 'A', value: 'a\u0000b' }])).toContain('空字符')
  })
})

describe('validateEnv 键名校验', () => {
  it('拒绝 __proto__ 这类会污染原型的键名', () => {
    const msg = validateEnv([{ key: '__proto__', value: 'x' }])
    expect(msg).not.toBeNull()
    expect(msg).toContain('保留名')
  })

  it('拒绝非法字符', () => {
    expect(validateEnv([{ key: 'A B', value: 'x' }])).toContain('不合法')
    expect(validateEnv([{ key: '1ABC', value: 'x' }])).toContain('不合法')
    expect(validateEnv([{ key: 'A-B', value: 'x' }])).toContain('不合法')
  })

  it('接受正常的环境变量名', () => {
    expect(validateEnv([{ key: 'ANTHROPIC_AUTH_TOKEN', value: 'x' }])).toBeNull()
    expect(validateEnv([{ key: '_PRIVATE', value: 'x' }])).toBeNull()
  })
})
