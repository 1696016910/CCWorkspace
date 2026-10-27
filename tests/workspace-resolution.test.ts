import { describe, expect, it } from 'vitest'

/**
 * 与 src/main/services/workspace.ts 的 resolveWorkspaceId 等价。
 *
 * 移除"默认工作区"后的语义：会话必须归属明确指定的工作区，
 * 不再有"没指定就用默认/第一个"的回落。
 */
function resolveWorkspaceId(requested: string | null): string | null {
  return requested
}

describe('会话归属工作区', () => {
  it('指定了工作区就原样返回', () => {
    expect(resolveWorkspaceId('ws-1')).toBe('ws-1')
  })

  it('未指定时返回 null，由调用方报错，不再回落到默认工作区', () => {
    expect(resolveWorkspaceId(null)).toBeNull()
  })

  it('空字符串也视为未指定', () => {
    expect(resolveWorkspaceId('')).toBe('')
  })
})
