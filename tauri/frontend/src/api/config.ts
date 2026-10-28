import { call } from './invoke'
import type { EnvEntry, Permissions } from './types'

export function readEnv(): Promise<EnvEntry[]> {
  return call('config_read_env')
}

export function saveEnv(entries: EnvEntry[]): Promise<void> {
  return call('config_save_env', { entries })
}

export function readPermissions(): Promise<Permissions> {
  return call('config_read_permissions')
}

export function savePermissions(p: Permissions): Promise<void> {
  return call('config_save_permissions', { p })
}

export function readModels(): Promise<Record<string, string>> {
  return call('config_read_models')
}

/** 后端 save_models 收的是 (key, value) 元组数组 */
export function saveModels(m: Record<string, string>): Promise<void> {
  return call('config_save_models', { m: Object.entries(m) })
}

/** settings.json 解析失败时的提示，正常时为 null。 */
export function health(): Promise<string | null> {
  return call('config_health')
}
