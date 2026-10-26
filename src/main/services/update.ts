import { app, BrowserWindow, shell } from 'electron'
import electronUpdater from 'electron-updater'
import { getDb } from '../db'
import type { UpdateStatus } from '../../shared/types'

const { autoUpdater } = electronUpdater

/** 产物发布所在的公开仓库，与 package.json 的 repository 保持一致 */
const REPO = '__OWNER__/CCWorkspace'
const FEED_KEY = 'update_feed_prefix'

let status: UpdateStatus | null = null
let wired = false

/**
 * GitHub 的 `releases/latest/download` 永远指向最新一版，
 * 所以发布新版本后这个地址不变，不需要随版本号改写。
 */
function baseFeed(): string {
  return `https://github.com/${REPO}/releases/latest/download/`
}

/** 读取代理前缀，留空表示直连 */
export function getFeedPrefix(): string {
  try {
    const row = getDb().prepare("SELECT value FROM meta WHERE key = ?").get(FEED_KEY) as
      | { value?: string }
      | undefined
    return row?.value ?? ''
  } catch {
    return ''
  }
}

export function setFeedPrefix(prefix: string): void {
  const trimmed = prefix.trim().replace(/\/+$/, '')
  if (trimmed && !/^https?:\/\//i.test(trimmed)) {
    throw new Error('代理前缀必须以 http:// 或 https:// 开头')
  }
  getDb()
    .prepare(
      'INSERT INTO meta(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value'
    )
    .run(FEED_KEY, trimmed)
}

/**
 * 实际请求的更新地址。
 * 代理只加在前面，后面的 GitHub 绝对地址原样保留，
 * 因为 latest.yml 里的文件名是相对路径，下载时会继承同一个前缀。
 */
export function feedUrl(): string {
  const prefix = getFeedPrefix()
  return prefix ? `${prefix}/${baseFeed()}` : baseFeed()
}

function snapshot(patch: Partial<UpdateStatus> = {}): UpdateStatus {
  const base: UpdateStatus = status ?? {
    state: 'idle',
    currentVersion: app.getVersion(),
    feed: feedUrl()
  }
  status = { ...base, ...patch, currentVersion: app.getVersion(), feed: feedUrl() }
  return status
}

function push(patch: Partial<UpdateStatus>): void {
  const next = snapshot(patch)
  for (const w of BrowserWindow.getAllWindows()) {
    w.webContents.send('update:event', next)
  }
}

function messageOf(e: unknown): string {
  return e instanceof Error ? e.message : String(e)
}

/**
 * electron-updater 的原始错误带有完整响应头与堆栈，直接展示会把设置页撑爆。
 * 这里只保留一句人能读懂的原因，其余信息仍通过日志留存。
 */
export function friendlyError(raw: string): string {
  if (process.platform === 'darwin' && /code sign|signature/i.test(raw)) {
    return 'macOS 需要 Apple 开发者签名才能自动更新，请打开发布页手动下载'
  }
  if (/\b404\b/.test(raw)) {
    return '未找到更新信息（404）：确认仓库已发布 Release，或检查代理前缀是否正确'
  }
  if (/ENOTFOUND|EAI_AGAIN|ETIMEDOUT|ECONNREFUSED|net::/i.test(raw)) {
    return '网络不可达：可在下方填写代理前缀后重试'
  }
  if (/sha512|checksum/i.test(raw)) {
    return '更新包校验失败：文件可能已被代理损坏，请重试或改为直连'
  }
  // 未分类异常只保留首行，避免整段响应头进入界面
  return raw.split('\n')[0].slice(0, 200)
}

function wire(): void {
  if (wired) return
  wired = true

  // 检查与下载分开：发现新版本后由用户决定何时下载 200MB 的包
  autoUpdater.autoDownload = false
  autoUpdater.autoInstallOnAppQuit = true

  autoUpdater.on('checking-for-update', () => push({ state: 'checking', message: undefined }))
  autoUpdater.on('update-available', (info) =>
    push({ state: 'available', version: info.version, message: undefined })
  )
  autoUpdater.on('update-not-available', () =>
    push({ state: 'idle', message: '已是最新版本' })
  )
  autoUpdater.on('download-progress', (p) =>
    push({ state: 'downloading', percent: Math.round(p.percent) })
  )
  autoUpdater.on('update-downloaded', (info) =>
    push({ state: 'downloaded', version: info.version, percent: 100 })
  )
  autoUpdater.on('error', (e) => push({ state: 'error', message: friendlyError(messageOf(e)) }))
}

export function getUpdateStatus(): UpdateStatus {
  return snapshot()
}

export async function checkForUpdate(): Promise<UpdateStatus> {
  if (!app.isPackaged) {
    push({ state: 'idle', message: '开发模式不检查更新' })
    return snapshot()
  }
  wire()
  autoUpdater.setFeedURL({ provider: 'generic', url: feedUrl() })
  push({ state: 'checking', message: undefined })
  try {
    await autoUpdater.checkForUpdates()
  } catch (e) {
    push({ state: 'error', message: friendlyError(messageOf(e)) })
  }
  return snapshot()
}

export async function downloadUpdate(): Promise<UpdateStatus> {
  if (status?.state !== 'available') throw new Error('当前没有可下载的版本')
  try {
    await autoUpdater.downloadUpdate()
  } catch (e) {
    push({ state: 'error', message: friendlyError(messageOf(e)) })
  }
  return snapshot()
}

export function installUpdate(): void {
  if (status?.state !== 'downloaded') throw new Error('更新尚未下载完成')
  autoUpdater.quitAndInstall()
}

/** 启动后延迟检查，失败保持静默，不打扰使用 */
export function checkOnStartup(): void {
  if (!app.isPackaged) return
  setTimeout(() => void checkForUpdate(), 5000)
}

/** 打开发布页，用于自动更新不可用时手动下载 */
export function openReleases(): void {
  void shell.openExternal(`https://github.com/${REPO}/releases`)
}
