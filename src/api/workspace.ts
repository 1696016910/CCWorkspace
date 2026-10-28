import { call } from './invoke'
import type { Workspace } from './types'
import { open } from '@tauri-apps/plugin-dialog'

export function list(): Promise<Workspace[]> {
  return call('workspace_list')
}

export function add(dir: string, name: string): Promise<Workspace> {
  return call('workspace_add', { dir, name })
}

export function remove(id: string): Promise<void> {
  return call('workspace_remove', { id })
}

export function rename(id: string, name: string): Promise<void> {
  return call('workspace_rename', { id, name })
}

export function pin(id: string, pinned: boolean): Promise<void> {
  return call('workspace_pin', { id, pinned })
}

/** 打开系统目录选择框，取消时返回 null。 */
export async function pick(): Promise<string | null> {
  const picked = await open({ directory: true, multiple: false, title: '选择项目目录' })
  if (!picked) return null
  return typeof picked === 'string' ? picked : null
}
