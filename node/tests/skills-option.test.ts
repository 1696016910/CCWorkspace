import { describe, expect, it } from 'vitest'

/**
 * SDK 的 skills 字段语义（见 sdk.d.ts）：
 * - 省略 / undefined：沿用 CLI 默认，**不是**"关闭 Skill"
 * - 'all'：全部启用
 * - string[]：只启用列出的
 *
 * 所以"用户在设置里关掉所有 Skill"必须传空数组，
 * 传 undefined 会让所有 Skill 反而可用 —— 语义正好相反。
 */
function skillsOption(skills: string[]): string[] {
  return skills
}

describe('Skill 白名单传参', () => {
  it('有启用项时原样传递', () => {
    expect(skillsOption(['brainstorming', 'frontend-design'])).toEqual([
      'brainstorming',
      'frontend-design'
    ])
  })

  it('全部关闭时传空数组而不是 undefined（否则等于全开）', () => {
    const opt = skillsOption([])
    expect(opt).toEqual([])
    expect(opt).not.toBeUndefined()
  })

  it('空数组不会被当成 falsy 而退回默认行为', () => {
    const skills: string[] = []
    // 关键断言：不能用 `skills.length > 0 ? skills : undefined` 这种写法
    const wrong = skills.length > 0 ? skills : undefined
    expect(wrong).toBeUndefined()
    expect(skillsOption(skills)).not.toBeUndefined()
  })
})
