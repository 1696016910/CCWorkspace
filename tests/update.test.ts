import { describe, expect, it, vi } from 'vitest'

// update.ts 顶层会创建 updater 实例并读数据库，测试里全部打桩，
// 只验证纯逻辑：错误翻译与代理前缀拼接。
vi.mock('electron', () => ({
  app: { getVersion: () => '0.1.0', isPackaged: false },
  BrowserWindow: { getAllWindows: () => [] },
  shell: { openExternal: () => undefined }
}))
vi.mock('electron-updater', () => ({
  default: { autoUpdater: { on: () => undefined } }
}))

let stored = ''

vi.mock('../src/main/db', () => ({
  getDb: () => ({
    prepare: () => ({
      get: () => (stored ? { value: stored } : undefined),
      run: (_k: string, v: string) => {
        stored = v
      }
    })
  })
}))

const { friendlyError, getFeedPrefix, setFeedPrefix, feedUrl } = await import(
  '../src/main/services/update'
)

describe('friendlyError 错误翻译', () => {
  it('404 翻译成可操作提示，不透出响应头', () => {
    const msg = friendlyError('Cannot find channel "latest.yml" update info: HttpError: 404\nHeaders: {...}')
    expect(msg).toContain('404')
    expect(msg).not.toContain('Headers')
  })

  it('网络类错误提示填代理', () => {
    expect(friendlyError('getaddrinfo ENOTFOUND github.com')).toContain('代理')
    expect(friendlyError('net::ERR_CONNECTION_REFUSED')).toContain('代理')
  })

  it('校验失败提示文件可能被代理损坏', () => {
    expect(friendlyError('sha512 checksum mismatch')).toContain('代理')
  })

  it('未分类错误只保留首行并截断', () => {
    const long = `${'x'.repeat(500)}\nsecond line`
    const msg = friendlyError(long)
    expect(msg).not.toContain('second line')
    expect(msg.length).toBeLessThanOrEqual(200)
  })
})

describe('代理前缀', () => {
  it('默认留空表示直连', () => {
    expect(getFeedPrefix()).toBe('')
    expect(feedUrl()).toBe('https://github.com/__OWNER__/CCWorkspace/releases/latest/download/')
  })

  it('填写后拼接在前，并去掉尾部斜杠避免双斜杠', () => {
    setFeedPrefix('https://proxy.example.com/')
    expect(getFeedPrefix()).toBe('https://proxy.example.com')
    expect(feedUrl()).toBe(
      'https://proxy.example.com/https://github.com/__OWNER__/CCWorkspace/releases/latest/download/'
    )
  })

  it('拒绝非 http(s) 前缀', () => {
    expect(() => setFeedPrefix('proxy.example.com')).toThrow('http')
    expect(() => setFeedPrefix('ftp://x')).toThrow('http')
  })

  it('清空后恢复直连', () => {
    setFeedPrefix('')
    expect(feedUrl()).toContain('https://github.com/')
    expect(feedUrl()).not.toContain('proxy')
  })
})
