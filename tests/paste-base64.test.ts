import { describe, expect, it } from 'vitest'

/**
 * 与 ChatPane.vue 的 toBase64 等价实现。
 *
 * 原实现 btoa(String.fromCharCode(...bytes)) 在 >12 万字节时抛 RangeError，
 * 而正常截图基本都超过这个尺寸，等于"粘贴图片必崩"。
 */
function toBase64(buf: Uint8Array): string {
  const CHUNK = 0x8000
  let binary = ''
  for (let i = 0; i < buf.length; i += CHUNK) {
    binary += String.fromCharCode(...buf.subarray(i, i + CHUNK))
  }
  return btoa(binary)
}

function randomBytes(n: number): Uint8Array {
  const buf = new Uint8Array(n)
  for (let i = 0; i < n; i++) buf[i] = i % 256
  return buf
}

describe('粘贴附件 base64 编码', () => {
  it('大文件不再抛 RangeError', () => {
    expect(() => toBase64(randomBytes(130_000))).not.toThrow()
    expect(() => toBase64(randomBytes(2_000_000))).not.toThrow()
  })

  it('结果与标准编码逐字节一致', () => {
    for (const n of [0, 1, 100, 65535, 65536, 130_000, 500_000]) {
      const bytes = randomBytes(n)
      expect(toBase64(bytes)).toBe(Buffer.from(bytes).toString('base64'))
    }
  })

  it('空输入得到空字符串', () => {
    expect(toBase64(new Uint8Array(0))).toBe('')
  })
})
