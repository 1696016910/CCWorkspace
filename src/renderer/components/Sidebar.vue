<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useToast } from '@/composables/useToast'
import { useWorkspaceStore } from '@/stores/workspace'
import { useSessionStore } from '@/stores/session'
import { useChatStore } from '@/stores/chat'
import type { SessionSummary } from '@shared/types'

const emit = defineEmits<{ 'open-settings': []; 'open-trash': [] }>()

const ws = useWorkspaceStore()
const sessions = useSessionStore()
const chat = useChatStore()
const toast = useToast()

/**
 * 删除会话前先停掉它的生成。
 * 不停的话请求会继续跑并计费，产出的回复又因为会话已软删而被丢弃，
 * 等于白花钱；同时侧栏绿点会一直亮着。
 */
async function removeSession(id: string): Promise<void> {
  if (sessions.isRunning(id)) await chat.stop()
  await sessions.remove(id)
}

onMounted(async () => {
  theme.value = await window.api.theme.get()
  document.documentElement.dataset.theme = theme.value
})

interface Group {
  workspaceId: string
  name: string
  path: string
  sessions: SessionSummary[]
}

const groups = computed<Group[]>(() =>
  ws.list.map((w) => ({
    workspaceId: w.id,
    name: w.name,
    path: w.path,
    sessions: sessions.sessionsOf(w.id)
  }))
)

const hasWorkspace = computed(() => ws.list.length > 0)

async function newChat(): Promise<void> {
  if (!hasWorkspace.value) {
    await ws.pickAndAdd()
    return
  }
  await sessions.create(ws.current?.id ?? null)
}

/** 在工作区名称右侧直接新建会话（Qoder 式） */
async function newChatIn(workspaceId: string): Promise<void> {
  const created = await sessions.create(workspaceId)
  if (created) toast.success('已新建会话')
}

/* ── 主题切换 ── */
const theme = ref<'light' | 'dark'>('light')

async function toggleTheme(): Promise<void> {
  theme.value = theme.value === 'light' ? 'dark' : 'light'
  document.documentElement.dataset.theme = theme.value
  await window.api.theme.set(theme.value)
}

/* ── 工作区重命名 ── */
const renamingWsId = ref<string | null>(null)
const wsRenameDraft = ref('')

function startWsRename(id: string, name: string): void {
  renamingWsId.value = id
  wsRenameDraft.value = name
  void nextTick(() => {
    const el = document.querySelector<HTMLInputElement>('.ws-rename')
    el?.focus()
    el?.select()
  })
}

async function commitWsRename(id: string): Promise<void> {
  if (renamingWsId.value !== id) return
  const name = wsRenameDraft.value.trim()
  renamingWsId.value = null
  if (name) {
    await ws.rename(id, name)
    toast.success('工作区已重命名')
  }
}

/* ── 重命名：Electron 不支持 window.prompt，改用内联输入 ── */
const renamingId = ref<string | null>(null)
const renameDraft = ref('')
const renameInput = ref<HTMLInputElement | null>(null)

function startRename(e: Event, id: string, currentTitle: string): void {
  e.preventDefault()
  e.stopPropagation()
  renamingId.value = id
  renameDraft.value = currentTitle
  void nextTick(() => {
    renameInput.value?.focus()
    renameInput.value?.select()
  })
}

async function commitRename(): Promise<void> {
  const id = renamingId.value
  if (!id) return
  const trimmed = renameDraft.value.trim()
  renamingId.value = null
  if (!trimmed) return
  await sessions.rename(id, trimmed)
  await sessions.select(id)
  toast.success('已重命名')
}

function cancelRename(): void {
  renamingId.value = null
}

/** 未命名的会话按时间顺序编号，避免列表里出现多个同名条目 */
function displayTitle(list: SessionSummary[], s: SessionSummary): string {
  if (s.title !== '(新会话)') return s.title
  const unnamed = list.filter((x) => x.title === '(新会话)')
  if (unnamed.length <= 1) return s.title
  const idx = unnamed.findIndex((x) => x.id === s.id)
  return `新会话 ${idx + 1}`
}

/** 只保留路径的最后两级，避免侧栏被长路径撑破 */
function shortPath(full: string): string {
  const parts = full.split(/[\/]/).filter(Boolean)
  if (parts.length <= 2) return full
  const tail = parts.slice(-2).join('/')
  const isWin = /^[A-Za-z]:/.test(full)
  return `${isWin ? parts[0] + ':/' : '/'}…/${tail}`
}

function relativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const min = Math.floor(diff / 60000)
  if (min < 1) return '刚刚'
  if (min < 60) return `${min} 分钟前`
  const hour = Math.floor(min / 60)
  if (hour < 24) return `${hour} 小时前`
  const day = Math.floor(hour / 24)
  return `${day} 天前`
}
</script>

<template>
  <aside class="sidebar">
    <button class="new-chat" @click="newChat">
      <svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
      {{ hasWorkspace ? '新建会话' : '添加工作区' }}
    </button>

    <div class="side-scroll">
      <div class="grp-label">
        <span>工作区</span>
        <span class="sp" />
        <button class="mini-add" title="添加工作区" @click="ws.pickAndAdd()">＋</button>
      </div>

      <div v-for="g in groups" :key="g.workspaceId" class="ws">
        <div class="ws-head" @click="ws.toggle(g.workspaceId)">
          <span class="tri" :class="{ closed: ws.isCollapsed(g.workspaceId) }">▼</span>
          <span class="fico">
            <svg viewBox="0 0 24 24"><path d="M3.5 7.5h6l1.8 2h9.2v9h-17v-11z" /></svg>
          </span>
          <span class="wtext">
            <input
              v-if="renamingWsId === g.workspaceId"
              ref="wsRenameInput"
              v-model="wsRenameDraft"
              class="ws-rename"
              @click.stop
              @keydown.enter.prevent="commitWsRename(g.workspaceId)"
              @keydown.esc.prevent="renamingWsId = null"
              @blur="commitWsRename(g.workspaceId)"
            />
            <template v-else>
              <span class="wname">{{ g.name }}</span>
              <span class="wpath" :title="g.path">{{ shortPath(g.path) }}</span>
            </template>
          </span>
          <span class="wcnt">{{ g.sessions.length }}</span>
          <span class="ws-actions">
            <button
              class="ws-add"
              title="重命名工作区"
              @click.stop="startWsRename(g.workspaceId, g.name)"
            >
              ✎
            </button>
            <button
              class="ws-add"
              title="在此工作区新建会话"
              @click.stop="newChatIn(g.workspaceId)"
            >
              ＋
            </button>
          </span>
        </div>

        <div v-show="!ws.isCollapsed(g.workspaceId)" class="ws-body">
          <div
            v-for="s in g.sessions"
            :key="s.id"
            class="chat"
            :class="{ on: sessions.currentId === s.id }"
            @click="sessions.select(s.id)"
            @contextmenu="startRename($event, s.id, s.title)"
          >
            <input
              v-if="renamingId === s.id"
              ref="renameInput"
              v-model="renameDraft"
              class="rename-input"
              @click.stop
              @keydown.enter.prevent="commitRename"
              @keydown.esc.prevent="cancelRename"
              @blur="commitRename"
            />
            <template v-else>
              <span class="cn">{{ displayTitle(g.sessions, s) }}</span>
              <span class="ctime">{{ relativeTime(s.updatedAt) }}</span>
              <span class="chat-actions">
                <button
                  class="edit-btn"
                  title="重命名"
                  @click.stop="startRename($event, s.id, s.title)"
                >
                  ✎
                </button>
                <button
                  class="edit-btn"
                  title="删除会话"
                  @click.stop="removeSession(s.id)"
                >
                  ✕
                </button>
              </span>
              <span class="cd" :class="sessions.isRunning(s.id) ? 'live' : 'idle'" />
            </template>
          </div>
          <p v-if="g.sessions.length === 0" class="ws-empty">暂无会话</p>
        </div>
      </div>

      <p v-if="groups.length === 0 && !ws.loading" class="side-empty">
        还没有工作区。点击「添加工作区」选择一个项目目录。
      </p>
      <p v-if="ws.error" class="side-error">{{ ws.error }}</p>
    </div>

    <div class="side-foot">
      <button class="foot-btn" @click="toggleTheme">
        <svg v-if="theme === 'light'" viewBox="0 0 24 24">
          <path d="M20 14.5A8.5 8.5 0 019.5 4 8.5 8.5 0 1020 14.5z" />
        </svg>
        <svg v-else viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.2 5.2l1.4 1.4M17.4 17.4l1.4 1.4M18.8 5.2l-1.4 1.4M6.6 17.4l-1.4 1.4" />
        </svg>
        {{ theme === 'light' ? '暗色模式' : '浅色模式' }}
      </button>
      <button class="foot-btn" @click="emit('open-trash')">
        <svg viewBox="0 0 24 24">
          <path d="M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13" />
        </svg>
        回收站
      </button>
      <button class="foot-btn" @click="emit('open-settings')">
        <svg viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="2.6" />
          <path d="M12 3.5v2M12 18.5v2M3.5 12h2M18.5 12h2M5.8 5.8l1.6 1.6M16.6 16.6l1.6 1.6M18.2 5.8l-1.6 1.6M7.4 16.6l-1.6 1.6" />
        </svg>
        设置
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--bg-bar);
  border-right: 1px solid var(--line);
  padding: 8px 8px 6px;
}

.new-chat {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  height: 32px;
  border-radius: var(--r);
  background: var(--green);
  color: #fff;
  font-size: 13px;
  font-weight: 500;
  margin-bottom: 10px;
  transition: background 0.12s;
}

.new-chat:hover {
  background: var(--green-h);
}

.new-chat svg {
  width: 14px;
  height: 14px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
}

.side-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scrollbar-width: thin;
}

.grp-label {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 9px 8px 5px;
  font-size: 11.5px;
  color: var(--t3);
}

.grp-label .sp {
  flex: 1;
}

.mini-add {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  display: grid;
  place-items: center;
  color: var(--t3);
  font-size: 13px;
  line-height: 1;
}

.mini-add:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.ws {
  margin-bottom: 2px;
}

.ws-head {
  position: relative;
  display: flex;
  align-items: center;
  gap: 7px;
  min-height: 34px;
  padding: 3px 8px;
  border-radius: var(--r);
  cursor: pointer;
  transition: background 0.12s;
}

.ws-head:hover {
  background: var(--bg-hover);
}

.tri {
  width: 10px;
  font-size: 8px;
  color: var(--t4);
  transition: transform 0.15s;
  flex: none;
  text-align: center;
}

.tri.closed {
  transform: rotate(-90deg);
}

.fico {
  display: flex;
  color: var(--t3);
  flex: none;
}

.fico svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.wtext {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  line-height: 1.3;
  overflow: hidden;
}

.wname {
  flex: none;
  font-size: 13px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.wpath {
  font-family: var(--mono);
  font-size: 9.5px;
  color: var(--t4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  /* 长路径优先保留尾部（项目名），用 dir=ltr + 反向省略不可行，
     这里直接截断显示，title 提供完整路径 */
  display: block;
}

.ws-rename {
  width: 100%;
  height: 20px;
  padding: 0 4px;
  border: 1px solid var(--green);
  border-radius: 4px;
  background: var(--surface);
  font-size: 12.5px;
  color: var(--t1);
}

.wcnt {
  font-size: 11px;
  color: var(--t4);
  flex: none;
}

.ws-actions {
  position: absolute;
  right: 6px;
  top: 50%;
  transform: translateY(-50%);
  display: none;
  gap: 2px;
  padding: 1px 2px 1px 6px;
  border-radius: 4px;
  background: var(--bg-hover);
  box-shadow: -6px 0 8px var(--bg-hover);
}

.ws-head:hover .ws-actions {
  display: flex;
}

.ws-add {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  color: var(--t3);
  font-size: 12px;
  line-height: 1;
  flex: none;
  display: grid;
  place-items: center;
}

.ws-add:hover {
  background: var(--bg-select);
  color: var(--t1);
}

.ws-body {
  position: relative;
  padding-left: 38px;
}

.ws-body::before {
  content: '';
  position: absolute;
  left: 32px;
  top: 0;
  bottom: 8px;
  width: 1px;
  background: #e4e4e6;
}

.chat {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 29px;
  padding: 4px 9px;
  margin: 1px 0;
  border-radius: var(--r);
  font-size: 12.5px;
  color: var(--t2);
  text-align: left;
  width: 100%;
  transition: background 0.12s, color 0.12s;
  position: relative;
}

.chat:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.chat.on {
  background: var(--bg-select);
  color: var(--t1);
  font-weight: 500;
}

.cn {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ctime {
  font-size: 10.5px;
  color: var(--t4);
  flex: none;
}

.chat-actions {
  display: none;
  gap: 1px;
  flex: none;
}

.chat:hover .chat-actions {
  display: flex;
}

.chat:hover .ctime {
  display: none;
}

.edit-btn {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  color: var(--t3);
  font-size: 10px;
  display: grid;
  place-items: center;
}

.edit-btn:hover {
  background: var(--bg-select);
  color: var(--t1);
}

/* 流式中显示绿点，否则显示时间 */
.cd.live {
  animation: pulse-dot 1.6s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%,
  100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.45;
    transform: scale(0.8);
  }
}

.rename-input {
  flex: 1;
  min-width: 0;
  height: 21px;
  padding: 0 5px;
  border: 1px solid var(--green);
  border-radius: 4px;
  background: var(--surface);
  font-size: 12.5px;
  color: var(--t1);
}

.cd {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  flex: none;
}

.cd.live {
  background: var(--green);
}

.cd.idle {
  background: #c6c6cb;
}

.ws-empty {
  padding: 6px 9px;
  font-size: 11.5px;
  color: var(--t4);
}

.side-empty {
  padding: 20px 10px;
  font-size: 12px;
  color: var(--t4);
  line-height: 1.7;
}

.side-error {
  margin: 10px 8px 0;
  padding: 8px 10px;
  border-radius: var(--r);
  background: var(--danger-bg);
  border: 1px solid var(--danger-line);
  color: #8a4b47;
  font-size: 11.5px;
  line-height: 1.6;
}

.side-foot {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding: 8px 4px 2px;
  border-top: 1px solid var(--line);
  margin-top: 6px;
}

.foot-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 9px;
  border-radius: var(--r);
  font-size: 12.5px;
  color: var(--t2);
  width: 100%;
  transition: background 0.12s, color 0.12s;
}

.foot-btn:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.foot-btn svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}
</style>
