import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { Workspace } from '@shared/types'

export const useWorkspaceStore = defineStore('workspace', () => {
  const list = ref<Workspace[]>([])
  const defaultId = ref<string | null>(null)
  const collapsed = ref<Set<string>>(new Set())
  const loading = ref(false)
  const error = ref<string | null>(null)

  const current = computed(() => {
    const def = defaultId.value
    if (def) {
      const found = list.value.find((w) => w.id === def)
      if (found) return found
    }
    return list.value[0] ?? null
  })

  const titlePath = computed(() => {
    const w = current.value
    if (!w) return '未添加工作区'
    return `${w.name} · ${w.path}`
  })

  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      const [workspaces, def] = await Promise.all([
        window.api.workspace.list(),
        window.api.workspace.getDefault()
      ])
      list.value = workspaces
      defaultId.value = def
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loading.value = false
    }
  }

  async function add(dir: string): Promise<void> {
    try {
      list.value = await window.api.workspace.add(dir)
      error.value = null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      throw e
    }
  }

  async function pickAndAdd(): Promise<void> {
    const dir = await window.api.workspace.pick()
    if (dir) await add(dir)
  }

  async function remove(id: string): Promise<void> {
    list.value = await window.api.workspace.remove(id)
    defaultId.value = await window.api.workspace.getDefault()
  }

  async function setDefault(id: string | null): Promise<void> {
    await window.api.workspace.setDefault(id)
    defaultId.value = id
  }

  function toggle(id: string): void {
    const next = new Set(collapsed.value)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    collapsed.value = next
  }

  function isCollapsed(id: string): boolean {
    return collapsed.value.has(id)
  }

  return {
    list,
    defaultId,
    current,
    titlePath,
    loading,
    error,
    load,
    add,
    pickAndAdd,
    remove,
    setDefault,
    toggle,
    isCollapsed
  }
})
