import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { ChatMessage, SessionSummary } from '@shared/types'

export const useSessionStore = defineStore('session', () => {
  const list = ref<SessionSummary[]>([])
  const currentId = ref<string | null>(null)
  const messages = ref<ChatMessage[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  const current = computed(() => list.value.find((s) => s.id === currentId.value) ?? null)

  const byWorkspace = computed(() => {
    const map = new Map<string, SessionSummary[]>()
    for (const s of list.value) {
      const bucket = map.get(s.workspaceId) ?? []
      bucket.push(s)
      map.set(s.workspaceId, bucket)
    }
    return map
  })

  function sessionsOf(workspaceId: string): SessionSummary[] {
    return byWorkspace.value.get(workspaceId) ?? []
  }

  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      list.value = await window.api.session.list()
      if (currentId.value && !list.value.some((s) => s.id === currentId.value)) {
        currentId.value = null
        messages.value = []
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loading.value = false
    }
  }

  async function select(id: string): Promise<void> {
    currentId.value = id
    messages.value = await window.api.session.messages(id)
  }

  async function create(workspaceId: string | null): Promise<SessionSummary | null> {
    try {
      const created = await window.api.session.create(workspaceId)
      await load()
      await select(created.id)
      return created
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    }
  }

  async function rename(id: string, title: string): Promise<void> {
    await window.api.session.rename(id, title)
    await load()
  }

  async function remove(id: string): Promise<void> {
    await window.api.session.remove(id)
    if (currentId.value === id) {
      currentId.value = null
      messages.value = []
    }
    await load()
  }

  async function duplicate(id: string): Promise<void> {
    const copy = await window.api.session.duplicate(id)
    await load()
    await select(copy.id)
  }

  return {
    list,
    currentId,
    current,
    messages,
    loading,
    error,
    byWorkspace,
    load,
    select,
    create,
    rename,
    remove,
    duplicate,
    sessionsOf
  }
})
