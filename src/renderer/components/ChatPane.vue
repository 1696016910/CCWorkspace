<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useSessionStore } from '@/stores/session'
import { useWorkspaceStore } from '@/stores/workspace'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage, ContextUsageDetail, SlashCommandInfo } from '@shared/types'
import MessageItem from './MessageItem.vue'
import ContextPopover from './ContextPopover.vue'

const sessions = useSessionStore()
const ws = useWorkspaceStore()
const chat = useChatStore()

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
    const d = await window.api.context.usage(current.value.id)
    if (d) ctxDetail.value = d
  } finally {
    ctxLoading.value = false
  }
}

async function toggleContext(): Promise<void> {
  ctxOpen.value = !ctxOpen.value
  if (ctxOpen.value) await loadContext()
}

async function doCompact(): Promise<void> {
  if (!current.value) return
  ctxOpen.value = false
  await chat.compact(current.value.id)
  await loadContext()
  await sessions.load()
}

/* ── 消息 ── */
async function scrollToBottom(): Promise<void> {
  await nextTick()
  thread.value?.scrollTo({ top: thread.value.scrollHeight, behavior: 'smooth' })
}

function onMessageSaved(msg: ChatMessage): void {
  sessions.messages = [...sessions.messages, msg]
  void sessions.load()
  void loadContext()
  void scrollToBottom()
}

onMounted(() => {
  chat.listen({
    onDone: onMessageSaved,
    onFinish: (id) => sessions.setRunning(id, false)
  })
})

watch(
  () => sessions.currentId,
  async () => {
    attachments.value = []
    ctxDetail.value = null
    if (current.value) {
      await loadContext()
      if (commands.value.length === 0) {
        commands.value = await window.api.commands.list(current.value.id)
      }
    }
    await nextTick()
    scrollToBottom()
  }
)

watch(
  () => chat.streamText,
  () => void scrollToBottom()
)

/* ── 输入框与命令面板 ── */
function onInput(e: Event): void {
  const el = e.target as HTMLTextAreaElement
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`

  const value = el.value
  cmdOpen.value = value.startsWith('/')
  if (cmdOpen.value) {
    cmdIndex.value = 0
  }
}

const filteredCommands = computed(() => {
  const q = input.value.startsWith('/') ? input.value.slice(1).toLowerCase() : ''
  const list = commands.value.filter((c) => c.name.toLowerCase().includes(q))
  return list.slice(0, 8)
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

async function pickFiles(): Promise<void> {
  const files = await window.api.dialog.pickFiles()
  if (files.length) {
    attachments.value = [...attachments.value, ...files]
  }
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

  await chat.send(current.value.id, prefix + text)
}

async function stop(): Promise<void> {
  await chat.stop()
}
</script>

<template>
  <main class="main">
    <div class="main-head">
      <span class="mh-title">{{ current?.title ?? '未选择会话' }}</span>
      <span v-if="current" class="mh-ws">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7">
          <path d="M3.5 7.5h6l1.8 2h9.2v9h-17v-11z" />
        </svg>
        {{ workspaceName }}
      </span>
      <span class="sp" />
      <button v-if="current" class="mh-btn" title="复制会话" @click="sessions.duplicate(current.id)">
        <svg viewBox="0 0 24 24"><rect x="8" y="8" width="11" height="11" rx="2" /><path d="M15 5.5H6A1.5 1.5 0 004.5 7v9" /></svg>
      </button>
      <button v-if="current" class="mh-btn" title="删除会话" @click="sessions.remove(current.id)">
        <svg viewBox="0 0 24 24"><path d="M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13" /><path d="M10.5 10v6M13.5 10v6" /></svg>
      </button>
    </div>

    <div v-if="current" ref="thread" class="thread">
      <div class="thread-inner">
        <MessageItem v-for="m in sessions.messages" :key="m.id" :message="m" />

        <div v-if="chat.streaming" class="msg-ai">
          <div class="ai-role"><span class="who">Claude</span></div>
          <div v-if="chat.streamTools.length" class="steps">
            <div v-for="(t, i) in chat.streamTools" :key="i" class="step">
              <span class="si">{{ t.name.slice(0, 1) }}</span>
              <span class="sname">{{ t.name }}</span>
              <span class="sarg">{{ t.target }}</span>
            </div>
          </div>
          <div class="ai-text">{{ chat.streamText }}<span class="caret" /></div>
        </div>

        <div v-if="chat.error" class="chat-error">{{ chat.error }}</div>
      </div>
    </div>

    <div v-else class="empty">
      <div class="eico">
        <svg viewBox="0 0 24 24"><path d="M4.5 5.5h15v10.5h-9l-6 4v-14.5z" /></svg>
      </div>
      <h2>{{ ws.list.length === 0 ? '先添加一个工作区' : '选择一个会话开始' }}</h2>
      <p v-if="ws.list.length === 0">工作区就是你的项目目录。添加后即可在里面新建会话。</p>
      <p v-else>从左侧选一个会话，或点击工作区右侧的 ＋ 新建。</p>
      <button v-if="ws.list.length === 0" class="primary-btn" @click="ws.pickAndAdd()">
        添加工作区
      </button>
    </div>

    <div v-if="current" class="composer-wrap">
      <div v-if="attachments.length" class="attach-row">
        <span v-for="(f, i) in attachments" :key="i" class="attach-chip">
          {{ f.split(/[\\/]/).pop() }}
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
            <span class="cmd-desc">{{ c.description || c.argumentHint }}</span>
          </button>
        </div>

        <textarea
          ref="textarea"
          v-model="input"
          rows="2"
          placeholder="描述你想让它做的事，或输入 / 调用命令与 Skill…"
          :disabled="chat.streaming"
          @input="onInput"
          @keydown="onKeydown"
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

          <button v-if="chat.streaming" class="send-btn stop" title="停止" @click="stop">
            <svg viewBox="0 0 24 24"><rect x="7" y="7" width="10" height="10" rx="1.5" fill="currentColor" stroke="none" /></svg>
          </button>
          <button v-else class="send-btn" :disabled="!input.trim()" title="发送" @click="send">
            <svg viewBox="0 0 24 24"><path d="M12 19V5M6 11l6-6 6 6" /></svg>
          </button>
        </div>
      </div>
      <div class="hint">Enter 发送 · Shift + Enter 换行 · / 调用命令</div>
    </div>

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
}

.main-head {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 46px;
  padding: 0 24px;
  border-bottom: 1px solid var(--line);
}

.mh-title {
  font-size: 14px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mh-ws {
  font-size: 11.5px;
  color: var(--t4);
  display: flex;
  align-items: center;
  gap: 5px;
  flex: none;
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
  padding: 26px 0 8px;
}

.thread-inner {
  max-width: 720px;
  margin: 0 auto;
  padding: 0 28px;
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

.caret {
  display: inline-block;
  width: 2px;
  height: 14px;
  background: var(--green);
  vertical-align: -2px;
  margin-left: 2px;
  animation: blink 1.1s steps(1) infinite;
}

@keyframes blink {
  50% {
    opacity: 0;
  }
}

.chat-error {
  margin-top: 12px;
  padding: 10px 13px;
  border-radius: 8px;
  background: #fdf5f5;
  border: 1px solid #f2dedd;
  color: #8a4b47;
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

.composer-wrap {
  flex: none;
  padding: 6px 28px 16px;
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
  height: 24px;
  padding: 0 8px;
  border-radius: 5px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  font-size: 11.5px;
  color: var(--t2);
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
  border-radius: var(--r-lg);
  background: #fff;
  display: flex;
  flex-direction: column;
  min-height: 96px;
  position: relative;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.composer:focus-within {
  border-color: #bfc3c7;
  box-shadow: 0 0 0 3px rgba(71, 129, 92, 0.06);
}

.cmd-panel {
  position: absolute;
  left: 0;
  right: 0;
  bottom: calc(100% + 6px);
  background: #fff;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.1);
  padding: 4px;
  max-height: 260px;
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
  padding: 13px 14px 2px;
  font-size: 13.5px;
  line-height: 21px;
  min-height: 48px;
}

.composer textarea::placeholder {
  color: var(--t4);
}

.composer-bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 4px;
  height: 38px;
  padding: 0 8px 0 9px;
}

.cb-btn {
  width: 26px;
  height: 26px;
  border-radius: 5px;
  display: grid;
  place-items: center;
  color: var(--t3);
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
  padding: 0 9px;
  border-radius: 6px;
  font-size: 11.5px;
  color: var(--t3);
  transition: background 0.12s;
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
  width: 29px;
  height: 29px;
  border-radius: 7px;
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
