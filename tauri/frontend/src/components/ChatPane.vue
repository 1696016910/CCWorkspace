<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import { useSessionStore } from '@/stores/session'
import { useWorkspaceStore } from '@/stores/workspace'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage } from '@/api/types'
import MessageItem from './MessageItem.vue'
import { renderMarkdown } from '@/utils/markdown'
import ContextPopover from './ContextPopover.vue'
import FileViewer from './FileViewer.vue'
import { useToast } from '@/composables/useToast'
import * as chatApi from '@/api/chat'
import type { UnlistenFn } from '@tauri-apps/api/event'

const sessions = useSessionStore()
const ws = useWorkspaceStore()
const chat = useChatStore()
const toast = useToast()
const viewingFile = ref<string | null>(null)

const input = ref('')
const thread = ref<HTMLElement | null>(null)
const textarea = ref<HTMLTextAreaElement | null>(null)

const current = computed(() => sessions.current)

const workspaceName = computed(() => {
  const s = current.value
  if (!s) return ''
  return ws.list.find((w) => w.id === s.workspaceId)?.name ?? ''
})

/* ── 上下文 ── */
const ctxOpen = ref(false)

const ctxPercent = computed(() => current.value?.contextPercent ?? 0)

/** 小于 1% 时保留一位小数，避免一进来就显示 0% */
const ctxLabel = computed(() => {
  if (!current.value) return '—'
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
  if (chat.streamingSessionId === id) await chat.stop()
  await sessions.remove(id)
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
    streamHtml.value = renderMarkdown(chat.streamText, false, false)
    if (!awayFromBottom.value) void scrollToBottom(false)
    streamTimer = null
    if (streamPending) {
      streamPending = false
      streamTimer = setTimeout(run, 120)
    }
  }
  streamHtml.value = renderMarkdown(chat.streamText, false, false)
  streamTimer = setTimeout(run, 120)
}

watch(
  () => chat.streamText,
  () => scheduleStreamRender()
)

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

watch(
  () => sessions.activeId,
  () => {
    if (!input.value && textarea.value) textarea.value.style.height = 'auto'
  }
)

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

onMounted(async () => {
  unlisten = await chatApi.onEvent((ev) => {
    chat.handle(ev, () => {
      // done/error 之后重新拉消息与列表
      const activeId = sessions.activeId
      void sessions.load()
      if (ev.sessionId === activeId) {
        void sessions.select(activeId)
        void scrollToBottom()
      }
      if (chat.error) toast.error(chat.error)
      void nextTick(() => textarea.value?.focus())
    })
  })
})

onUnmounted(() => {
  if (streamTimer) clearTimeout(streamTimer)
  unlisten?.()
  unlisten = null
})

/* ── 输入 ── */
function onInput(e: Event): void {
  const el = e.target as HTMLTextAreaElement
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`
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
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    void send()
  }
}

async function send(): Promise<void> {
  const text = input.value.trim()
  if (!text || !current.value || chat.streaming) return

  input.value = ''
  // 清空后把输入框收回默认高度，否则会留着被撑高的空框
  if (textarea.value) textarea.value.style.height = 'auto'

  const optimistic: ChatMessage = {
    id: `local-${Date.now()}`,
    role: 'user',
    text,
    timestamp: new Date().toISOString()
  }
  sessions.messages = [...sessions.messages, optimistic]
  awayFromBottom.value = false
  void scrollToBottom(false, true)

  const sessionId = current.value.id
  try {
    await chat.send(sessionId, text)
  } catch {
    // 发送失败必须回滚：否则列表里留一条库里不存在的消息
    sessions.messages = sessions.messages.filter((m) => m.id !== optimistic.id)
    input.value = text
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
  await chat.stop()
  await nextTick()
  textarea.value?.focus()
}

function openFile(path: string): void {
  viewingFile.value = path
}
</script>

<template>
  <main class="main">
    <div class="main-head">
      <span class="mh-title" :title="current?.title ?? ''">{{
        current?.title || '未选择会话'
      }}</span>
      <span v-if="current" class="mh-ws" :title="workspaceName">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7">
          <path d="M3.5 7.5h6l1.8 2h9.2v9h-17v-11z" />
        </svg>
        <span>{{ workspaceName }}</span>
      </span>
      <span class="sp" />
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
        <div v-if="!sessions.messages.length && !chat.streaming" class="thread-empty">
          <div class="te-ico">✦</div>
          <div class="te-title">开始对话</div>
          <div class="te-desc">在下方输入你想让它做的事</div>
        </div>

        <MessageItem
          v-for="m in sessions.messages"
          :key="m.id"
          :message="m"
          @open-file="openFile"
        />

        <div
          v-if="chat.streaming && chat.streamingSessionId === current?.id"
          class="msg-ai"
        >
          <div class="a-avatar">C</div>
          <div class="a-body">
            <div class="ai-role">
              <span class="who">Claude</span>
            </div>
            <div v-if="chat.streamText" class="ai-text md" v-html="streamHtml" @click="onStreamClick" />
            <div v-else class="thinking">
              <span class="dots"><span class="dot" /><span class="dot" /><span class="dot" /></span>
              <span class="thinking-label">{{ chat.stopping ? '正在停止…' : '正在思考…' }}</span>
            </div>
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
      <p>{{ ws.list.length === 0 ? '先新建一个工作区，指向你的项目目录' : '从左侧列表里选一个会话' }}</p>
    </div>

    <!-- 回到最新 -->
    <Transition name="jump">
      <button
        v-if="current && awayFromBottom"
        class="jump-latest"
        :class="{ live: chat.streaming && chat.streamingSessionId === current?.id }"
        @click="backToLatest"
      >
        <span v-if="chat.streaming && chat.streamingSessionId === current?.id" class="jd" />
        <svg v-else viewBox="0 0 24 24"><path d="M12 5v14M5 12l7 7 7-7" /></svg>
        <span>回到最新</span>
      </button>
    </Transition>

    <div v-if="current" class="composer-wrap">
      <div class="composer">
        <textarea
          ref="textarea"
          v-model="input"
          rows="2"
          placeholder="描述你想让它做的事，或输入 / 调用命令与 Skill..."
          @input="onInput"
          @keydown="onKeydown"
          @compositionstart="onCompositionStart"
          @compositionend="onCompositionEnd"
        />
        <div class="composer-bar">
          <button class="cb-btn" title="上下文用量" @click="ctxOpen = true">
            <svg viewBox="0 0 24 24">
              <circle cx="12" cy="12" r="9" />
              <path d="M12 8v5l3 2" />
            </svg>
          </button>
          <span class="sp" />
          <button class="ctx" title="上下文用量" @click="ctxOpen = true">
            <span class="ctx-bar"><i :style="{ width: Math.min(ctxPercent, 100) + '%' }" /></span>
            {{ ctxLabel }}
          </button>
          <button
            v-if="!chat.streaming"
            class="send-btn"
            :disabled="!input.trim()"
            title="发送"
            @click="send"
          >
            <svg viewBox="0 0 24 24"><path d="M12 19V5M5 12l7-7 7 7" /></svg>
          </button>
          <button v-else class="send-btn stop" title="停止" @click="stop">
            <svg viewBox="0 0 24 24"><rect x="7" y="7" width="10" height="10" rx="1.5" /></svg>
          </button>
        </div>
      </div>
      <div class="cm-hint">Enter 发送 · Shift + Enter 换行</div>
    </div>

    <ContextPopover
      v-if="ctxOpen"
      :percent="ctxPercent"
      :tokens="current?.contextTokens ?? 0"
      :max="current?.contextMax ?? 0"
      info
      @close="ctxOpen = false"
    />

    <FileViewer v-if="viewingFile" :path="viewingFile" @close="viewingFile = null" />
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
  max-width: 720px;
  margin: 0 auto;
  padding: 0 28px;
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
  margin: 12px 0;
  padding: 10px 14px;
  border-radius: 8px;
  background: var(--danger-bg);
  border: 1px solid var(--danger-line);
  font-size: 12.5px;
  color: var(--danger-t);
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
  bottom: 148px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 16;
  display: flex;
  align-items: center;
  gap: 6px;
  height: 30px;
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
  padding: 14px 28px 16px;
}

.composer {
  max-width: 720px;
  margin: 0 auto;
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
