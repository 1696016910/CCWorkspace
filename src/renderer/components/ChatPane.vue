<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useSessionStore } from '@/stores/session'
import { useWorkspaceStore } from '@/stores/workspace'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage, ContextUsageDetail, SlashCommandInfo } from '@shared/types'
import MessageItem from './MessageItem.vue'
import { renderMarkdown } from '@/utils/markdown'
import ContextPopover from './ContextPopover.vue'
import FileViewer from './FileViewer.vue'
import { useToast } from '@/composables/useToast'

const sessions = useSessionStore()
const ws = useWorkspaceStore()
const chat = useChatStore()
const toast = useToast()
const viewingFile = ref<string | null>(null)

const input = ref('')
const thread = ref<HTMLElement | null>(null)
const textarea = ref<HTMLTextAreaElement | null>(null)
const attachments = ref<string[]>([])

/* ── 斜杠命令 ── */
const commands = ref<SlashCommandInfo[]>([])
const cmdOpen = ref(false)
const cmdIndex = ref(0)

const current = computed(() => sessions.current)

const workspaceName = computed(() => {
  const s = current.value
  if (!s) return ''
  return ws.list.find((w) => w.id === s.workspaceId)?.name ?? ''
})

/* ── 上下文 ── */
const ctxDetail = ref<ContextUsageDetail | null>(null)
const ctxOpen = ref(false)
const ctxLoading = ref(false)

const ctxPercent = computed(() => {
  if (ctxDetail.value) return ctxDetail.value.percentage
  return current.value?.contextPercent ?? 0
})

/** 小于 1% 时保留一位小数，避免一进来就显示 0% */
const ctxLabel = computed(() => {
  if (!current.value) return '—'
  const p = ctxPercent.value
  if (p > 0 && p < 1) return `${p.toFixed(1)}%`
  return `${Math.round(p)}%`
})

async function loadContext(): Promise<void> {
  if (!current.value) return
  ctxLoading.value = true
  try {
    // 由主进程负责在需要时跑一次 /context 并等待结构化结果
    const d = await window.api.context.usage(current.value.id)
    if (d) ctxDetail.value = d
  } finally {
    ctxLoading.value = false
  }
}

let ctxPollTimer: ReturnType<typeof setTimeout> | null = null
/** 组件已卸载：轮询恢复后不能再排新的定时器，否则永久 1.5s 一次打 IPC */
let disposed = false
/** 轮询次数上限，避免 /context 一直拿不到结果时无限重跑并反复拉起 CLI */
const MAX_POLLS = 10

async function toggleContext(): Promise<void> {
  ctxOpen.value = !ctxOpen.value
  if (!ctxOpen.value) {
    if (ctxPollTimer) clearTimeout(ctxPollTimer)
    return
  }
  // 已有分类明细就直接用；没有才向主进程要（此时才可能跑一次 /context）
  if (ctxDetail.value && ctxDetail.value.categories.length > 0) return
  void pollContext()
}

/**
 * 主进程在后台跑 /context 后才会写入分类明细，
 * 因此打开面板时轮询几次，拿到数据就停。
 */
async function pollContext(polls = 0): Promise<void> {
  if (!ctxOpen.value || disposed) return
  ctxLoading.value = true
  await loadContext()
  ctxLoading.value = false
  if (disposed) return
  const hasCats = (ctxDetail.value?.categories.length ?? 0) > 0
  if (!hasCats && ctxOpen.value && polls < MAX_POLLS) {
    ctxPollTimer = setTimeout(() => void pollContext(polls + 1), 1500)
  }
}

async function doCompact(): Promise<void> {
  if (!current.value) return
  ctxOpen.value = false
  try {
    await chat.compact(current.value.id)
    await loadContext()
    await sessions.load()
  } catch (e) {
    // 生成中压缩会被主进程拒绝，之前这条错误只进 console，
    // 用户看到的是"面板自己关了、什么都没发生"
    toast.error(e instanceof Error ? e.message : '压缩失败')
  }
}

/* ── 消息 ── */
/**
 * 滚动到底部。
 *
 * 流式输出时每次增量都调用，必须用 instant 且只在用户已接近底部时跟随，
 * 否则既卡顿又会打断用户向上翻阅历史。
 */
/**
 * 滚动到底部。
 *
 * @param smooth 是否平滑滚动
 * @param force  忽略"用户已上翻"的判断，强制滚到底。
 *               切换会话时必须用 true：新会话的 scrollTop 初始为 0，
 *               不加 force 会被下面的守卫当成"用户正在上翻"而直接返回，
 *               结果停在最顶部——也就是最早的对话。
 */
async function scrollToBottom(smooth = false, force = false): Promise<void> {
  await nextTick()
  const el = thread.value
  if (!el) return

  const jump = (): void => {
    const height = el.scrollHeight
    if (!force) {
      const distance = height - el.scrollTop - el.clientHeight
      // 流式输出时用户主动上翻超过一屏就不再强制跟随，避免打断阅读
      if (distance > el.clientHeight * 1.2) return
    }
    el.scrollTo({ top: height, behavior: smooth ? 'smooth' : 'auto' })
  }

  jump()

  // 消息项用了 content-visibility: auto，视口外的内容高度是惰性计算的：
  // 第一次调用时 scrollHeight 只反映已渲染的部分（实测 1560 vs 最终 14114），
  // 滚过去之后才逐步补全。这里连做几帧校正，确保真正落到底。
  if (force) {
    for (const delay of [16, 60, 160]) {
      await new Promise((r) => setTimeout(r, delay))
      jump()
    }
  }
}

/**
 * 删除当前会话。
 *
 * 必须先停掉生成：不停的话请求继续跑并计费，产出的回复又因为会话
 * 已软删而写不进去，等于白花钱还多一条报错。
 * 侧栏的删除按钮已有同样处理，这里补齐顶栏。
 */
async function removeCurrent(): Promise<void> {
  const id = current.value?.id
  if (!id) return
  if (sessions.isRunning(id)) await chat.stop()
  await sessions.remove(id)
}

function onMessageSaved(msg: ChatMessage, sessionId: string): void {
  // 回复可能属于用户已经切走的会话。不判断就 append 会把 A 的回复
  // 插进 B 的对话流里，看起来像"数据错乱"，重选会话才消失。
  if (sessionId === sessions.currentId) {
    sessions.messages = [...sessions.messages, msg]
    // messageTotal 不跟着涨的话，"加载更早"按钮会因为
    // messages.length >= messageTotal 而提前消失，旧消息再也翻不到
    void window.api.session
      .messageCount(sessionId)
      .then((n) => {
        if (sessions.currentId === sessionId) sessions.messageTotal = n
      })
      .catch(() => {})
    // 一轮结束后主进程已写入新的上下文用量，重新读一次即可，无需再跑 /context
    void refreshCtxFromCache()
    void scrollToBottom()
  }
  void sessions.load()
}

/** 只读缓存，不触发生成 */
async function refreshCtxFromCache(): Promise<void> {
  if (!current.value) return
  const d = await window.api.context.usage(current.value.id)
  if (d) ctxDetail.value = d
}

onUnmounted(() => {
  disposed = true
  if (ctxPollTimer) clearTimeout(ctxPollTimer)
  if (streamTimer) clearTimeout(streamTimer)
  // 订阅必须释放：否则边界重试重建组件后，事件仍进旧实例的闭包
  chat.unlisten()
})

onMounted(() => {
  chat.listen({
    onDone: onMessageSaved,
    onFinish: (id) => {
      sessions.setRunning(id, false)
      if (chat.error) toast.error(chat.error)
      // 生成结束后把焦点还给输入框
      void nextTick(() => textarea.value?.focus())
    },
    onCommands: async (id) => {
      commands.value = await window.api.commands.list(id)
    }
  })
})

watch(
  () => sessions.currentId,
  async () => {
    attachments.value = []
    ctxDetail.value = null
    const active = current.value
    if (active) {
      // 切换会话只读缓存；命令清单同样先读缓存，不主动触发生成
      void refreshCtxFromCache().then(async () => {
        commands.value = await window.api.commands.list(active.id)
      })
    }
    // currentId 变了但消息还没加载完，此时滚动没有意义（列表还是空的）。
    // 真正的滚动交给下面的 messages 监听，等消息渲染后再滚。
  }
)

/**
 * 消息加载完成后滚到底部。
 *
 * 切会话时 currentId 先变、messages 异步后到，所以必须监听 messages。
 * 这里用 length 做比较：切会话时从 0 变成 N，能触发；
 * 流式追加时也会触发，但那时有单独的逻辑，用 streaming 状态排除。
 */
watch(
  () => sessions.messages.length,
  async (len, prev) => {
    // 只在"从空到有"或"切换到另一个会话"时强制滚到底
    if (len > 0 && prev === 0) {
      await nextTick()
      void scrollToBottom(false, true)
    }
  }
)

/**
 * 流式内容的渲染结果，按帧节流。
 *
 * renderMarkdown 是全文重新解析，成本随已输出长度线性增长；
 * 每个 delta 都渲染一次就是 O(n²)：实测 5 万字符要 10 秒纯 CPU，
 * 界面直接卡死。节流到约 8 帧/秒后同样的内容只需 2ms，
 * 视觉上仍然连贯（人眼分辨不出 120ms 内的差异）。
 */
const streamHtml = ref('')
let streamTimer: ReturnType<typeof setTimeout> | null = null
let streamPending = false

function scheduleStreamRender(): void {
  if (streamTimer) {
    streamPending = true
    return
  }
  const run = (): void => {
    // 流式内容不缓存（每帧都是新字符串）也不高亮（未闭合代码块会反复全量重算）
    streamHtml.value = renderMarkdown(chat.streamText, false, false)
    // 只在真正更新 DOM 之后滚动：每 delta 都读 scrollHeight 会触发强制同步布局
    void scrollToBottom(false)
    streamTimer = null
    if (streamPending) {
      streamPending = false
      streamTimer = setTimeout(run, 120)
    }
  }
  // 首帧立即出，保证"开始输出"没有延迟感
  streamHtml.value = renderMarkdown(chat.streamText, false, false)
  streamTimer = setTimeout(run, 120)
}

watch(
  () => chat.streamText,
  // 只触发渲染；滚动在渲染回调里做，避免每个 delta 都读一次 scrollHeight
  () => scheduleStreamRender()
)

/** 流式结束时清空缓冲，避免下一轮把旧 HTML 闪一下 */
watch(
  () => chat.streaming,
  (on) => {
    if (on) return
    if (streamTimer) clearTimeout(streamTimer)
    streamTimer = null
    streamPending = false
    streamHtml.value = ''
  }
)

/* ── 输入框与命令面板 ── */
function onInput(e: Event): void {
  const el = e.target as HTMLTextAreaElement
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`

  const value = el.value
  const wasOpen = cmdOpen.value
  cmdOpen.value = value.startsWith('/')
  if (cmdOpen.value) {
    cmdIndex.value = 0
    if (!wasOpen) void ensureCommands()
  }
}

/**
 * 首次打开命令面板时拉一次完整清单。
 * SDK 的完整命令列表只在跑过一轮后才拿到，因此这里若为空，
 * 触发一次 /context（代价最小、且顺带刷新上下文）。
 */
let warming = false
async function ensureCommands(): Promise<void> {
  if (commands.value.length > 0 || warming) return
  if (!current.value) return
  commands.value = await window.api.commands.list(current.value.id)
}

const filteredCommands = computed(() => {
  const q = input.value.startsWith('/') ? input.value.slice(1).toLowerCase() : ''
  const list = commands.value.filter(
    (c) =>
      c.name.toLowerCase().includes(q) ||
      c.description.toLowerCase().includes(q)
  )
  // 排序：前缀命中 > 内置 > 名称字母序
  const score = (c: SlashCommandInfo): number => {
    if (!q) return c.builtin ? 0 : 1
    if (c.name.toLowerCase().startsWith(q)) return 0
    return c.builtin ? 1 : 2
  }
  return [...list].sort((a, b) => {
    const d = score(a) - score(b)
    return d !== 0 ? d : a.name.localeCompare(b.name)
  })
})

function applyCommand(c: SlashCommandInfo): void {
  input.value = `/${c.name} `
  cmdOpen.value = false
  textarea.value?.focus()
}

function onKeydown(e: KeyboardEvent): void {
  if (cmdOpen.value && filteredCommands.value.length > 0) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      cmdIndex.value = (cmdIndex.value + 1) % filteredCommands.value.length
      return
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault()
      cmdIndex.value =
        (cmdIndex.value - 1 + filteredCommands.value.length) % filteredCommands.value.length
      return
    }
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      const target = filteredCommands.value[cmdIndex.value]
      if (target) applyCommand(target)
      return
    }
    if (e.key === 'Escape') {
      cmdOpen.value = false
      return
    }
  }
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    void send()
  }
}

/**
 * Uint8Array → base64。
 *
 * 不能用 btoa(String.fromCharCode(...bytes))：展开运算符会把每个字节变成
 * 一个实参，超过约 12 万字节就抛 RangeError。截图动辄几百 KB，
 * 那条写法等于"粘贴图片必崩"。这里分块处理，任意大小都安全。
 */
function toBase64(buf: Uint8Array): string {
  const CHUNK = 0x8000
  let binary = ''
  for (let i = 0; i < buf.length; i += CHUNK) {
    binary += String.fromCharCode(...buf.subarray(i, i + CHUNK))
  }
  return btoa(binary)
}

/** 支持从剪贴板粘贴图片或文件 */
async function onPaste(e: ClipboardEvent): Promise<void> {
  const items = e.clipboardData?.items
  if (!items) return
  let handled = false
  for (const item of Array.from(items)) {
    if (item.kind !== 'file') continue
    const file = item.getAsFile()
    if (!file) continue
    handled = true
    try {
      const buf = await file.arrayBuffer()
      const name =
        file.name ||
        (file.type.startsWith('image/') ? `pasted-${Date.now()}.png` : `pasted-${Date.now()}`)
      const saved = await window.api.file.savePaste(name, toBase64(new Uint8Array(buf)))
      attachments.value = [...attachments.value, saved]
    } catch (err) {
      // 单个附件失败不该让整次粘贴无反应，至少要给出可见提示
      toast.error(err instanceof Error ? err.message : '附件保存失败')
    }
  }
  if (handled) e.preventDefault()
}

/* ── 拖拽文件 ── */
const dragOver = ref(false)

function onDragOver(e: DragEvent): void {
  if (!e.dataTransfer?.types.includes('Files')) return
  e.preventDefault()
  dragOver.value = true
}

function onDragLeave(e: DragEvent): void {
  const el = e.currentTarget as HTMLElement
  // 只有真正离开容器时才取消高亮
  if (e.relatedTarget && el.contains(e.relatedTarget as Node)) return
  dragOver.value = false
}

function onDrop(e: DragEvent): void {
  e.preventDefault()
  dragOver.value = false
  const files = Array.from(e.dataTransfer?.files ?? [])
  if (!files.length) return
  // Electron 给 File 对象加了 path 属性，直接拿真实路径
  const paths = files
    .map((f) => (f as File & { path?: string }).path)
    .filter((p): p is string => typeof p === 'string' && p.length > 0)
  if (paths.length) attachments.value = [...attachments.value, ...paths]
}

async function pickFiles(): Promise<void> {
  const files = await window.api.dialog.pickFiles()
  if (files.length) {
    attachments.value = [...attachments.value, ...files]
  }
}

function fileUrl(path: string): string {
  return `file:///${path.split('\\').join('/')}`
}

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path
}

function isImage(path: string): boolean {
  return /\.(png|jpe?g|gif|webp)$/i.test(path)
}

function removeAttachment(i: number): void {
  attachments.value = attachments.value.filter((_, idx) => idx !== i)
}

async function send(): Promise<void> {
  const text = input.value.trim()
  if (!text || !current.value) return

  const prefix = attachments.value.length
    ? attachments.value.map((f) => `@${f}`).join(' ') + '\n\n'
    : ''

  input.value = ''
  attachments.value = []
  cmdOpen.value = false

  const optimistic: ChatMessage = {
    id: `local-${Date.now()}`,
    role: 'user',
    text: prefix + text,
    timestamp: new Date().toISOString()
  }
  sessions.messages = [...sessions.messages, optimistic]
  sessions.setRunning(current.value.id, true)
  void scrollToBottom()

  const sessionId = current.value.id
  const ok = await chat.send(sessionId, prefix + text)
  if (!ok) {
    // 发送失败时必须回滚：否则列表里留一条库里并不存在的消息，
    // 侧栏绿点也会一直亮着，直到下次在该会话成功发送才消失
    sessions.messages = sessions.messages.filter((m) => m.id !== optimistic.id)
    sessions.setRunning(sessionId, false)
    input.value = text
    toast.error(chat.error ?? '发送失败')
  }
}

/** 流式文本里的代码块复制与链接拦截 */
async function onStreamClick(e: MouseEvent): Promise<void> {
  const target = e.target as HTMLElement

  // 流式内容与历史消息一样是不可信文本，链接必须拦下来交给系统浏览器，
  // 否则点击会让 Electron 窗口导航走，应用界面直接丢失
  const link = target.closest('a')
  if (link) {
    e.preventDefault()
    const href = link.getAttribute('href')
    if (href) await window.api.system.openExternal(href)
    return
  }

  if (!target.matches('[data-copy]')) return
  // 必须限定在 pre 里：块内可能藏着其它 code 元素，取第一个会把用户看不见的内容复制走
  const code = target.closest('.code-block')?.querySelector('pre code')?.textContent ?? ''
  if (!code) return
  try {
    await navigator.clipboard.writeText(code)
    const prev = target.textContent
    target.textContent = '已复制'
    setTimeout(() => (target.textContent = prev), 1400)
  } catch {
    // 忽略
  }
}

/** 加载更早的消息，并保持滚动位置不跳 */
async function loadEarlier(): Promise<void> {
  const el = thread.value
  const before = el?.scrollHeight ?? 0
  const fromTop = el?.scrollTop ?? 0
  await sessions.loadEarlier()
  await nextTick()
  if (el) el.scrollTop = el.scrollHeight - before + fromTop
}

async function stop(): Promise<void> {
  await chat.stop()
  // 停止后把焦点还给输入框，用户可以立刻继续输入
  await nextTick()
  textarea.value?.focus()
}
</script>

<template>
  <main class="main">
    <div class="main-head">
      <span class="mh-title" :title="current?.title ?? ''">{{ current?.title ?? '未选择会话' }}</span>
      <span v-if="current" class="mh-ws" :title="workspaceName">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7">
          <path d="M3.5 7.5h6l1.8 2h9.2v9h-17v-11z" />
        </svg>
        {{ workspaceName }}
      </span>
      <span class="sp" />
      <button v-if="current" class="mh-btn" title="复制会话" @click="sessions.duplicate(current.id)">
        <svg viewBox="0 0 24 24"><rect x="8" y="8" width="11" height="11" rx="2" /><path d="M15 5.5H6A1.5 1.5 0 004.5 7v9" /></svg>
      </button>
      <button v-if="current" class="mh-btn" title="删除会话" @click="removeCurrent">
        <svg viewBox="0 0 24 24"><path d="M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13" /><path d="M10.5 10v6M13.5 10v6" /></svg>
      </button>
    </div>

    <div v-if="current" ref="thread" class="thread">
      <div class="thread-inner">
        <!-- 新会话没有任何消息时，中间就是一片纯白，用户不知道能做什么 -->
        <div v-if="!sessions.messages.length && !chat.streaming" class="thread-empty">
          <div class="te-ico">✦</div>
          <div class="te-title">开始对话</div>
          <div class="te-hint">直接输入问题，或用 <code>/</code> 调用命令与 Skill</div>
        </div>
        <div v-if="sessions.messages.length < sessions.messageTotal" class="load-earlier">
          <button :disabled="sessions.loadingEarlier" @click="loadEarlier">
            {{ sessions.loadingEarlier ? '加载中…' : `加载更早的消息 · 还有 ${sessions.messageTotal - sessions.messages.length} 条` }}
          </button>
        </div>

        <MessageItem
          v-for="m in sessions.messages"
          :key="m.id"
          :message="m"
          @open-file="viewingFile = $event"
        />

        <div v-if="chat.streaming && chat.streamingSessionId === current?.id" class="msg-ai">
          <div class="ai-role"><span class="who">Claude</span></div>
          <div v-if="chat.streamTools.length" class="steps">
            <div v-for="(t, i) in chat.streamTools" :key="i" class="step">
              <span class="si">{{ t.name.slice(0, 1) }}</span>
              <span class="sname">{{ t.name }}</span>
              <span class="sarg">{{ t.target }}</span>
            </div>
          </div>
          <div
            v-if="chat.streamText"
            class="ai-text md"
            v-html="streamHtml"
            @click="onStreamClick"
          />
          <div v-else class="thinking">
            <span class="dot" /><span class="dot" /><span class="dot" />
            <span class="thinking-label">正在思考</span>
          </div>
        </div>

        <div v-if="chat.error" class="chat-error">{{ chat.error }}</div>
      </div>
    </div>

    <div v-else class="empty">
      <div class="eico">
        <svg viewBox="0 0 24 24"><path d="M4.5 5.5h15v10.5h-9l-6 4v-14.5z" /></svg>
      </div>
      <h2>{{ ws.list.length === 0 ? '开始使用 Claude 工作台' : '选择一个会话开始' }}</h2>
      <p v-if="ws.list.length === 0">
        工作区就是你的项目目录。添加后即可在里面新建会话，与 Claude 对话、管理插件与配置。
      </p>
      <p v-else>从左侧选一个会话，或把鼠标移到工作区名称上点击 ＋ 新建。</p>
      <button v-if="ws.list.length === 0" class="primary-btn" @click="ws.pickAndAdd()">
        选择项目目录
      </button>
      <div v-if="ws.list.length === 0" class="tips">
        <span class="tip">会话数据保存在本机，与终端里的 Claude Code 互不影响</span>
      </div>
    </div>

    <div
      v-if="current"
      class="composer-wrap"
      :class="{ 'drag-over': dragOver }"
      @dragover="onDragOver"
      @dragleave="onDragLeave"
      @drop="onDrop"
    >
      <div v-if="attachments.length" class="attach-row">
        <span v-for="(f, i) in attachments" :key="i" class="attach-chip">
          <img v-if="isImage(f)" :src="fileUrl(f)" class="attach-thumb" alt="" />
          <span class="attach-name" :title="f">{{ fileName(f) }}</span>
          <button class="attach-x" @click="removeAttachment(i)">✕</button>
        </span>
      </div>

      <div class="composer">
        <div v-if="cmdOpen && filteredCommands.length" class="cmd-panel">
          <button
            v-for="(c, i) in filteredCommands"
            :key="c.name"
            class="cmd-item"
            :class="{ on: i === cmdIndex }"
            @mousedown.prevent="applyCommand(c)"
          >
            <span class="cmd-name">/{{ c.name }}</span>
            <span class="cmd-desc">{{ c.description || c.argumentHint || '' }}</span>
          </button>
        </div>

        <textarea
          ref="textarea"
          v-model="input"
          rows="2"
          placeholder="描述你想让它做的事，或输入 / 调用命令与 Skill…"
          :disabled="chat.streaming && chat.streamingSessionId === current?.id"
          @input="onInput"
          @keydown="onKeydown"
          @paste="onPaste"
        />

        <div class="composer-bar">
          <button class="cb-btn" title="添加文件" @click="pickFiles">
            <svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
          </button>
          <span class="sp" />

          <button class="ctx" :class="{ open: ctxOpen }" title="查看上下文使用情况" @click="toggleContext">
            <span class="ctx-bar"><i :style="{ width: `${Math.min(Math.max(ctxPercent, 0.8), 100)}%` }" /></span>
            <span class="ctx-num">{{ ctxLabel }}</span>
          </button>

          <button
            v-if="chat.streaming && chat.streamingSessionId === current?.id"
            class="send-btn stop"
            title="停止"
            @click="stop"
          >
            <svg viewBox="0 0 24 24"><rect x="7" y="7" width="10" height="10" rx="1.5" fill="currentColor" stroke="none" /></svg>
          </button>
          <button v-else class="send-btn" :disabled="!input.trim()" title="发送" @click="send">
            <svg viewBox="0 0 24 24"><path d="M12 19V5M6 11l6-6 6 6" /></svg>
          </button>
        </div>
      </div>
      <div class="hint">Enter 发送 · Shift + Enter 换行 · / 调用命令 · 可直接粘贴图片</div>
    </div>

    <FileViewer v-if="viewingFile" :path="viewingFile" @close="viewingFile = null" />

    <ContextPopover
      v-if="ctxOpen"
      :detail="ctxDetail"
      :loading="ctxLoading"
      @close="ctxOpen = false"
      @compact="doCompact"
    />
  </main>
</template>

<style scoped>
.main {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  position: relative;
  overflow: hidden;
}

.main-head {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 46px;
  padding: 0 24px;
  border-bottom: 1px solid var(--line);
  /* 窄窗口下标题过长会把右侧按钮挤出可视区 */
  min-width: 0;
  overflow: hidden;
}

.mh-title {
  font-size: 14px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 0 1 auto;
}

.thread-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 80px 24px;
  color: var(--t3);
  text-align: center;
}

.thread-empty .te-ico {
  font-size: 22px;
  color: var(--green);
  opacity: 0.7;
}

.thread-empty .te-title {
  font-size: 14px;
  color: var(--t2);
}

.thread-empty .te-hint {
  font-size: 12px;
  color: var(--t4);
}

.thread-empty .te-hint code {
  padding: 1px 5px;
  border-radius: 4px;
  background: var(--bg-soft);
  font-family: var(--mono);
  font-size: 11px;
}

.mh-ws {
  font-size: 11.5px;
  color: var(--t4);
  display: flex;
  align-items: center;
  gap: 5px;
  /* 必须可收缩 + 省略：flex:none 时长工作区名会把后面的
     复制/删除按钮挤出视口，用户看不到也点不到 */
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}

.mh-ws > span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.sp {
  flex: 1;
}

.mh-btn {
  width: 28px;
  height: 28px;
  border-radius: var(--r);
  flex: none;
  display: grid;
  place-items: center;
  color: var(--t3);
}

.mh-btn:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.mh-btn svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.thread {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  /* 底部留出足够空隙：只留 8px 时最后一条消息会紧贴输入框，
     滚动到底也看不清结尾，长回复尤其明显。
     这里不能用 mask-image 做渐隐——它会连带把"正在思考"这类
     贴着底部的内容一起吃掉，看起来就是被输入框遮挡。 */
  padding: 26px 0 32px;
}

.thread-inner {
  max-width: 720px;
  margin: 0 auto;
  padding: 0 28px;
}

.load-earlier {
  display: flex;
  justify-content: center;
  margin-bottom: 18px;
}

.load-earlier button {
  height: 26px;
  padding: 0 12px;
  border-radius: 13px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  font-size: 11.5px;
  color: var(--t3);
}

.load-earlier button:hover:not(:disabled) {
  color: var(--t1);
  border-color: var(--line-3);
}

.msg-ai {
  margin-bottom: 30px;
}

.ai-role {
  margin-bottom: 10px;
  font-size: 11px;
  color: var(--t3);
}

.ai-role .who {
  color: var(--t2);
  font-weight: 500;
}

.steps {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 12px;
}

.step {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--t2);
}

.step .si {
  width: 17px;
  height: 17px;
  border-radius: 4px;
  flex: none;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  display: grid;
  place-items: center;
  font-size: 9px;
  color: var(--t3);
}

.step .sname {
  flex: none;
}

.step .sarg {
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--t3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-text {
  font-size: 13.5px;
  line-height: 23px;
  color: var(--t1);
  white-space: pre-wrap;
  word-break: break-word;
}

.thinking {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 23px;
  color: var(--t3);
}

.thinking .dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--green);
  opacity: 0.35;
  animation: breathe 1.4s ease-in-out infinite;
}

.thinking .dot:nth-child(2) {
  animation-delay: 0.18s;
}

.thinking .dot:nth-child(3) {
  animation-delay: 0.36s;
}

.thinking-label {
  margin-left: 6px;
  font-size: 12px;
  color: var(--t4);
}

@keyframes breathe {
  0%,
  100% {
    opacity: 0.25;
    transform: scale(0.85);
  }
  50% {
    opacity: 1;
    transform: scale(1);
  }
}

/* 流式输出必须与历史消息用同一套 Markdown 排版。
   之前只定义了 p/ul/code/pre，标题会按浏览器默认 27px 渲染、
   链接是蓝色下划线、代码块没有边框和复制按钮样式，
   消息落库的瞬间整块会跳变一次。 */
.ai-text.md :deep(p) {
  margin: 0 0 12px;
}

.ai-text.md :deep(p:last-child) {
  margin-bottom: 0;
}

.ai-text.md :deep(h1),
.ai-text.md :deep(h2),
.ai-text.md :deep(h3),
.ai-text.md :deep(h4) {
  margin: 18px 0 8px;
  font-weight: 600;
  line-height: 1.4;
}

.ai-text.md :deep(h1) { font-size: 18px; }
.ai-text.md :deep(h2) { font-size: 16px; }
.ai-text.md :deep(h3) { font-size: 14.5px; }
.ai-text.md :deep(h4) { font-size: 13.5px; }

.ai-text.md :deep(ul),
.ai-text.md :deep(ol) {
  margin: 0 0 12px;
  padding-left: 22px;
}

.ai-text.md :deep(li) {
  margin-bottom: 4px;
}

.ai-text.md :deep(code) {
  font-family: var(--mono);
  font-size: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  border-radius: 4px;
  padding: 1px 5px;
}

.ai-text.md :deep(pre) {
  background: var(--bg-soft);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 12px 14px;
  overflow-x: auto;
  margin: 0 0 12px;
}

.ai-text.md :deep(pre code) {
  background: none;
  border: none;
  padding: 0;
  font-size: 12px;
  line-height: 1.7;
}

.ai-text.md :deep(a) {
  color: var(--green-t);
  text-decoration: underline;
  cursor: pointer;
}

.ai-text.md :deep(blockquote) {
  border-left: 3px solid var(--line-3);
  padding: 2px 0 2px 14px;
  margin: 0 0 12px;
  color: var(--t2);
}

.ai-text.md :deep(table) {
  border-collapse: collapse;
  margin: 0 0 12px;
  font-size: 12.5px;
  width: 100%;
  table-layout: fixed;
}

.ai-text.md :deep(th),
.ai-text.md :deep(td) {
  border: 1px solid var(--line-2);
  padding: 6px 10px;
  text-align: left;
  word-break: break-word;
  overflow-wrap: anywhere;
}

.ai-text.md :deep(th) {
  background: var(--bg-soft);
  font-weight: 500;
}

/* 代码块的头部（语言标签 + 复制按钮）也要有样式，
   否则流式时那两个元素是裸按钮 */
.ai-text.md :deep(.code-block) {
  margin: 0 0 12px;
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
}

.ai-text.md :deep(.code-head) {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  height: 28px;
  background: var(--bg-soft);
  border-bottom: 1px solid var(--line);
  font-size: 11px;
  color: var(--t4);
}

.ai-text.md :deep(.code-lang) {
  font-family: var(--mono);
}

.ai-text.md :deep(.code-copy) {
  height: 20px;
  padding: 0 8px;
  border-radius: 4px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  font-size: 11px;
  color: var(--t2);
  cursor: pointer;
}

.ai-text.md :deep(.code-copy:hover) {
  background: var(--bg-hover);
}

.ai-text.md :deep(.code-block pre) {
  margin: 0;
  border: none;
  border-radius: 0;
}

.chat-error {
  margin-top: 12px;
  padding: 10px 13px;
  border-radius: 8px;
  background: var(--danger-bg);
  border: 1px solid var(--danger-line);
  color: var(--danger-t);
  font-size: 12.5px;
  line-height: 1.6;
}

.empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 0 28px 60px;
}

.eico {
  width: 40px;
  height: 40px;
  border-radius: 10px;
  margin-bottom: 16px;
  background: var(--green-soft);
  display: grid;
  place-items: center;
}

.eico svg {
  width: 20px;
  height: 20px;
  stroke: var(--green);
  fill: none;
  stroke-width: 1.7;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.empty h2 {
  font-size: 17px;
  font-weight: 600;
  margin-bottom: 7px;
}

.empty p {
  font-size: 13px;
  color: var(--t3);
  text-align: center;
  max-width: 380px;
  line-height: 1.65;
}

.primary-btn {
  margin-top: 18px;
  height: 34px;
  padding: 0 18px;
  border-radius: var(--r);
  background: var(--green);
  color: #fff;
  font-size: 13px;
  font-weight: 500;
}

.primary-btn:hover {
  background: var(--green-h);
}

.tips {
  margin-top: 22px;
}

.tip {
  font-size: 11.5px;
  color: var(--t4);
}

.composer-wrap {
  flex: none;
  padding: 14px 28px 16px;
  transition: background 0.15s;
}

.composer-wrap.drag-over {
  background: var(--green-soft);
  border-radius: 10px;
  outline: 1px dashed var(--green);
  outline-offset: -6px;
}

.attach-row {
  max-width: 720px;
  margin: 0 auto 8px;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.attach-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  min-width: 0;
  height: 24px;
  padding: 0 8px;
  border-radius: 5px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  font-size: 11.5px;
  color: var(--t2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attach-chip .attach-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.attach-thumb {
  width: 16px;
  height: 16px;
  border-radius: 3px;
  object-fit: cover;
}

.attach-x {
  color: var(--t4);
  font-size: 9px;
}

.attach-x:hover {
  color: var(--red);
}

.composer {
  max-width: 720px;
  margin: 0 auto;
  border: 1px solid var(--line-2);
  border-radius: 14px;
  background: var(--surface);
  display: flex;
  flex-direction: column;
  min-height: 92px;
  position: relative;
  transition: border-color 0.16s ease;
}

.cmd-panel {
  position: absolute;
  left: 0;
  right: 0;
  bottom: calc(100% + 6px);
  background: var(--surface);
  border: 1px solid var(--line-2);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.1);
  padding: 4px;
  max-height: 380px;
  overflow-y: auto;
  z-index: 20;
}

.cmd-item {
  display: flex;
  align-items: baseline;
  gap: 10px;
  width: 100%;
  padding: 7px 9px;
  border-radius: 5px;
  text-align: left;
}

.cmd-item:hover,
.cmd-item.on {
  background: var(--bg-hover);
}

.cmd-name {
  font-family: var(--mono);
  font-size: 12px;
  color: var(--green-t);
  flex: none;
}

.cmd-desc {
  font-size: 11.5px;
  color: var(--t3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.composer textarea {
  flex: 1;
  padding: 14px 16px 4px;
  font-size: 13.5px;
  line-height: 22px;
  min-height: 48px;
  background: transparent;
}

.composer textarea::placeholder {
  color: var(--t4);
}

.composer-bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 6px;
  height: 42px;
  padding: 0 10px 0 10px;
}

.cb-btn {
  width: 28px;
  height: 28px;
  border-radius: 7px;
  display: grid;
  place-items: center;
  color: var(--t3);
  transition: background 0.13s, color 0.13s;
}

.cb-btn:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.cb-btn svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.ctx {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 28px;
  padding: 0 10px;
  border-radius: 7px;
  font-size: 11.5px;
  color: var(--t3);
  transition: background 0.13s, color 0.13s;
}

.ctx:hover,
.ctx.open {
  background: var(--bg-hover);
}

.ctx-bar {
  width: 120px;
  height: 8px;
  border-radius: 3px;
  background: #e7e7e9;
  overflow: hidden;
  flex: none;
}

.ctx-bar i {
  display: block;
  height: 100%;
  background: var(--green);
  border-radius: 3px;
  transition: width 0.3s;
}

.ctx-num {
  font-family: var(--mono);
  font-size: 12px;
  color: var(--t2);
  min-width: 42px;
  text-align: right;
}

.send-btn {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  flex: none;
  background: var(--green);
  color: #fff;
  display: grid;
  place-items: center;
  transition: background 0.12s, opacity 0.12s;
}

.send-btn:hover:not(:disabled) {
  background: var(--green-h);
}

.send-btn:disabled {
  opacity: 0.4;
  cursor: default;
}

.send-btn.stop {
  background: var(--red);
}

.send-btn svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.hint {
  max-width: 720px;
  margin: 8px auto 0;
  text-align: center;
  font-size: 11px;
  color: var(--t4);
}
</style>
