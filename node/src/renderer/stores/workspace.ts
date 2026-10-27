import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { Workspace } from '@shared/types'

export const useWorkspaceStore = defineStore('workspace', () => {
  const list = ref<Workspace[]>([])
  const collapsed = ref<Set<string>>(new Set())
  const loading = ref(false)
  const error = ref<string | null>(null)

  const current = computed(() => list.value[0] ?? null)

  const titlePath = computed(() => {
    const w = current.value
    if (!w) return '未添加工作区'
    return `${w.name} · ${w.path}`
  })

  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      list.value = await window.api.workspace.list()
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      loading.value = false
    }
  }

  async function add(dir: string, name?: string): Promise<void> {
    try {
      list.value = await window.api.workspace.add(dir, name)
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

  /**
   * 移除工作区。确认流程由调用方用 ConfirmDialog 完成，
   * 这里只负责执行，避免弹窗样式两套并存。
   */
  async function removeConfirmed(id: string): Promise<void> {
    list.value = await window.api.workspace.remove(id)
  }

  async function rename(id: string, name: string): Promise<void> {
    list.value = await window.api.workspace.rename(id, name)
  }

  async function pin(id: string, pinned: boolean): Promise<void> {
    list.value = await window.api.workspace.pin(id, pinned)
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
    current,
    titlePath,
    loading,
    error,
    load,
    add,
    pickAndAdd,
    removeConfirmed,
    rename,
    pin,
    toggle,
    isCollapsed
  }
})
