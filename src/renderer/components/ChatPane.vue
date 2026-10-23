<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useSessionStore } from '@/stores/session'
import { useWorkspaceStore } from '@/stores/workspace'
import { useChatStore } from '@/stores/chat'
import type { ChatMessage } from '@shared/types'
import MessageItem from './MessageItem.vue'

const sessions = useSessionStore()
const ws = useWorkspaceStore()
const chat = useChatStore()

const input = ref('')
const thread = ref<HTMLElement | null>(null)

const current = computed(() => sessions.current)

const workspaceName = computed(() => {
  const s = current.value
  if (!s) return ''
  return ws.list.find((w) => w.id === s.workspaceId)?.name ?? ''
})

const ctxPercent = computed(() => {
  const tokens = chat.usage.inputTokens
  if (!tokens) return 0
  // 以 200k 作为窗口参考值
  return Math.min(100, Math.round((tokens / 200_000) * 100))
})

async function scrollToBottom(): Promise<void> {
  await nextTick()
  thread.value?.scrollTo({ top: thread.value.scrollHeight, behavior: 'smooth' })
}

function onMessageSaved(msg: ChatMessage): void {
  sessions.messages = [...sessions.messages, msg]
  // 首条消息会补自动标题、并刷新 updated_at，因此需要重载列表
  void sessions.load()
  void scrollToBottom()
}

onMounted(() => {
  chat.listen(onMessageSaved)
})

watch(
  () => sessions.currentId,
  async () => {
    await nextTick()
    scrollToBottom()
  }
)

watch(
  () => chat.streamText,
  () => {
    void scrollToBottom()
  }
)

async function send(): Promise<void> {
  const text = input.value.trim()
  if (!text || !current.value) return
  input.value = ''

  // 先把用户消息落到本地列表，服务端回传 done 时只追加助手消息
  const optimistic: ChatMessage = {
    id: `local-${Date.now()}`,
    role: 'user',
    text,
    timestamp: new Date().toISOString()
  }
  sessions.messages = [...sessions.messages, optimistic]
  void scrollToBottom()

  await chat.send(current.value.id, text)
}

function autoGrow(e: Event): void {
  const el = e.target as HTMLTextAreaElement
  el.style.height = 'auto'
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`
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
        <MessageItem
          v-for="m in sessions.messages"
          :key="m.id"
          :message="m"
        />

        <!-- 流式中的回复 -->
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
      <p v-else>从左侧选一个会话，或点击「新建会话」开始与 Claude 对话。</p>
      <button v-if="ws.list.length === 0" class="primary-btn" @click="ws.pickAndAdd()">
        添加工作区
      </button>
    </div>

    <div v-if="current" class="composer-wrap">
      <div class="composer">
        <textarea
          v-model="input"
          rows="2"
          placeholder="描述你想让它做的事…"
          :disabled="chat.streaming"
          @input="autoGrow"
          @keydown.enter.exact.prevent="send"
        />
        <div class="composer-bar">
          <button class="cb-btn" title="添加文件">
            <svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
          </button>
          <span class="sp" />
          <div class="ctx" :title="`上下文用量 ${ctxPercent}%`">
            <span class="ctx-bar"><i :style="{ width: `${ctxPercent}%` }" /></span>
            <span class="ctx-num">{{ chat.usage.inputTokens ? `${Math.round(chat.usage.inputTokens / 1000)}k` : '—' }}</span>
          </div>
          <button v-if="chat.streaming" class="send-btn stop" title="停止" @click="stop">
            <svg viewBox="0 0 24 24"><rect x="7" y="7" width="10" height="10" rx="1.5" fill="currentColor" stroke="none" /></svg>
          </button>
          <button v-else class="send-btn" :disabled="!input.trim()" title="发送" @click="send">
            <svg viewBox="0 0 24 24"><path d="M12 19V5M6 11l6-6 6 6" /></svg>
          </button>
        </div>
      </div>
      <div class="hint">Enter 发送 · Shift + Enter 换行</div>
    </div>
  </main>
</template>

<style scoped>
.main {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
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
  color: var(--t2);
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

.composer {
  max-width: 720px;
  margin: 0 auto;
  border: 1px solid var(--line-2);
  border-radius: var(--r-lg);
  background: #fff;
  display: flex;
  flex-direction: column;
  min-height: 96px;
  transition: border-color 0.15s, box-shadow 0.15s;
}

.composer:focus-within {
  border-color: #bfc3c7;
  box-shadow: 0 0 0 3px rgba(71, 129, 92, 0.06);
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
  gap: 7px;
  height: 26px;
  padding: 0 8px;
  border-radius: 5px;
  font-size: 11.5px;
  color: var(--t3);
}

.ctx-bar {
  width: 46px;
  height: 4px;
  border-radius: 2px;
  background: #e7e7e9;
  overflow: hidden;
  flex: none;
}

.ctx-bar i {
  display: block;
  height: 100%;
  background: var(--green);
  border-radius: 2px;
  transition: width 0.3s;
}

.ctx-num {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--t2);
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
