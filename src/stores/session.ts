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

  /**
   * 草稿会话：点「新建会话」时只在前端占位，不落库。
   *
   * 用户要的是「点新建 → 一个空白对话框 → 输入第一条消息才算一次对话」。
   * 立刻建库会让左侧列表堆满空条目，库里也会留下永远没消息的会话。
   */
  const draft = ref<{ workspaceId: string } | null>(null)

  /** 草稿会话的占位 id。带前缀便于各处识别，不会与真实 uuid 冲突。 */
  const DRAFT_ID = '__draft__'

  const current = computed(() => {
    if (draft.value) {
      // 虚拟会话：字段按新建会话应有的初值给，id 用固定前缀标记
      return {
        id: DRAFT_ID,
        workspaceId: draft.value.workspaceId,
        title: '',
        updatedAt: new Date().toISOString(),
        messageCount: 0,
        running: false,
        renamed: false,
        contextPercent: 0,
        contextTokens: 0,
        contextMax: 0,
        pinnedAt: null
      } as SessionSummary
    }
    return list.value.find((s) => s.id === activeId.value) ?? null
  })

  const isDraft = computed(() => draft.value !== null)

  /** 开一个草稿会话：不落库，只切到空白对话界面 */
  function startDraft(workspaceId: string): void {
    draft.value = { workspaceId }
    activeId.value = null
    messages.value = []
  }

  /**
   * 把草稿转成真实会话。发送第一条消息前调用。
   *
   * 返回真实会话 id，后续发送与落库都用它。
   */
  async function commitDraft(): Promise<string> {
    const d = draft.value
    if (!d) return activeId.value ?? ''
    const s = await api.create(d.workspaceId)
    draft.value = null
    await load()
    activeId.value = s.id
    return s.id
  }

  /** 放弃草稿（例如切到别的会话） */
  function discardDraft(): void {
    draft.value = null
  }

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
    // 切到真实会话时丢掉草稿，否则 current 会一直返回虚拟对象
    draft.value = null
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
    startDraft(workspaceId)
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
    isDraft,
    startDraft,
    commitDraft,
    discardDraft,
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
