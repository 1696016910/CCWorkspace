import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import type { MarketplaceInfo, PluginInfo } from '../../shared/types'
import { readRaw, writeSettings } from './claudeSettings'

const run = promisify(execFile)

interface CliPlugin {
  id?: string
  version?: string
  enabled?: boolean
}

/** CLI 不可用时抛出可读错误，由上层转成界面提示 */
async function callCli(args: string[]): Promise<string> {
  try {
    const { stdout } = await run('claude', args, { timeout: 15000, maxBuffer: 8 * 1024 * 1024 })
    return stdout
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e)
    throw new Error(`调用 claude 命令失败：${message}`)
  }
}

function splitId(id: string): { name: string; marketplace: string } {
  const at = id.lastIndexOf('@')
  if (at < 0) return { name: id, marketplace: 'unknown' }
  return { name: id.slice(0, at), marketplace: id.slice(at + 1) }
}

/** 列出已安装插件；CLI 输出无法解析时退化为读配置 */
export async function listPlugins(): Promise<PluginInfo[]> {
  try {
    const stdout = await callCli(['plugin', 'list', '--json'])
    const parsed = JSON.parse(stdout) as CliPlugin[]
    return parsed.map((p) => {
      const id = p.id ?? ''
      const { name, marketplace } = splitId(id)
      return {
        id,
        name,
        version: p.version ?? '',
        marketplace,
        enabled: p.enabled !== false
      }
    })
  } catch {
    // 降级：从 settings.json 的 enabledPlugins 推导
    const raw = await readRaw()
    const map = (raw['enabledPlugins'] ?? {}) as Record<string, boolean>
    return Object.entries(map).map(([id, enabled]) => {
      const { name, marketplace } = splitId(id)
      return { id, name, version: '', marketplace, enabled }
    })
  }
}

/** 开关插件：写入 settings.json 的 enabledPlugins */
export async function togglePlugin(id: string, enabled: boolean): Promise<void> {
  const raw = await readRaw()
  const existing = (raw['enabledPlugins'] ?? {}) as Record<string, boolean>
  await writeSettings({ ...raw, enabledPlugins: { ...existing, [id]: enabled } })
}

export async function installPlugin(id: string): Promise<void> {
  await callCli(['plugin', 'install', id])
}

export async function uninstallPlugin(id: string): Promise<void> {
  await callCli(['plugin', 'uninstall', id])
}

/** 仓库：内置官方仓库不可移除 */
const BUILTIN = 'claude-plugins-official'

export async function listMarketplaces(): Promise<MarketplaceInfo[]> {
  const raw = await readRaw()
  const map = (raw['extraKnownMarketplaces'] ?? {}) as Record<string, unknown>
  const result: MarketplaceInfo[] = [{ name: BUILTIN, source: '内置', builtin: true }]
  for (const [name, value] of Object.entries(map)) {
    const src = value as { source?: { repo?: string; url?: string } } | undefined
    const repo = src?.source?.repo ?? src?.source?.url ?? 'unknown'
    result.push({ name, source: repo, builtin: false })
  }
  return result
}

export async function addMarketplace(name: string, repo: string): Promise<void> {
  const trimmed = name.trim()
  if (!trimmed) throw new Error('仓库名称不能为空')
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo.trim())) {
    throw new Error('仓库地址格式应为 owner/repo')
  }
  const raw = await readRaw()
  const map = (raw['extraKnownMarketplaces'] ?? {}) as Record<string, unknown>
  if (map[trimmed]) throw new Error('该仓库已存在')
  await writeSettings({
    ...raw,
    extraKnownMarketplaces: {
      ...map,
      [trimmed]: { source: { source: 'github', repo: repo.trim() } }
    }
  })
}

export async function removeMarketplace(name: string): Promise<void> {
  if (name === BUILTIN) throw new Error('内置仓库不可移除')
  const raw = await readRaw()
  const map = { ...((raw['extraKnownMarketplaces'] ?? {}) as Record<string, unknown>) }
  delete map[name]
  await writeSettings({ ...raw, extraKnownMarketplaces: map })
}
