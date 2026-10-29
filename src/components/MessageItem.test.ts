import { describe, expect, it } from 'vitest'

/** 毫秒 → "1m 35s" / "4.2s"，与 CLI 的 "Cooked for ..." 风格一致 */
function fmtDuration(ms: number): string {
  const total = Math.round(ms / 1000)
  if (total < 60) return `${(ms / 1000).toFixed(1)}s`
  const m = Math.floor(total / 60)
  const rest = total % 60
  return rest ? `${m}m ${rest}s` : `${m}m`
}

describe('本轮耗时格式化', () => {
  it('一分钟以内用秒，保留一位小数', () => {
    expect(fmtDuration(4200)).toBe('4.2s')
    expect(fmtDuration(30400)).toBe('30.4s')
  })

  it('59.9 秒进位成 1m，不显示 59.9s', () => {
    // 四舍五入到整秒是 60，应该走分钟分支，而不是显示 "59.9s"
    expect(fmtDuration(59900)).toBe('1m')
  })

  it('超过一分钟用 分+秒', () => {
    expect(fmtDuration(95000)).toBe('1m 35s')
    expect(fmtDuration(120000)).toBe('2m')
  })

  it('整分钟不带 0s', () => {
    expect(fmtDuration(60000)).toBe('1m')
    expect(fmtDuration(180000)).toBe('3m')
  })

  it('秒数四舍五入', () => {
    // 59.6s 应进位成 1m，而不是显示 0m 60s
    expect(fmtDuration(59600)).toBe('1m')
  })
})
