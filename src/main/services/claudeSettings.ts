import { readFile, writeFile, rename, mkdir, copyFile, stat } from 'node:fs/promises'
import { join } from 'node:path'
import type { EnvEntry } from '../../shared/types'
import { claudeBackupsDir, claudeSettingsPath, pathExists } from './paths'

/** settings.json 的原始结构，未识别的字段原样保留 */
type RawSettings = Record<string, unknown> & {
  env?: Record<string, string>
  permissions?: { defaultMode?: string; allow?: string[]; deny?: string[] }
}

/** 模型相关字段，它们实际存在 env 里 */
export const MODEL_KEYS = [
  'ANTHROPIC_MODEL',
  'ANTHROPIC_DEFAULT_OPUS_MODEL',
  'ANTHROPIC_DEFAULT_SONNET_MODEL',
  'ANTHROPIC_DEFAULT_HAIKU_MODEL',
  'CLAUDE_CODE_EFFORT_LEVEL',
  'MAX_THINKING_TOKENS'
] as const

/** 最近一次读取是否因文件损坏而降级，供界面提示 */
let lastReadError: string | null = null

export function getLastReadError(): string | null {
  return lastReadError
}

export async function readRaw(): Promise<RawSettings> {
  const p = claudeSettingsPath()
  if (!(await pathExists(p))) {
    lastReadError = null
    return {}
  }
  try {
    const text = await readFile(p, 'utf-8')
    const parsed = JSON.parse(text)
    lastReadError = null
    return typeof parsed === 'object' && parsed !== null ? (parsed as RawSettings) : {}
  } catch {
    // 文件损坏时降级为空配置：让界面仍能打开并提示用户，
    // 而不是让整个应用不可用。
    lastReadError = 'settings.json 无法解析，配置以空值展示；修复后请重启应用'
    return {}
  }
}

/** 写入前记录 mtime，用于并发冲突检测 */
export async function statSettings(): Promise<number | null> {
  try {
    return (await stat(claudeSettingsPath())).mtimeMs
  } catch {
    return null
  }
}

/**
 * 安全写入：备份 → 临时文件 → rename。
 * expectedMtime 若提供，且当前文件 mtime 已变化，则拒绝写入。
 */
export async function writeSettings(
  next: RawSettings,
  expectedMtime?: number | null
): Promise<void> {
  const p = claudeSettingsPath()

  if (expectedMtime !== undefined && expectedMtime !== null) {
    const current = await statSettings()
    if (current !== null && Math.abs(current - expectedMtime) > 1) {
      throw new Error('文件已被其它进程修改，请重新加载后再保存')
    }
  }

  await mkdir(claudeBackupsDir(), { recursive: true })
  if (await pathExists(p)) {
    const stamp = new Date().toISOString().replace(/[:.]/g, '-')
    await copyFile(p, join(claudeBackupsDir(), `settings-${stamp}.json`))
    await pruneBackups()
  }

  const tmp = `${p}.tmp-${Date.now()}`
  await writeFile(tmp, JSON.stringify(next, null, 2), 'utf-8')
  await rename(tmp, p)
}

/** 只保留最近 20 份备份，避免长期使用后无限堆积 */
const MAX_BACKUPS = 20

async function pruneBackups(): Promise<void> {
  try {
    const { readdir, unlink } = await import('node:fs/promises')
    const dir = claudeBackupsDir()
    const files = (await readdir(dir))
      .filter((f) => f.startsWith('settings-') && f.endsWith('.json'))
      .sort()
    const excess = files.length - MAX_BACKUPS
    for (let i = 0; i < excess; i++) {
      await unlink(join(dir, files[i]!)).catch(() => {})
    }
  } catch {
    // 清理失败不影响保存
  }
}

function envObject(raw: RawSettings): Record<string, string> {
  return raw.env && typeof raw.env === 'object' ? raw.env : {}
}

export async function readEnv(): Promise<EnvEntry[]> {
  const env = envObject(await readRaw())
  return Object.entries(env).map(([key, value]) => ({ key, value: String(value) }))
}

/** 校验环境变量条目；返回错误信息，合法时返回 null */
export function validateEnv(entries: EnvEntry[]): string | null {
  const seen = new Set<string>()
  for (const e of entries) {
    const key = e.key.trim()
    if (!key) return '键名不能为空'
    if (seen.has(key)) return `键名重复：${key}`
    if (/[\r\n]/.test(e.value)) return `值不能包含换行：${key}`
    seen.add(key)
  }
  return null
}

/** 校验并写入 env 区段；其它字段原样保留 */
export async function saveEnv(entries: EnvEntry[]): Promise<void> {
  const invalid = validateEnv(entries)
  if (invalid) throw new Error(invalid)

  const mtime = await statSettings()
  const raw = await readRaw()
  const env: Record<string, string> = {}
  for (const e of entries) env[e.key.trim()] = e.value
  await writeSettings({ ...raw, env }, mtime)
}

export async function readPermissions(): Promise<{
  defaultMode: string
  allow: string[]
  deny: string[]
}> {
  const raw = await readRaw()
  const p = raw.permissions ?? {}
  return {
    defaultMode: p.defaultMode ?? 'default',
    allow: Array.isArray(p.allow) ? p.allow : [],
    deny: Array.isArray(p.deny) ? p.deny : []
  }
}

export async function savePermissions(next: {
  defaultMode: string
  allow: string[]
  deny: string[]
}): Promise<void> {
  const mtime = await statSettings()
  const raw = await readRaw()
  const existing = raw.permissions ?? {}
  await writeSettings(
    { ...raw, permissions: { ...existing, ...next } },
    mtime
  )
}

/** 模型配置存在 env 里，读取时只取相关键 */
export async function readModels(): Promise<Record<string, string>> {
  const env = envObject(await readRaw())
  const out: Record<string, string> = {}
  for (const k of MODEL_KEYS) out[k] = env[k] ?? ''
  return out
}

export async function saveModels(models: Record<string, string>): Promise<void> {
  const mtime = await statSettings()
  const raw = await readRaw()
  const env = { ...envObject(raw) }
  for (const k of MODEL_KEYS) {
    const v = models[k]
    if (v === undefined) continue
    if (v === '') delete env[k]
    else env[k] = v
  }
  await writeSettings({ ...raw, env }, mtime)
}

/** 用当前 env 发一次最小请求，验证中转是否可用 */
export async function testConnection(): Promise<{ ok: boolean; message: string; ms: number }> {
  const raw = await readRaw()
  const env = envObject(raw)
  const baseUrl = env['ANTHROPIC_BASE_URL']
  const token = env['ANTHROPIC_AUTH_TOKEN']
  const started = Date.now()

  if (!baseUrl) return { ok: false, message: '未配置 ANTHROPIC_BASE_URL', ms: 0 }
  if (!token) return { ok: false, message: '未配置 ANTHROPIC_AUTH_TOKEN', ms: 0 }

  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), 10000)
  try {
    const res = await fetch(`${baseUrl.replace(/\/$/, '')}/models`, {
      headers: { 'x-api-key': token, authorization: `Bearer ${token}` },
      signal: controller.signal
    })
    const ms = Date.now() - started
    if (!res.ok) {
      return { ok: false, message: `服务返回 HTTP ${res.status}`, ms }
    }
    return { ok: true, message: '连接正常', ms }
  } catch (e) {
    const ms = Date.now() - started
    const message = e instanceof Error ? e.message : String(e)
    return { ok: false, message: `连接失败：${message}`, ms }
  } finally {
    clearTimeout(timer)
  }
}
