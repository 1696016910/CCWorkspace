import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as api from '@/api/workspace'
import type { Workspace } from '@/api/types'

export const useWorkspaceStore = defineStore('workspace', () => {
  const list = ref<Workspace[]>([])
  const loading = ref(false)
  const error = ref<string | null>(null)

  async function load(): Promise<void> {
    loading.value = true
    try {
      list.value = await api.list()
      error.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loading.value = false
    }
  }

  async function add(dir: string, name: string): Promise<void> {
    await api.add(dir, name)
    await load()
  }

  async function remove(id: string): Promise<void> {
    await api.remove(id)
    await load()
  }

  async function rename(id: string, name: string): Promise<void> {
    await api.rename(id, name)
    await load()
  }

  async function togglePin(id: string): Promise<void> {
    const w = list.value.find((x) => x.id === id)
    if (!w) return
    await api.pin(id, !w.pinnedAt)
    await load()
  }

  /** 置顶的排前面，其余保持后端顺序 */
  const ordered = computed(() => list.value)

  return { list, ordered, loading, error, load, add, remove, rename, togglePin }
})
