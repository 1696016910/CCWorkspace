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
    expect(validateEnv([{ key: 'A', value: 'line1\nline2' }])).toBe('值不能包含换行：A')
  })
})
