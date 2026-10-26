import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import type { ChatMessage, SessionSummary } from '@shared/types'

export const useSessionStore = defineStore('session', () => {
  const list = ref<SessionSummary[]>([])
  const currentId = ref<string | null>(null)
  const messages = ref<ChatMessage[]>([])
  /** 该会话的消息总数，用于判断是否还有更早的历史 */
  const messageTotal = ref(0)
  const loadingEarlier = ref(false)
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** 正在生成回复的会话，用于侧栏绿点 */
  const runningIds = ref<Set<string>>(new Set())

  /** 当前会话是否就是正在生成的那个 */
  const streamingSessionId = ref<string | null>(null)

  function setRunning(id: string, running: boolean): void {
    const next = new Set(runningIds.value)
    if (running) next.add(id)
    else next.delete(id)
    runningIds.value = next
    streamingSessionId.value = running ? id : streamingSessionId.value === id ? null : streamingSessionId.value
  }

  function isRunning(id: string): boolean {
    return runningIds.value.has(id)
  }

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

  /** 防抖令牌：快速切换会话时，只有最后一次请求的结果会写入 */
  let selectToken = 0

  /** 首屏只取最近 80 条，避免长会话一次性渲染 */
  const PAGE_SIZE = 80

  async function select(id: string): Promise<void> {
    const token = ++selectToken
    currentId.value = id
    messages.value = []
    messageTotal.value = 0
    try {
      const [loaded, total] = await Promise.all([
        window.api.session.messages(id, PAGE_SIZE),
        window.api.session.messageCount(id)
      ])
      if (token !== selectToken) return
      messages.value = loaded
      messageTotal.value = total
    } catch (e) {
      if (token !== selectToken) return
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  /** 加载更早的一页，追加到列表头部 */
  async function loadEarlier(): Promise<void> {
    const id = currentId.value
    if (!id || loadingEarlier.value) return
    if (messages.value.length >= messageTotal.value) return

    const token = ++selectToken
    loadingEarlier.value = true
    // 记录"这次请求发起时界面已有多少条"，切片必须基于它。
    // 用 await 之后的 messages.length 会错位：请求期间新消息到达、
    // 或库比界面多（压缩摘要、失败轮次），都会导致重复或漏载一条。
    const already = messages.value.length
    try {
      const all = await window.api.session.messages(id)
      // 请求期间用户可能已经切走，这时结果属于旧会话，不能往新会话里拼
      if (token !== selectToken || currentId.value !== id) return

      // 库里现在可能比发起时更多（新消息已写入）。以"当前库总数"为基准，
      // 取比已加载部分更早的那一段，避免用旧偏移量切错位置。
      const extra = Math.max(0, all.length - messageTotal.value)
      const loadedFromEnd = already + extra
      const start = Math.max(0, all.length - loadedFromEnd - PAGE_SIZE)
      const end = Math.max(0, all.length - loadedFromEnd)

      const page = all.slice(start, end)
      // 用 id 去重，防御任何残余的计数漂移
      const seen = new Set(messages.value.map((m) => m.id))
      messages.value = page.filter((m) => !seen.has(m.id)).concat(messages.value)
      messageTotal.value = all.length
    } finally {
      loadingEarlier.value = false
    }
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
    // 先作废在飞的 select/loadEarlier，避免它们的返回把已删会话的消息写回来
    selectToken++
    await window.api.session.remove(id)
    if (currentId.value === id) {
      currentId.value = null
      messages.value = []
      messageTotal.value = 0
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
    messageTotal,
    loadingEarlier,
    loading,
    error,
    runningIds,
    streamingSessionId,
    byWorkspace,
    load,
    select,
    loadEarlier,
    create,
    rename,
    remove,
    duplicate,
    sessionsOf,
    setRunning,
    isRunning
  }
})
