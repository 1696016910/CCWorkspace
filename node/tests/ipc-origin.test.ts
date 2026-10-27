import { describe, expect, it } from 'vitest'

/**
 * 与 src/main/index.ts 的 isAppUrl 等价实现。
 *
 * 不能用字符串比较：Chromium 会把 frame.url 规范化成带尾斜杠的形式，
 * 而 electron-vite 注入的 ELECTRON_RENDERER_URL 没有尾斜杠。
 * 一旦比错，开发模式下所有 IPC 都会被判成"非法调用来源"，界面全空。
 */
function isAppUrl(url: string, app: string): boolean {
  try {
    const a = new URL(url)
    const b = new URL(app)
    return a.origin === b.origin && a.pathname === b.pathname
  } catch {
    return false
  }
}

const DEV = 'http://localhost:5173'
const PROD = 'file:///C:/Program%20Files/Claude/index.html'

describe('IPC 来源校验', () => {
  it('开发模式：带尾斜杠的 frame.url 与应用地址匹配', () => {
    expect(isAppUrl('http://localhost:5173/', DEV)).toBe(true)
  })

  it('开发模式：无尾斜杠也匹配', () => {
    expect(isAppUrl('http://localhost:5173', DEV)).toBe(true)
  })

  it('开发模式：hash 不影响判断', () => {
    expect(isAppUrl('http://localhost:5173/#/chat', DEV)).toBe(true)
  })

  it('开发模式：同源其它路径不匹配', () => {
    expect(isAppUrl('http://localhost:5173/evil', DEV)).toBe(false)
  })

  it('生产模式：安装路径含空格仍匹配（URL 已编码）', () => {
    expect(isAppUrl(PROD, PROD)).toBe(true)
  })

  it('生产模式：同目录其它页面不匹配', () => {
    expect(isAppUrl('file:///C:/Program%20Files/Claude/evil.html', PROD)).toBe(false)
  })

  it('UNC 共享不匹配', () => {
    expect(isAppUrl('file://attacker.example/share/x.html', PROD)).toBe(false)
  })

  it('畸形 URL 不匹配且不抛错', () => {
    expect(isAppUrl('not a url', PROD)).toBe(false)
    expect(isAppUrl('', PROD)).toBe(false)
  })
})
