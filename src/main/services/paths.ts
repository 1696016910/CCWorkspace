import { homedir } from 'node:os'
import { join } from 'node:path'
import { access } from 'node:fs/promises'

/** 用户主目录下的 .claude 路径 */
export function claudeDir(): string {
  return join(homedir(), '.claude')
}

export function claudeSettingsPath(): string {
  return join(claudeDir(), 'settings.json')
}

export function claudeRegistryPath(): string {
  return join(homedir(), '.claude.json')
}

export function claudeProjectsDir(): string {
  return join(claudeDir(), 'projects')
}

export function claudeBackupsDir(): string {
  return join(claudeDir(), 'backups')
}

export async function pathExists(p: string): Promise<boolean> {
  try {
    await access(p)
    return true
  } catch {
    return false
  }
}
