import { call } from './invoke'
import type { ChatMessage, SessionSummary } from './types'

export function list(): Promise<SessionSummary[]> {
  return call('session_list')
}

export function listDeleted(): Promise<SessionSummary[]> {
  return call('session_list_deleted')
}

export function create(workspaceId: string): Promise<SessionSummary> {
  return call('session_create', { workspaceId })
}

export function rename(id: string, title: string): Promise<void> {
  return call('session_rename', { id, title })
}

/** 软删除，进回收站。 */
export function remove(id: string): Promise<void> {
  return call('session_remove', { id })
}

export function restore(id: string): Promise<void> {
  return call('session_restore', { id })
}

/** 彻底删除，不可恢复。 */
export function purge(id: string): Promise<void> {
  return call('session_purge', { id })
}

export function pin(id: string, pinned: boolean): Promise<void> {
  return call('session_pin', { id, pinned })
}

export function duplicate(id: string): Promise<SessionSummary> {
  return call('session_duplicate', { id })
}

export function messages(id: string, limit?: number): Promise<ChatMessage[]> {
  return call('session_messages', { id, limit })
}

export function messageCount(id: string): Promise<number> {
  return call('session_message_count', { id })
}
