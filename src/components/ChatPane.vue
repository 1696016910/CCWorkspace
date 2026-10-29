<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useSessionStore } from '@/stores/session'
import { useWorkspaceStore } from '@/stores/workspace'
import { useChatStore } from '@/stores/chat'
import type { Attachment, ChatMessage, CommandInfo, SkillInfo } from '@/api/types'
import MessageItem from './MessageItem.vue'
import SlashPanel from './SlashPanel.vue'
import AttachmentStrip from './AttachmentStrip.vue'
import { renderMarkdown } from '@/utils/markdown'
import ContextPopover from './dialogs/ContextPopover.vue'
import FileViewer from './dialogs/FileViewer.vue'
import ImageViewer from './dialogs/ImageViewer.vue'
import FileTreeDrawer from './FileTreeDrawer.vue'
import { useToast } from '@/composables/useToast'
import * as chatApi from '@/api/chat'
import * as skillApi from '@/api/skill'
import * as workspaceApi from '@/api/workspace'
import * as fileApi from '@/api/file'
import { extractFilePaths } from '@/utils/markdown'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { open as openPick } from '@tauri-apps/plugin-dialog'

const sessions = useSessionStore()
const ws = useWorkspaceStore()
const chat = useChatStore()
const toast = useToast()
const viewingFile = ref<string | null>(null)
/** 图片大图预览的 data URL；null 表示没打开 */
const viewingImage = ref<string | null>(null)
/** 右侧文件树抽屉是否展开 */
const treeOpen = ref(false)
/** 文件树抽屉的引用，用于对话结束后触发刷新 */
const fileTree = ref<{ refresh: () => Promise<void> } | null>(null)

/**
 * 草稿按会话保存。
 *
 * 原来是一份 `ref('')`，切会话时串味：在 A 打的字会出现在 B 的输入框里。
 */
const current = computed(() => sessions.current)
const currentId = computed(() => current.value?.id ?? null)

const drafts = ref<Map<string, string>>(new Map())
/**
 * 输入框内容。
 *
 * 键必须用 `current?.id` 而不是 `sessions.activeId`：草稿会话的 activeId 是 null，
 * 用 null 做键会让 setter 直接丢弃输入，表现为「输入框打不进字、发送按钮一直灰着」。
 */
const input = computed({
  get: () => {
    const id = current.value?.id
    return id ? (drafts.value.get(id) ?? '') : ''
  },
  set: (v: string) => {
    const id = current.value?.id
    if (!id) return
    if (v) drafts.value.set(id, v)
    else drafts.value.delete(id)
  }
})

const thread = ref<HTMLElement | null>(null)
const textarea = ref<HTMLTextAreaElement | null>(null)
const composer = ref<HTMLElement | null>(null)


/** 是否只在当前会话正在生成——不能看全局，否则切到别的会话按钮也跟着变 */
const streaming = computed(() => chat.isStreaming(currentId.value))
const stopping = computed(() => chat.isStopping(currentId.value))
const liveText = computed(() => chat.streamText(currentId.value))
/** 本轮已发生的工具调用，最后一个就是"正在做什么" */
const liveTools = computed(() => chat.streamTools(currentId.value))

/** 工具目标可能是长路径/长命令，截断显示 */
function shortToolTarget(t: string): string {
  const one = t.replace(/\s+/g, ' ').trim()
  if (one.length <= 48) return one
  const parts = one.split(/[/\\]/)
  return parts.length > 1 ? parts[parts.length - 1] : one.slice(0, 46) + '…'
}

const workspace = computed(() => {
  const s = current.value
  if (!s) return null
  return ws.list.find((w) => w.id === s.workspaceId) ?? null
})
const workspaceName = computed(() => workspace.value?.name ?? '')

/* ── 上下文 ── */
const ctxOpen = ref(false)
/** 正在向 CLI 查询上下文用量 */
const probing = ref(false)
const compacting = ref(false)
/** 锚点位置：由触发按钮的 rect 算出，替代原来靠容器居中猜的位置 */
const ctxAnchor = ref<{ left: number; top: number; right: number } | null>(null)

function openContext(e: MouseEvent): void {
  const btn = e.currentTarget as HTMLElement
  const r = btn.getBoundingClientRect()
  ctxAnchor.value = { left: r.left, top: r.top, right: r.right }
  ctxOpen.value = true

  // 不在这里重新探测。
  //
  // `/context` 报的是"当前上下文构成"，与对话中逐轮累积的 usage 口径不同
  // （实测同会话对话中 7%，探测回来 5%）。每次打开都覆盖会让数字来回跳，
  // 用户会以为统计不准。只有完全没有数据时（新会话、从未测过）才探一次。
  const id = currentId.value
  if (id && !sessions.isDraft && !hasContextData.value) {
    probing.value = true
    void chatApi
      .probeContext(id)
      .catch(() => {})
      .finally(() => {
        setTimeout(() => (probing.value = false), 1500)
      })
  }
}

/** 压缩上下文。命令立即返回，结果由 chat:compact 事件带回。 */
async function doCompact(): Promise<void> {
  const id = currentId.value
  if (!id || compacting.value) return
  if (streaming.value) {
    toast.error('对话进行中，结束后再压缩')
    return
  }
  compacting.value = true
  try {
    await chatApi.compact(id)
  } catch (e) {
    compacting.value = false
    toast.error(e instanceof Error ? e.message : '压缩失败')
  }
}

/**
 * 上下文百分比。
 *
 * 不直接用库里存的 context_percent：旧版本用错的算法写过 150% 这类值，
 * 直接显示会一直错下去。有 tokens/max 就以它们为准重算，两者都没有才退回存的值。
 */
const ctxPercent = computed(() => {
  const c = current.value
  if (!c) return 0
  // 一律以 tokens/max 为准。
  // 不要再回退到 c.contextPercent：那个字段可能是历史脏数据（曾经显示过 100%），
  // 而 max 为 0 时它没有任何可信度。没有 tokens 就当没有数据。
  if (c.contextMax > 0 && c.contextTokens > 0) {
    const p = (c.contextTokens / c.contextMax) * 100
    return Math.min(Math.max(p, 0), 100)
  }
  return 0
})

/**
 * 上下文百分比文案。
 *
 * `contextMax` 为 0 说明这个会话还没测过（新建会话、还没发消息）——
 * 这时显示「—」而不是「0%」。0% 会让人以为统计出错，而不是「暂无数据」。
 */
const hasContextData = computed(() => (current.value?.contextMax ?? 0) > 0)

const ctxLabel = computed(() => {
  if (!current.value || !hasContextData.value) return '—'
  const p = ctxPercent.value
  if (p > 0 && p < 1) return `${p.toFixed(1)}%`
  return `${Math.round(p)}%`
})

/* ── 滚动位置与浮层 ── */
/** 用户已上翻离开底部：不自动跟随，并显示"回到最新" */
const awayFromBottom = ref(false)
/** 当前视口最上方的用户提问，用于顶部悬浮条 */
const stickyQuestion = ref<{ id: string; text: string } | null>(null)

/** 按消息 id 跳转 */
function scrollToMessage(id: string): void {
  const el = thread.value?.querySelector(`[data-mid="${id}"]`)
  if (!el) return
  el.scrollIntoView({ block: 'start', behavior: 'smooth' })
}

/**
 * 滚动时更新两个状态：
 * - 距底部超过一屏 → 视为在读历史，停止自动跟随
 * - 取视口内最靠上的用户消息作为悬浮条内容
 */
function onThreadScroll(): void {
  const el = thread.value
  if (!el) return
  const distance = el.scrollHeight - el.scrollTop - el.clientHeight
  awayFromBottom.value = distance > 120

  const users = Array.from(el.querySelectorAll<HTMLElement>('.msg-user[data-mid]'))
  const top = el.getBoundingClientRect().top
  const cur = users.find((n) => n.getBoundingClientRect().bottom > top + 8)
  stickyQuestion.value = cur
    ? {
        id: cur.dataset.mid ?? '',
        text: (cur.querySelector('.bubble')?.textContent ?? '').trim()
      }
    : null
}

/** 回到最新位置 */
function backToLatest(): void {
  const el = thread.value
  if (!el) return
  awayFromBottom.value = false
  el.scrollTo({ top: el.scrollHeight, behavior: 'smooth' })
}

/* ── 消息 ── */
/**
 * 滚动到底部。
 *
 * @param smooth 是否平滑
 * @param force  忽略"用户已上翻"的判断。切换会话时必须 true：
 *               新会话 scrollTop 初始为 0，不加 force 会被守卫当成上翻而直接返回，
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
      if (distance > el.clientHeight * 1.2) return
    }
    el.scrollTo({ top: height, behavior: smooth ? 'smooth' : 'auto' })
  }

  jump()
  // 消息用了 content-visibility: auto，视口外高度是惰性算的，
  // 第一次 scrollHeight 只反映已渲染部分，连做几帧校正才真正落到底
  if (force) {
    for (const delay of [16, 60, 160]) {
      await new Promise((r) => setTimeout(r, delay))
      jump()
    }
  }
}

async function removeCurrent(): Promise<void> {
  const id = current.value?.id
  if (!id) return
  if (chat.isStreaming(id)) await chat.stop(id)
  chat.forget(id)
  await sessions.remove(id)
}

/** 在资源管理器中打开当前工作区 */
async function revealWorkspace(): Promise<void> {
  const w = workspace.value
  if (!w) return
  try {
    await workspaceApi.reveal(w.id)
  } catch (e) {
    toast.error(e instanceof Error ? e.message : '打开失败')
  }
}

/** 流式内容的渲染结果，按帧节流 */
const streamHtml = ref('')
let streamTimer: ReturnType<typeof setTimeout> | null = null
let streamPending = false

function scheduleStreamRender(): void {
  if (streamTimer) {
    streamPending = true
    return
  }
  const run = (): void => {
    // 流式内容不缓存（每帧都是新串）也不高亮（未闭合代码块会反复全量重算）
    streamHtml.value = renderMarkdown(liveText.value, false, false)
    if (!awayFromBottom.value) void scrollToBottom(false)
    streamTimer = null
    if (streamPending) {
      streamPending = false
      streamTimer = setTimeout(run, 120)
    }
  }
  streamHtml.value = renderMarkdown(liveText.value, false, false)
  streamTimer = setTimeout(run, 120)
}

watch(liveText, () => scheduleStreamRender())

watch(streaming, (on) => {
  if (on) return
  if (streamTimer) clearTimeout(streamTimer)
  streamTimer = null
  streamPending = false
  streamHtml.value = ''
})

/** 把输入框高度贴合当前内容 */
function resizeTextarea(): void {
  const el = textarea.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`
}

watch(
  // 监听 currentId 而不是 activeId：草稿会话的 activeId 是 null，
  // 从真实会话切到草稿时 activeId 只从 X 变成 null，两者都触发，
  // 但用 currentId 才能区分「切到哪个草稿」——它的值是 '__draft__'。
  () => currentId.value,
  async (id) => {
    // 切换会话：关掉上一条会话的文件预览，恢复该会话自己的草稿与高度
    viewingFile.value = null
    attachments.value = []
    slashOpen.value = false
    chat.markSeen(id)
    await nextTick()
    resizeTextarea()
    void scrollToBottom(false, true)
  }
)

/** 会话被删掉时清掉它的草稿，否则 Map 只涨不消 */
watch(
  () => sessions.list.map((s) => s.id).join(','),
  () => {
    const alive = new Set(sessions.list.map((s) => s.id))
    // 草稿的键不在 list 里，但它的输入必须留着，否则切走再切回草稿会丢字
    alive.add('__draft__')
    const next = new Map<string, string>()
    for (const [k, v] of drafts.value) if (alive.has(k)) next.set(k, v)
    if (next.size !== drafts.value.size) drafts.value = next
    for (const id of [...chat.runs.keys()]) if (!alive.has(id)) chat.forget(id)
  }
)

/* ── 停止超时兜底 ── */
/**
 * 后端停止失败时不能让界面永远卡在"正在停止"。
 * 每两秒检查一次，超时由 store 强制复位并给出提示。
 */
const stopWatch = setInterval(() => {
  // 必须遍历全部会话：切走再切回来时，那个会话的计时器不能被漏掉，
  // 否则它会永远停在"正在停止"。
  for (const id of [...chat.runs.keys()]) {
    if (chat.checkStopTimeout(id)) {
      toast.error(chat.error ?? '停止超时')
    }
  }
}, 2000)

/**
 * 消息加载完成后滚到底部。
 *
 * 切会话时 activeId 先变、messages 异步后到，所以必须监听 messages。
 * 用长度比较：切会话时 0 → N 能触发；流式追加时另有逻辑，用 streaming 排除。
 */
watch(
  () => sessions.messages.length,
  async (len, prev) => {
    if (len > 0 && prev === 0) {
      await nextTick()
      void scrollToBottom(false, true)
    }
  }
)

/* ── 事件订阅 ── */
let unlisten: UnlistenFn | null = null
let unlistenCompact: UnlistenFn | null = null

onMounted(async () => {
  unlisten = await chatApi.onEvent((ev) => {
    chat.handle(ev, (sessionId) => {
      // done/error 之后重新拉消息与列表
      const activeId = sessions.activeId
      void sessions.load()
      if (sessionId === activeId) {
        // 记住刷新前的"是否在底部"：select 会整体替换消息列表，
        // DOM 重建后 scrollTop 归 0，守卫会误判成"用户在看历史"而跳过滚动，
        // 表现为回答完成后视图突然跳回问题所在的位置。
        const wasAtBottom = !awayFromBottom.value
        void sessions.select(activeId).then(() => {
          if (wasAtBottom) void scrollToBottom(false, true)
        })
        // 模型这一轮很可能改了文件，抽屉开着就顺手刷一下
        if (treeOpen.value) void fileTree.value?.refresh()
      }
      if (chat.error) toast.error(chat.error)
      // init 事件在本轮到达过，这时才拿得到全量命令清单
      void loadSlashSources()
      void nextTick(() => textarea.value?.focus())
    }, (sessionId) => {
      // 上下文用量校准回来：只刷新会话列表，不动消息、不抢焦点
      void sessions.load()
      if (sessionId === sessions.activeId) void sessions.select(sessionId)
    })
  })

  unlistenCompact = await chatApi.onCompact((r) => {
    compacting.value = false
    if (r.ok) {
      toast.success('上下文已压缩')
      const id = r.sessionId
      void sessions.load()
      if (id === sessions.activeId) void sessions.select(id)
      ctxOpen.value = false
    } else {
      toast.error(r.error ?? '压缩失败')
    }
  })

  void loadSlashSources()
})

onUnmounted(() => {
  if (streamTimer) clearTimeout(streamTimer)
  clearInterval(stopWatch)
  unlisten?.()
  unlistenCompact?.()
  unlisten = null
  unlistenCompact = null
})

/* ── 附件 ── */
const MAX_ATTACHMENTS = 6
/** 超过这个大小标黄提示：base64 膨胀后可能超出 IPC 与 CLI 的承受范围 */
const LARGE_ATTACHMENT_BYTES = 4.5 * 1024 * 1024

const attachments = ref<Attachment[]>([])
const dragging = ref(false)

function addAttachment(a: Attachment): void {
  if (attachments.value.length >= MAX_ATTACHMENTS) {
    toast.error(`最多只能添加 ${MAX_ATTACHMENTS} 个附件`)
    return
  }
  attachments.value = [...attachments.value, a]
}

function removeAttachment(id: string): void {
  attachments.value = attachments.value.filter((a) => a.id !== id)
}

const IMAGE_EXTS = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp']

function isImagePath(p: string): boolean {
  const ext = p.split('.').pop()?.toLowerCase() ?? ''
  return IMAGE_EXTS.includes(ext)
}

/**
 * 把 dialog 插件选出的绝对路径转成附件。
 *
 * 不用 FileReader：Tauri 的 `<input type="file">` 拿不到真实路径
 * （`.path` 是 Electron 的扩展），没法把文件位置告诉模型，也没法预览。
 */
async function pathToAttachment(path: string, size: number): Promise<Attachment | null> {
  const name = path.split(/[\/]/).pop() ?? path
  if (!isImagePath(path)) {
    return {
      id: `a-${Date.now()}-${Math.random().toString(36).slice(2)}`,
      name,
      kind: 'file',
      path,
      size
    }
  }
  try {
    const img = await fileApi.readImage(path)
    return {
      id: `a-${Date.now()}-${Math.random().toString(36).slice(2)}`,
      name: img.name,
      kind: 'image',
      path,
      size: img.size,
      preview: img.preview,
      data: img.data,
      mediaType: img.mediaType
    }
  } catch (e) {
    // 静默返回 null 会让用户觉得"点了没反应"，必须说清楚为什么没加上
    toast.error(`无法读取图片「${name}」：${e instanceof Error ? e.message : '未知错误'}`)
    return null
  }
}

async function pickAttachments(): Promise<void> {
  const picked = await openPick({
    multiple: true,
    title: '选择图片或文件',
    filters: [
      { name: '图片', extensions: [...IMAGE_EXTS] },
      { name: '全部文件', extensions: ['*'] }
    ]
  })
  if (!picked) return
  const list = Array.isArray(picked) ? picked : [picked]
  for (const p of list) {
    const a = await pathToAttachment(p, 0)
    if (a) addAttachment(a)
  }
  // 全都没加成功时也给一句，避免只在 console 里留痕
  if (!attachments.value.length) toast.error('没有可用的附件')
}

/**
 * 拖拽与粘贴只有 File 对象，没有绝对路径。
 *
 * 图片没关系：base64 直接发给模型，不需要路径。
 * 普通文件不行：模型只能靠路径去读，而这里拿不到路径 —— 提示用户改用选择按钮。
 */
function onPaste(e: ClipboardEvent): void {
  const items = e.clipboardData?.items
  if (!items) return
  const images: File[] = []
  for (const it of Array.from(items)) {
    if (it.kind !== 'file') continue
    const f = it.getAsFile()
    if (f && f.type.startsWith('image/')) images.push(f)
  }
  if (!images.length) return
  e.preventDefault()
  void addPastedImages(images)
}

async function addPastedImages(files: File[]): Promise<void> {
  for (const f of files) {
    if (attachments.value.length >= MAX_ATTACHMENTS) {
      toast.error(`最多只能添加 ${MAX_ATTACHMENTS} 个附件`)
      return
    }
    const url = await readFileAsDataUrl(f).catch(() => '')
    if (!url) continue
    const comma = url.indexOf(',')
    addAttachment({
      id: `a-${Date.now()}-${Math.random().toString(36).slice(2)}`,
      name: f.name || '粘贴的图片.png',
      kind: 'image',
      size: f.size,
      preview: url,
      data: comma >= 0 ? url.slice(comma + 1) : url,
      mediaType: f.type || 'image/png'
    })
  }
}

function readFileAsDataUrl(f: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader()
    r.onerror = () => reject(new Error('读取失败'))
    r.onload = () => resolve(String(r.result ?? ''))
    r.readAsDataURL(f)
  })
}

function onDragOver(e: DragEvent): void {
  e.preventDefault()
  dragging.value = true
}

function onDragLeave(): void {
  dragging.value = false
}

function onDrop(e: DragEvent): void {
  e.preventDefault()
  dragging.value = false
  const files = Array.from(e.dataTransfer?.files ?? [])
  if (!files.length) return

  const images = files.filter((f) => f.type.startsWith('image/'))
  const others = files.filter((f) => !f.type.startsWith('image/'))
  if (others.length) {
    toast.error('拖拽文件暂不支持（拿不到真实路径），请点回形针选择')
  }
  if (images.length) void addPastedImages(images)
}

/* ── 斜杠补全 ── */
const slashOpen = ref(false)
const slashQuery = ref('')
const slashItems = ref<{ name: string; description: string; group: string }[]>([])
/** CLI 首轮之后才有清单；在此之前面板给出说明 */
const slashReady = ref(false)

async function loadSlashSources(): Promise<void> {
  try {
    const m = await chatApi.meta()
    const groups: { name: string; description: string; group: string }[] = []
    if (m) {
      slashReady.value = true
      const skillSet = new Set(m.skills)
      const agentSet = new Set(m.agents)
      const seen = new Set<string>()
      for (const c of m.slashCommands) {
        if (seen.has(c)) continue
        seen.add(c)
        let group = '内置命令'
        if (skillSet.has(c)) group = 'Skill'
        else if (agentSet.has(c)) group = 'Agent'
        else if (c.includes(':')) group = '插件命令'
        groups.push({ name: c, description: '', group })
      }
      slashItems.value = groups
      return
    }
  } catch {
    // 拿不到 CLI 清单就退到本地扫描
  }

  // 兜底：本地扫描命令与 Skill，内置命令这次拿不到
  const projectDir = workspace.value?.path
  const [cmds, skills] = await Promise.all([
    skillApi.commandList(projectDir).catch(() => [] as CommandInfo[]),
    skillApi.list().catch(() => [] as SkillInfo[])
  ])
  slashItems.value = [
    ...cmds.map((c) => ({ name: c.name, description: c.description, group: '命令' })),
    ...skills.map((s) => ({ name: s.name, description: s.description, group: 'Skill' }))
  ]
  slashReady.value = slashItems.value.length > 0
}

/** 输入以 `/` 开头且还没敲空格时唤起面板 */
function syncSlashPanel(): void {
  const v = input.value
  if (!v.startsWith('/') || v.includes(' ')) {
    slashOpen.value = false
    return
  }
  slashQuery.value = v.slice(1)
  slashOpen.value = true
  // 首次打开时清单可能还没拉到（init 只在发过消息后才有），现场补一次
  if (!slashItems.value.length) void loadSlashSources()
}

function pickSlash(name: string): void {
  slashOpen.value = false
  input.value = ''
  void send(`/${name}`)
}

/* ── 输入 ── */
function onInput(e: Event): void {
  const el = e.target as HTMLTextAreaElement
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`
  syncSlashPanel()
}

/** 输入法合成中：此时回车是"选词"，不能当发送 */
const composing = ref(false)

function onCompositionStart(): void {
  composing.value = true
}

function onCompositionEnd(): void {
  // compositionend 与 keydown 同帧触发，延后一拍再放开，
  // 否则中文选词的那个回车仍会被当成发送
  setTimeout(() => (composing.value = false), 0)
}

function onKeydown(e: KeyboardEvent): void {
  // isComposing：Safari/WebView2 在合成期间也可能报 key=Enter
  if (composing.value || e.isComposing) return
  if (slashOpen.value && e.key === 'Escape') {
    slashOpen.value = false
    return
  }
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    void send()
  }
}

/** `override` 用于斜杠面板：直接发命令，不经过输入框 */
async function send(override?: string): Promise<void> {
  const text = (override ?? input.value).trim()
  const files = attachments.value
  if ((!text && !files.length) || !current.value || streaming.value) {
    console.warn('[wb] send 被守卫拦下', {
      text: text.length,
      files: files.length,
      hasCurrent: !!current.value,
      streaming: streaming.value
    })
    return
  }

  if (!override) {
    input.value = ''
    // 清空后把输入框收回默认高度，否则会留着被撑高的空框
    if (textarea.value) textarea.value.style.height = 'auto'
  }
  slashOpen.value = false

  // 附件只在发送前显示，发出去即从界面清掉（用户明确要求不保留）
  attachments.value = []

  // 乐观消息必须带上附件元数据：发出去到落库回来之间，界面显示的是这一条。
  // 不带的话附件要等 AI 回复完、重新拉消息之后才出现，中间那段看起来像附件丢了。
  const optimistic: ChatMessage = {
    id: `local-${Date.now()}`,
    role: 'user',
    text: text || '（附件）',
    timestamp: new Date().toISOString(),
    attachments: files.map((a) => ({
      kind: a.kind,
      name: a.name,
      path: a.path,
      size: a.size
    }))
  }
  sessions.messages = [...sessions.messages, optimistic]
  awayFromBottom.value = false
  void scrollToBottom(false, true)

  // 草稿会话在这里才真正落库：输入第一条消息之前它在左侧列表里不存在，
  // 也不会在数据库里留下空会话。
  let sessionId: string
  try {
    sessionId = sessions.isDraft ? await sessions.commitDraft() : (current.value?.id ?? '')
  } catch (e) {
    sessions.messages = sessions.messages.filter((m) => m.id !== optimistic.id)
    attachments.value = files
    if (!override) input.value = text
    toast.error(e instanceof Error ? e.message : '创建会话失败')
    return
  }
  if (!sessionId) return

  try {
    await chat.send(sessionId, text, files)
  } catch {
    // 发送失败必须回滚：否则列表里留一条库里不存在的消息
    sessions.messages = sessions.messages.filter((m) => m.id !== optimistic.id)
    attachments.value = files
    if (!override) input.value = text
    toast.error(chat.error ?? '发送失败')
  }
}

/** 流式文本里的代码块复制与链接拦截 */
async function onStreamClick(e: MouseEvent): Promise<void> {
  const target = e.target as HTMLElement
  const link = target.closest('a')
  if (link) {
    e.preventDefault()
    const href = link.getAttribute('href')
    if (href) console.info('[link]', href)
    return
  }

  if (!target.matches('[data-copy]')) return
  // 必须限定在 pre 里：块内可能有其它 code 元素，取第一个会复制走用户看不见的内容
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

async function stop(): Promise<void> {
  const id = currentId.value
  if (!id) return
  await chat.stop(id)
  await nextTick()
  textarea.value?.focus()
}

function openFile(path: string): void {
  viewingFile.value = path
}

function previewImage(src: string): void {
  viewingImage.value = src
}

/**
 * 可预览路径集合。
 *
 * 批量判定而不是让每条消息自己调：历史会话几十条消息，逐条调用就是几十次 IPC。
 * 默认空集合 = 全部灰显，拿到结果后再放开，避免用户抢在未知状态上点击。
 */
const previewable = ref<Set<string>>(new Set())

watch(
  () => sessions.messages.map((m) => m.text).join(String.fromCharCode(0)),
  async () => {
    const paths = [...new Set(sessions.messages.flatMap((m) => extractFilePaths(m.text)))]
    if (!paths.length) {
      previewable.value = new Set()
      return
    }
    try {
      const flags = await fileApi.check(paths)
      previewable.value = new Set(paths.filter((_, i) => flags[i]))
    } catch {
      previewable.value = new Set()
    }
  },
  { immediate: true }
)
</script>

<template>
  <main class="main">
    <div class="main-head">
      <span class="mh-title" :title="current?.title ?? ''">{{
        current ? current.title || '(新会话)' : '未选择会话'
      }}</span>
      <span v-if="current" class="mh-ws" :title="workspaceName">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7">
          <path d="M3.5 7.5h6l1.8 2h9.2v9h-17v-11z" />
        </svg>
        <span>{{ workspaceName }}</span>
      </span>
      <span class="sp" />
      <button
        v-if="current && workspace"
        class="mh-btn"
        :class="{ on: treeOpen }"
        title="工作区文件"
        @click="treeOpen = !treeOpen"
      >
        <svg viewBox="0 0 24 24">
          <path d="M4 6h5M4 12h9M4 18h6" />
          <path d="M15 6h5M15 12h5M15 18h5" />
        </svg>
      </button>
      <button v-if="current && workspace" class="mh-btn" title="在资源管理器中打开工作区" @click="revealWorkspace">
        <svg viewBox="0 0 24 24">
          <path d="M14.5 4.5h5v5" />
          <path d="M19.5 4.5L11 13" />
          <path d="M17.5 13.5v5a1.5 1.5 0 01-1.5 1.5H5.5A1.5 1.5 0 014 18.5V8a1.5 1.5 0 011.5-1.5H10" />
        </svg>
      </button>
      <button v-if="current" class="mh-btn" title="复制会话" @click="sessions.duplicate(current.id)">
        <svg viewBox="0 0 24 24">
          <rect x="8" y="8" width="11" height="11" rx="2" />
          <path d="M15 5.5H6A1.5 1.5 0 004.5 7v9" />
        </svg>
      </button>
      <button v-if="current" class="mh-btn" title="删除会话" @click="removeCurrent">
        <svg viewBox="0 0 24 24">
          <path d="M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13" />
          <path d="M10.5 10v6M13.5 10v6" />
        </svg>
      </button>
    </div>

    <div class="main-body">
      <div class="main-col">
    <!-- 悬浮的当前提问：只在滚动离开底部时出现 -->
    <Transition name="sticky">
      <button
        v-if="current && awayFromBottom && stickyQuestion"
        class="sticky-q"
        :title="stickyQuestion.text"
        @click="scrollToMessage(stickyQuestion.id)"
      >
        <span class="sq-dot" />
        <span class="sq-text">{{ stickyQuestion.text }}</span>
      </button>
    </Transition>

    <div v-if="current" ref="thread" class="thread" @scroll.passive="onThreadScroll">
      <div class="thread-inner">
        <div v-if="!sessions.messages.length && !streaming" class="thread-empty">
          <div class="te-ico">✦</div>
          <div class="te-title">开始对话</div>
          <div class="te-desc">在下方输入你想让它做的事</div>
        </div>

        <MessageItem
          v-for="m in sessions.messages"
          :key="m.id"
          :message="m"
          :previewable="previewable"
          :duration-ms="chat.lastDuration.get(m.id)"
          @open-file="openFile"
          @preview-image="previewImage"
        />

        <div
          v-if="streaming"
          class="msg-ai"
        >
          <div class="a-avatar">C</div>
          <div class="a-body">
            <div class="ai-role">
              <span class="who">Claude</span>
            </div>
            <div v-if="liveText" class="ai-text md" v-html="streamHtml" @click="onStreamClick" />

            <!-- 工具执行中：与 CLI 一致地显示"正在做什么" -->
            <div v-if="streaming && liveTools.length" class="tool-live">
              <span class="tl-spin" />
              <span class="tl-name">{{ liveTools[liveTools.length - 1].name }}</span>
              <span v-if="liveTools[liveTools.length - 1].target" class="tl-target">
                {{ shortToolTarget(liveTools[liveTools.length - 1].target) }}
              </span>
              <span class="tl-count">{{ liveTools.length }} 步</span>
            </div>

            <div v-if="!liveText && !liveTools.length" class="thinking">
              <span class="dots"><span class="dot" /><span class="dot" /><span class="dot" /></span>
              <span class="thinking-label">{{ stopping ? '正在停止…' : '正在思考…' }}</span>
            </div>
          </div>
        </div>

      </div>
    </div>

    <div v-else class="empty">
      <div class="eico">
        <svg viewBox="0 0 24 24"><path d="M4.5 5.5h15v10.5h-9l-6 4v-14.5z" /></svg>
      </div>
      <h2>{{ ws.list.length === 0 ? '开始使用 Claude 工作台' : '选择一个会话开始' }}</h2>
      <p>{{ ws.list.length === 0 ? '先新建一个工作区，指向你的项目目录' : '从左侧列表里选一个会话' }}</p>
    </div>

    <!-- 回到最新 -->
    <Transition name="jump">
      <button
        v-if="current && awayFromBottom"
        class="jump-latest"
        :class="{ live: streaming }"
        @click="backToLatest"
      >
        <span v-if="streaming" class="jd" />
        <svg v-else viewBox="0 0 24 24"><path d="M12 5v14M5 12l7 7 7-7" /></svg>
        <span>回到最新</span>
      </button>
    </Transition>

    <div
      v-if="current"
      ref="composer"
      class="composer-wrap"
      :class="{ dragging }"
      @dragover="onDragOver"
      @dragleave="onDragLeave"
      @drop="onDrop"
    >
      <div class="composer-stack">
        <!-- 放在 stack 内：与输入框同一个宽度基准，left/right:0 才是输入框的边 -->
        <SlashPanel
          v-if="slashOpen"
          :query="slashQuery"
          :items="slashItems"
          :ready="slashReady"
          @pick="pickSlash"
          @close="slashOpen = false"
        />

        <div v-if="chat.error" class="chat-error">
          <span class="ce-text">{{ chat.error }}</span>
          <button class="ce-x" title="关闭" @click="chat.error = null">✕</button>
        </div>

        <AttachmentStrip
          v-if="attachments.length"
          :items="attachments"
          :large-bytes="LARGE_ATTACHMENT_BYTES"
          @remove="removeAttachment"
        />

      <div class="composer">
        <textarea
          ref="textarea"
          v-model="input"
          rows="2"
          placeholder="描述你想让它做的事，或输入 / 调用命令与 Skill..."
          @input="onInput"
          @keydown="onKeydown"
          @paste="onPaste"
          @compositionstart="onCompositionStart"
          @compositionend="onCompositionEnd"
        />
        <div class="composer-bar">
          <button class="cb-btn" title="添加图片或文件" @click="pickAttachments">
            <svg viewBox="0 0 24 24">
              <path d="M8 12.5l4.5-4.5a2.5 2.5 0 013.5 3.5L9.5 18a4 4 0 01-5.5-5.5l7-7" />
            </svg>
          </button>
          <span class="sp" />
          <button class="ctx" title="上下文用量" @click="openContext">
            <span class="ctx-bar"><i :style="{ width: Math.min(ctxPercent, 100) + '%' }" /></span>
            {{ ctxLabel }}
          </button>
          <button
            v-if="!streaming"
            class="send-btn"
            :disabled="!input.trim() && !attachments.length"
            title="发送"
            @click="send()"
          >
            <svg viewBox="0 0 24 24"><path d="M12 19V5M5 12l7-7 7 7" /></svg>
          </button>
          <button v-else class="send-btn stop" :disabled="stopping" :title="stopping ? '正在停止…' : '停止'" @click="stop">
            <svg viewBox="0 0 24 24"><rect x="7" y="7" width="10" height="10" rx="1.5" /></svg>
          </button>
        </div>
      </div>
      </div>
      <div class="cm-hint">Enter 发送 · Shift + Enter 换行</div>
      </div>
      </div>

      <FileTreeDrawer
        ref="fileTree"
        v-if="treeOpen && workspace"
        :workspace-id="workspace.id"
        :root-path="workspace.path"
        @close="treeOpen = false"
        @open-file="openFile"
      />
    </div>

    <ContextPopover
      v-if="ctxOpen"
      :anchor="ctxAnchor"
      :streaming="streaming"
      :percent="ctxPercent"
      :has-data="hasContextData"
      :probing="probing"
      :tokens="current?.contextTokens ?? 0"
      :max="current?.contextMax ?? 0"
      :compacting="compacting"
      @close="ctxOpen = false"
      @compact="doCompact"
    />

    <FileViewer v-if="viewingFile" :path="viewingFile" @close="viewingFile = null" />
    <ImageViewer v-if="viewingImage" :src="viewingImage" @close="viewingImage = null" />
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

/* 头部以下是横向布局：左边会话区，右边文件树抽屉。
   抽屉必须在横向容器里，放在 .main 下会掉到最底部。 */
.main-body {
  flex: 1;
  min-height: 0;
  min-width: 0;
  display: flex;
}

.main-col {
  flex: 1;
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.main-head {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 48px;
  padding: 0 24px;
  border-bottom: 1px solid var(--line);
  min-width: 0;
  overflow: hidden;
}

.mh-title {
  font-size: 14px;
  font-weight: 500;
  line-height: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 0 1 auto;
}

.mh-ws {
  font-size: 11.5px;
  line-height: 1;
  color: var(--t4);
  display: flex;
  align-items: center;
  gap: 5px;
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}

.mh-ws > span {
  overflow: hidden;
  text-overflow: ellipsis;
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
  transition: background 0.12s, color 0.12s;
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

.sticky-q {
  flex: none;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin: 8px auto;
  max-width: min(680px, calc(100% - 48px));
  padding: 9px 14px;
  border: 1px solid var(--line-2);
  border-radius: 10px;
  background: var(--surface);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.07);
  font-size: 12.5px;
  line-height: 1.5;
  color: var(--t2);
  text-align: left;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}

.sticky-q:hover {
  background: var(--bg-hover);
  border-color: var(--line-3);
}

.sticky-q .sq-dot {
  flex: none;
  width: 6px;
  height: 6px;
  margin-top: 6px;
  border-radius: 50%;
  background: var(--green);
}

.sticky-q .sq-text {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}

.sticky-enter-active,
.sticky-leave-active {
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.sticky-enter-from,
.sticky-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

.thread {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 26px 0 32px;
}

.thread-inner {
  /* 随窗口增长，但停在可读上限：13.5px 字号下 900px 约一行 85 字符，
     再宽眼睛回行会串行。100% - 96px 保证窄窗口下左右各留 48px。 */
  max-width: min(900px, calc(100% - 96px));
  margin: 0 auto;
  /* 不加左右内边距：.composer-stack 没有，加了会让输入框和消息错开 28px */
}

.thread-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 80px 20px;
  text-align: center;
}

.te-ico {
  font-size: 32px;
  color: var(--green);
  margin-bottom: 14px;
}

.te-title {
  font-size: 15px;
  font-weight: 500;
  margin-bottom: 6px;
}

.te-desc {
  font-size: 12.5px;
  color: var(--t4);
}

.chat-error {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
  padding: 9px 12px;
  border-radius: 8px;
  background: var(--danger-bg);
  border: 1px solid var(--danger-line);
  color: var(--danger-t);
  font-size: 12px;
  line-height: 1.5;
}

.ce-text {
  flex: 1;
  min-width: 0;
}

.ce-x {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  color: var(--danger-t);
  font-size: 10px;
  flex: none;
  opacity: 0.7;
}

.ce-x:hover {
  opacity: 1;
  background: var(--danger-soft);
}

.empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  color: var(--t4);
}

.eico {
  width: 52px;
  height: 52px;
  border-radius: 14px;
  background: var(--bg-soft);
  display: grid;
  place-items: center;
  margin-bottom: 16px;
}

.eico svg {
  width: 24px;
  height: 24px;
  stroke: var(--t3);
  fill: none;
  stroke-width: 1.6;
}

.empty h2 {
  font-size: 15px;
  font-weight: 500;
  color: var(--t2);
  margin-bottom: 6px;
}

.empty p {
  font-size: 12.5px;
}

.tool-live {
  display: flex;
  align-items: center;
  gap: 8px;
  margin: 4px 0 10px;
  padding: 7px 11px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: var(--surface-2);
  font-size: 11.5px;
  color: var(--t2);
  max-width: 100%;
  overflow: hidden;
}

.tl-spin {
  width: 12px;
  height: 12px;
  flex: none;
  border: 2px solid var(--line-3);
  border-top-color: var(--green);
  border-radius: 50%;
  animation: tl-spin 0.7s linear infinite;
}

@keyframes tl-spin {
  to {
    transform: rotate(360deg);
  }
}

.tl-name {
  font-weight: 500;
  color: var(--t1);
  flex: none;
}

.tl-target {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--t4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.tl-count {
  margin-left: auto;
  flex: none;
  font-size: 10.5px;
  color: var(--t4);
  padding-left: 8px;
}

.thinking {
  display: flex;
  align-items: center;
  gap: 7px;
  height: 28px;
  color: var(--t3);
  font-size: 12.5px;
}

.thinking .dots {
  display: flex;
  gap: 3px;
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

.msg-ai {
  display: flex;
  gap: 12px;
  margin-bottom: 26px;
}

.a-avatar {
  width: 26px;
  height: 26px;
  flex: none;
  border-radius: 8px;
  background: var(--green-soft);
  color: var(--green);
  display: grid;
  place-items: center;
  font-size: 12px;
  font-weight: 600;
  margin-top: 1px;
}

.a-body {
  flex: 1;
  min-width: 0;
}

.ai-role {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  font-size: 11.5px;
  color: var(--t4);
}

.ai-role .who {
  color: var(--t2);
  font-weight: 500;
}

.ai-text {
  font-size: 13.5px;
  line-height: 23px;
  color: var(--t1);
  word-break: break-word;
}

.ai-text :deep(p) {
  margin: 0 0 12px;
}

.ai-text :deep(p:last-child) {
  margin-bottom: 0;
}

.ai-text :deep(code) {
  font-family: var(--mono);
  font-size: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  border-radius: 4px;
  padding: 1px 5px;
}

.ai-text :deep(pre) {
  background: var(--bg-soft);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 12px 14px;
  overflow-x: auto;
  margin: 0 0 12px;
  font-family: var(--mono);
  font-size: 12px;
  line-height: 1.7;
}

.ai-text :deep(ul),
.ai-text :deep(ol) {
  margin: 0 0 12px;
  padding-left: 22px;
}

.ai-text :deep(li) {
  margin-bottom: 4px;
}

.jump-latest {
  position: absolute;
  /* 贴在输入区上方。输入框高度随内容变化，用 align-self 不好算，
     所以用 bottom 的估算值并留足余量，避免压住输入框。 */
  bottom: 168px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 16;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  /* 不换行：按钮高 30px，文字一换行就撑成竖排、圆角崩坏 */
  white-space: nowrap;
  padding: 0 14px;
  border-radius: 15px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  box-shadow: 0 4px 14px rgba(0, 0, 0, 0.1);
  font-size: 12px;
  color: var(--t2);
  transition: background 0.15s, border-color 0.15s;
}

.jump-latest:hover {
  background: var(--bg-hover);
  border-color: var(--line-3);
}

.jump-latest.live {
  border-color: var(--green);
  color: var(--green);
}

.jump-latest.live:hover {
  background: var(--green-soft);
}

.jump-latest .jd {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--green);
  flex: none;
}

/* svg 必须显式给尺寸：默认会按 viewBox 撑满，把 30px 高的按钮挤变形 */
.jump-latest svg {
  width: 13px;
  height: 13px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
  flex: none;
}

/* 同理：文字不能被压缩换行 */
.jump-latest span {
  flex: none;
  white-space: nowrap;
}

.jump-enter-active,
.jump-leave-active {
  transition: opacity 0.16s ease, transform 0.16s ease;
}

.jump-enter-from,
.jump-leave-to {
  opacity: 0;
  transform: translate(-50%, 6px);
}

.composer-wrap {
  flex: none;
  /* 左右不留内边距：宽度由 .composer-stack 的 max-width 控制，
     这里再加 28px 会让输入框比消息窄 56px */
  padding: 14px 0 16px;
  position: relative;
}

/* 附件条、输入框、快捷键提示统一受这个宽度约束 */
.composer-stack {
  /* 必须与 .thread-inner 用同一个宽度，否则输入框和消息左右边缘对不齐 */
  max-width: min(900px, calc(100% - 96px));
  margin: 0 auto;
  position: relative;
}

.composer {
  border: 1px solid var(--line-2);
  border-radius: 14px;
  background: var(--surface);
  display: flex;
  flex-direction: column;
  transition: border-color 0.16s;
}

.composer:focus-within {
  border-color: var(--line-3);
}

.composer textarea {
  padding: 14px 16px 8px;
  font-size: 13.5px;
  line-height: 22px;
  min-height: 48px;
  max-height: 180px;
  overflow-y: auto;
  color: var(--t1);
}

.composer textarea::placeholder {
  color: var(--t4);
}

.file-input {
  display: none;
}

.composer-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 42px;
  padding: 0 10px;
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
  width: 16px;
  height: 16px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.8;
  stroke-linecap: round;
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
  transition: background 0.13s;
}

.ctx:hover {
  background: var(--bg-hover);
}

.ctx-bar {
  width: 64px;
  height: 4px;
  border-radius: 2px;
  background: var(--bg-soft);
  overflow: hidden;
}

.ctx-bar i {
  display: block;
  height: 100%;
  background: var(--green);
  border-radius: 2px;
}

.send-btn {
  width: 30px;
  height: 30px;
  border-radius: 8px;
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

.send-btn svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
}

.send-btn.stop {
  background: var(--red);
}

.cm-hint {
  text-align: center;
  font-size: 11px;
  color: var(--t4);
  padding-top: 8px;
}
</style>
