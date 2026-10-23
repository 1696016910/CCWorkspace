import { homedir } from 'node:os'
import { join, resolve, sep } from 'node:path'
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

/**
 * 规范化路径用于比较：统一分隔符、转小写（Windows 大小写不敏感）。
 * 不用 realpath —— 目录可能已不存在，且 realpath 会跟随符号链接改变语义。
 */
export function normalizePath(p: string): string {
  return resolve(p).split(sep).join('/').toLowerCase()
}

export async function pathExists(p: string): Promise<boolean> {
  try {
    await access(p)
    return true
  } catch {
    return false
  }
}
