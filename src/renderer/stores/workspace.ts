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

  /**
   * 移除工作区会级联删掉它下面的所有会话与消息（不可恢复），
   * 因此必须先向用户确认，并明确告知后果。
   */
  async function remove(id: string, sessionCount: number, deletedCount = 0): Promise<boolean> {
    const name = list.value.find((w) => w.id === id)?.name ?? '该工作区'
    const parts: string[] = []
    if (sessionCount > 0) parts.push(`${sessionCount} 个会话及其全部消息`)
    if (deletedCount > 0) parts.push(`回收站里的 ${deletedCount} 个会话`)
    const detail =
      parts.length > 0
        ? `移除「${name}」会同时永久删除它下面的${parts.join('、')}，且无法恢复。磁盘目录不会被删除。`
        : `移除「${name}」？磁盘目录不会被删除。`
    const ok = await window.api.dialog.confirm(detail)
    if (!ok) return false
    list.value = await window.api.workspace.remove(id)
    defaultId.value = await window.api.workspace.getDefault()
    return true
  }

  async function setDefault(id: string | null): Promise<void> {
    await window.api.workspace.setDefault(id)
    defaultId.value = id
  }

  async function rename(id: string, name: string): Promise<void> {
    list.value = await window.api.workspace.rename(id, name)
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
    rename,
    setDefault,
    toggle,
    isCollapsed
  }
})
