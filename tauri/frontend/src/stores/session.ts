import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as api from '@/api/session'
import type { ChatMessage, SessionSummary } from '@/api/types'

export const useSessionStore = defineStore('session', () => {
  const list = ref<SessionSummary[]>([])
  const deleted = ref<SessionSummary[]>([])
  const messages = ref<ChatMessage[]>([])
  const activeId = ref<string | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)

  const current = computed(() => list.value.find((s) => s.id === activeId.value) ?? null)

  async function load(): Promise<void> {
    loading.value = true
    try {
      list.value = await api.list()
      // 选中的会话没了就清空，避免界面停在幽灵状态
      if (activeId.value && !list.value.some((s) => s.id === activeId.value)) {
        activeId.value = null
        messages.value = []
      }
      error.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loading.value = false
    }
  }

  async function loadDeleted(): Promise<void> {
    try {
      deleted.value = await api.listDeleted()
    } catch {
      deleted.value = []
    }
  }

  async function select(id: string | null): Promise<void> {
    activeId.value = id
    if (!id) {
      messages.value = []
      return
    }
    try {
      messages.value = await api.messages(id)
      error.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      messages.value = []
    }
  }

  async function create(workspaceId: string): Promise<void> {
    const s = await api.create(workspaceId)
    await load()
    await select(s.id)
  }

  async function rename(id: string, title: string): Promise<void> {
    await api.rename(id, title)
    await load()
  }

  async function remove(id: string): Promise<void> {
    await api.remove(id)
    if (activeId.value === id) {
      activeId.value = null
      messages.value = []
    }
    await load()
  }

  async function restore(id: string): Promise<void> {
    await api.restore(id)
    await load()
    await loadDeleted()
  }

  async function purge(id: string): Promise<void> {
    await api.purge(id)
    await loadDeleted()
  }

  async function togglePin(id: string): Promise<void> {
    const s = list.value.find((x) => x.id === id)
    if (!s) return
    await api.pin(id, !s.pinnedAt)
    await load()
  }

  async function duplicate(id: string): Promise<void> {
    await api.duplicate(id)
    await load()
  }

  /** 按工作区分组，置顶在前。 */
  function byWorkspace(workspaceId: string): SessionSummary[] {
    return list.value.filter((s) => s.workspaceId === workspaceId)
  }

  return {
    list,
    deleted,
    messages,
    activeId,
    loading,
    error,
    current,
    load,
    loadDeleted,
    select,
    create,
    rename,
    remove,
    restore,
    purge,
    togglePin,
    duplicate,
    byWorkspace
  }
})
