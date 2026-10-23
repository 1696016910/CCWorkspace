<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { useWorkspaceStore } from '@/stores/workspace'
import { useSessionStore } from '@/stores/session'
import type { SessionSummary } from '@shared/types'

const emit = defineEmits<{ 'open-settings': []; 'open-trash': [] }>()

const ws = useWorkspaceStore()
const sessions = useSessionStore()

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
}

function cancelRename(): void {
  renamingId.value = null
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
          <span class="wname">{{ g.name }}</span>
          <span class="wcnt">{{ g.sessions.length }}</span>
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
              <span class="cn">{{ s.title }}</span>
              <span class="ctime">{{ relativeTime(s.updatedAt) }}</span>
              <button
                class="edit-btn"
                title="重命名"
                @click.stop="startRename($event, s.id, s.title)"
              >
                ✎
              </button>
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
  display: flex;
  align-items: center;
  gap: 7px;
  height: 30px;
  padding: 0 8px;
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

.wname {
  flex: 1;
  font-size: 13px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.wcnt {
  font-size: 11px;
  color: var(--t4);
  flex: none;
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

.edit-btn {
  display: none;
  width: 18px;
  height: 18px;
  border-radius: 4px;
  color: var(--t3);
  font-size: 10px;
  flex: none;
  place-items: center;
}

.chat:hover .edit-btn {
  display: grid;
}

.chat:hover .ctime {
  display: none;
}

.edit-btn:hover {
  background: #e0e0e3;
  color: var(--t1);
}

.rename-input {
  flex: 1;
  min-width: 0;
  height: 21px;
  padding: 0 5px;
  border: 1px solid var(--green);
  border-radius: 4px;
  background: #fff;
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
  background: #fdf5f5;
  border: 1px solid #f2dedd;
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
