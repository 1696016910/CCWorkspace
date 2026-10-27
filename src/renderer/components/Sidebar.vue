<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useToast } from '@/composables/useToast'
import { useWorkspaceStore } from '@/stores/workspace'
import { useSessionStore } from '@/stores/session'
import { useChatStore } from '@/stores/chat'
import ConfirmDialog from './ConfirmDialog.vue'
import NewWorkspaceDialog from './NewWorkspaceDialog.vue'
import type { SessionSummary } from '@shared/types'

const emit = defineEmits<{ 'open-settings': []; 'open-trash': [] }>()

const ws = useWorkspaceStore()
const sessions = useSessionStore()
const chat = useChatStore()
const toast = useToast()

/* ── 弹窗状态 ── */
const showNewWs = ref(false)
/** 待确认的删除动作：只有点了确认才真正执行 */
const pending = ref<
  | { kind: 'session'; id: string; title: string }
  | { kind: 'workspace'; id: string; name: string; sessionCount: number; deletedCount: number }
  | null
>(null)

/* ── 下拉菜单（挂在 body 上，避免被 overflow 裁掉） ── */
type MenuItem = {
  label: string
  icon: string
  danger?: boolean
  action: () => void | Promise<void>
}
const menu = ref<{ x: number; y: number; items: MenuItem[] } | null>(null)

function openMenu(e: MouseEvent, items: MenuItem[]): void {
  e.stopPropagation()
  e.preventDefault()
  const rect = (e.currentTarget as HTMLElement).getBoundingClientRect()
  menu.value = { x: rect.right, y: rect.bottom + 4, items }
}

function closeMenu(): void {
  menu.value = null
}

async function runMenu(item: MenuItem): Promise<void> {
  closeMenu()
  await item.action()
}

onMounted(async () => {
  theme.value = await window.api.theme.get()
  document.documentElement.dataset.theme = theme.value
  window.addEventListener('click', closeMenu)
  window.addEventListener('keydown', onGlobalKey)
})

onUnmounted(() => {
  window.removeEventListener('click', closeMenu)
  window.removeEventListener('keydown', onGlobalKey)
})

function onGlobalKey(e: KeyboardEvent): void {
  if (e.key === 'Escape' && menu.value) closeMenu()
}

interface Group {
  workspaceId: string
  name: string
  path: string
  pinned: boolean
  sessions: SessionSummary[]
}

const groups = computed<Group[]>(() =>
  ws.list.map((w) => ({
    workspaceId: w.id,
    name: w.name,
    path: w.path,
    pinned: w.pinnedAt !== null,
    sessions: sessions.sessionsOf(w.id)
  }))
)

/* ── 新建会话 ── */
async function newChatIn(workspaceId: string): Promise<void> {
  const created = await sessions.create(workspaceId)
  if (created) toast.success('已新建会话')
}

/* ── 删除会话 / 工作区（先弹确认） ── */
function askRemoveSession(s: SessionSummary): void {
  pending.value = { kind: 'session', id: s.id, title: displayTitle(sessions.list, s) }
}

async function confirmPending(): Promise<void> {
  const p = pending.value
  pending.value = null
  if (!p) return

  if (p.kind === 'session') {
    if (sessions.isRunning(p.id)) await chat.stop()
    await sessions.remove(p.id)
    toast.info?.('已删除 1 个会话')
  } else {
    await ws.removeConfirmed(p.id)
    toast.info?.('已删除 1 个工作区')
  }
}

function askRemoveWorkspace(g: Group): void {
  const deleted = deletedCountOf(g.workspaceId)
  pending.value = {
    kind: 'workspace',
    id: g.workspaceId,
    name: g.name,
    sessionCount: g.sessions.length,
    deletedCount: deleted
  }
}

/** 回收站里属于该工作区的会话数（也会被级联删除） */
const deletedByWorkspace = ref<Record<string, number>>({})

function deletedCountOf(workspaceId: string): number {
  return deletedByWorkspace.value[workspaceId] ?? 0
}

async function loadDeletedCounts(): Promise<void> {
  try {
    const list = await window.api.session.listDeleted()
    const map: Record<string, number> = {}
    for (const s of list) map[s.workspaceId] = (map[s.workspaceId] ?? 0) + 1
    deletedByWorkspace.value = map
  } catch {
    // 回收站读取失败不影响主流程
  }
}

/* ── 重命名 ── */
const renamingWsId = ref<string | null>(null)
const wsRenameDraft = ref('')
const renamingId = ref<string | null>(null)
const renameDraft = ref('')
const renameInput = ref<HTMLInputElement | null>(null)

function startWsRename(id: string, name: string): void {
  renamingWsId.value = id
  wsRenameDraft.value = name
  void nextTick(() => {
    document.querySelector<HTMLInputElement>('.ws-rename')?.select()
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

/* ── 详情 ── */
const detail = ref<{ title: string; rows: [string, string][] } | null>(null)

function showWorkspaceDetail(g: Group): void {
  const w = ws.list.find((x) => x.id === g.workspaceId)
  if (!w) return
  detail.value = {
    title: `工作区 · ${w.name}`,
    rows: [
      ['路径', w.path],
      ['会话数', `${g.sessions.length} 个`],
      ['回收站', `${deletedCountOf(g.workspaceId)} 个`],
      ['添加时间', formatDate(w.addedAt)]
    ]
  }
}

function showSessionDetail(s: SessionSummary): void {
  detail.value = {
    title: `会话 · ${displayTitle(sessions.list, s)}`,
    rows: [
      ['消息数', `${s.messageCount} 条`],
      ['上下文', `${Math.round(s.contextPercent)}%`],
      ['更新时间', formatDate(s.updatedAt)],
      ['置顶', s.pinnedAt ? '是' : '否']
    ]
  }
}

function formatDate(iso: string): string {
  const d = new Date(iso)
  const p = (n: number): string => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`
}

/* ── 菜单构造 ── */
function workspaceMenu(g: Group): MenuItem[] {
  return [
    {
      label: '重命名',
      icon: 'edit',
      action: () => startWsRename(g.workspaceId, g.name)
    },
    { label: '详情', icon: 'info', action: () => showWorkspaceDetail(g) },
    {
      label: g.pinned ? '取消置顶' : '置顶',
      icon: 'pin',
      action: () => ws.pin(g.workspaceId, !g.pinned)
    },
    {
      label: '删除工作区',
      icon: 'trash',
      danger: true,
      action: () => askRemoveWorkspace(g)
    }
  ]
}

function sessionMenu(s: SessionSummary): MenuItem[] {
  const title = displayTitle(sessions.list, s)
  const pinned = s.pinnedAt !== null
  return [
    {
      label: '重命名',
      icon: 'edit',
      action: () => startRename(new MouseEvent('click'), s.id, s.title)
    },
    {
      label: pinned ? '取消置顶' : '置顶',
      icon: 'pin',
      action: () => sessions.pin(s.id, !pinned)
    },
    {
      label: '复制会话',
      icon: 'copy',
      action: async () => {
        await sessions.duplicate(s.id)
        toast.success('已复制会话')
      }
    },
    { label: '详情', icon: 'info', action: () => showSessionDetail(s) },
    {
      label: `删除「${title}」`,
      icon: 'trash',
      danger: true,
      action: () => askRemoveSession(s)
    }
  ]
}

/* ── 主题 ── */
const theme = ref<'light' | 'dark'>('light')

async function toggleTheme(): Promise<void> {
  theme.value = theme.value === 'light' ? 'dark' : 'light'
  document.documentElement.dataset.theme = theme.value
  await window.api.theme.set(theme.value)
}

/** 未命名的会话按时间顺序编号，避免列表里出现多个同名条目 */
function displayTitle(list: SessionSummary[], s: SessionSummary): string {
  if (s.title !== '(新会话)') return s.title
  const unnamed = list.filter((x) => x.title === '(新会话)')
  if (unnamed.length <= 1) return s.title
  const idx = unnamed.findIndex((x) => x.id === s.id)
  return `新会话 ${idx + 1}`
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

/* ── 图标路径 ── */
const ICONS: Record<string, string> = {
  edit: 'M4 20h4l10-10-4-4L4 16v4zM13.5 6.5l4 4',
  info: 'M12 11v5M12 8v.01',
  pin: 'M12 19V5M5 12l7-7 7 7',
  copy: 'M8 8h11v11H8zM15 5.5H6A1.5 1.5 0 004.5 7v9',
  trash: 'M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13'
}
</script>

<template>
  <aside class="sidebar">
    <!-- 顶部：新建工作区 -->
    <div class="side-top">
      <button class="btn-new-ws" @click="showNewWs = true">
        <svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
        新建工作区
      </button>
    </div>

    <!-- 工作区与会话 -->
    <div class="side-list">
      <div v-for="g in groups" :key="g.workspaceId" class="ws">
        <div
          class="ws-head"
          role="button"
          tabindex="0"
          :title="g.path"
          @click="ws.toggle(g.workspaceId)"
          @keydown.enter.prevent="ws.toggle(g.workspaceId)"
          @keydown.space.prevent="ws.toggle(g.workspaceId)"
        >
          <span class="ws-tri" :class="{ closed: ws.isCollapsed(g.workspaceId) }">
            <svg viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" /></svg>
          </span>
          <span class="ws-ico">
            <svg viewBox="0 0 24 24"><path d="M3.5 7.5h6l1.8 2h9.2v9h-17v-11z" /></svg>
          </span>
          <input
            v-if="renamingWsId === g.workspaceId"
            v-model="wsRenameDraft"
            class="ws-rename"
            @click.stop
            @keydown.enter.prevent="commitWsRename(g.workspaceId)"
            @keydown.esc.prevent="renamingWsId = null"
            @blur="commitWsRename(g.workspaceId)"
          />
          <span v-else class="ws-name">
            <span v-if="g.pinned" class="pin-dot" title="已置顶">•</span>{{ g.name }}
          </span>
          <span class="ws-count">{{ g.sessions.length }}</span>
          <span class="ws-btns">
            <button class="row-btn add" title="新建会话" @click.stop="newChatIn(g.workspaceId)">
              <svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
            </button>
            <button class="row-btn more" title="更多" @click.stop="openMenu($event, workspaceMenu(g))">
              <svg viewBox="0 0 24 24">
                <circle cx="12" cy="5" r="1.8" />
                <circle cx="12" cy="12" r="1.8" />
                <circle cx="12" cy="19" r="1.8" />
              </svg>
            </button>
          </span>
        </div>

        <div v-show="!ws.isCollapsed(g.workspaceId)" class="ws-body">
          <div
            v-for="s in g.sessions"
            :key="s.id"
            class="chat-item"
            :class="{ on: sessions.currentId === s.id }"
            role="button"
            tabindex="0"
            @click="sessions.select(s.id)"
            @keydown.enter.prevent="sessions.select(s.id)"
            @keydown.space.prevent="sessions.select(s.id)"
            @contextmenu="startRename($event, s.id, s.title)"
          >
            <input
              v-if="renamingId === s.id"
              ref="renameInput"
              v-model="renameDraft"
              class="rename-input"
              @click.stop
              @keydown.enter.prevent="commitRename"
              @keydown.esc.prevent="renamingId = null"
              @blur="commitRename"
            />
            <template v-else>
              <span class="chat-title">
                <span v-if="s.pinnedAt" class="pin-dot" title="已置顶">•</span>{{
                  displayTitle(g.sessions, s)
                }}
              </span>
              <span class="chat-time">{{ relativeTime(s.updatedAt) }}</span>
              <button class="row-btn more" title="更多" @click.stop="openMenu($event, sessionMenu(s))">
                <svg viewBox="0 0 24 24">
                  <circle cx="12" cy="5" r="1.8" />
                  <circle cx="12" cy="12" r="1.8" />
                  <circle cx="12" cy="19" r="1.8" />
                </svg>
              </button>
              <span class="chat-dot" :class="sessions.isRunning(s.id) ? 'live' : 'idle'" />
            </template>
          </div>
          <p v-if="g.sessions.length === 0" class="ws-empty">暂无会话，点上方 ＋ 新建</p>
        </div>
      </div>

      <p v-if="groups.length === 0 && !ws.loading" class="side-empty">
        还没有工作区。点上方「新建工作区」选择一个项目目录。
      </p>
    </div>

    <!-- 底部：主题 / 回收站 / 设置 -->
    <div class="side-foot">
      <button class="foot-btn" @click="toggleTheme">
        <svg v-if="theme === 'light'" viewBox="0 0 24 24">
          <path d="M20 14.5A8.5 8.5 0 019.5 4 8.5 8.5 0 1020 14.5z" />
        </svg>
        <svg v-else viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="4" />
          <path
            d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.2 5.2l1.4 1.4M17.4 17.4l1.4 1.4M18.8 5.2l-1.4 1.4M6.6 17.4l-1.4 1.4"
          />
        </svg>
        {{ theme === 'light' ? '暗色模式' : '浅色模式' }}
      </button>
      <button class="foot-btn" @click="emit('open-trash')">
        <svg viewBox="0 0 24 24">
          <path d="M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13" />
        </svg>
        回收站
        <span v-if="Object.keys(deletedByWorkspace).length" class="fb-badge">
          {{ Object.values(deletedByWorkspace).reduce((a, b) => a + b, 0) }}
        </span>
      </button>
      <button class="foot-btn" @click="emit('open-settings')">
        <svg viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="2.6" />
          <path
            d="M12 3.5v2M12 18.5v2M3.5 12h2M18.5 12h2M5.8 5.8l1.6 1.6M16.6 16.6l1.6 1.6M18.2 5.8l-1.6 1.6M7.4 16.6l-1.6 1.6"
          />
        </svg>
        设置
      </button>
    </div>

    <!-- 下拉菜单 -->
    <Teleport to="body">
      <div v-if="menu" class="menu-pop" :style="{ left: menu.x + 'px', top: menu.y + 'px' }" @click.stop>
        <button
          v-for="item in menu.items"
          :key="item.label"
          class="menu-item"
          :class="{ danger: item.danger }"
          @click="runMenu(item)"
        >
          <svg viewBox="0 0 24 24"><path :d="ICONS[item.icon]" /></svg>
          {{ item.label }}
        </button>
      </div>
    </Teleport>

    <!-- 详情弹窗 -->
    <Teleport to="body">
      <div v-if="detail" class="mask" @click.self="detail = null">
        <div class="detail-box">
          <div class="detail-title">{{ detail.title }}</div>
          <div class="detail-body">
            <div v-for="[k, v] in detail.rows" :key="k" class="detail-row">
              <span class="dk">{{ k }}</span>
              <span class="dv">{{ v }}</span>
            </div>
          </div>
          <div class="detail-foot">
            <button class="btn" @click="detail = null">关闭</button>
          </div>
        </div>
      </div>
    </Teleport>

    <!-- 确认弹窗 -->
    <ConfirmDialog
      v-if="pending?.kind === 'session'"
      :title="`删除会话「${pending.title}」？`"
      message="会话将移到回收站，可以随时恢复。若该会话正在生成回复，会先停止。"
      confirm-text="移到回收站"
      tone="warn"
      @confirm="confirmPending"
      @cancel="pending = null"
    />
    <ConfirmDialog
      v-else-if="pending?.kind === 'workspace'"
      :title="`删除工作区「${pending.name}」？`"
      :message="
        pending.sessionCount > 0
          ? `将同时删除该工作区下的 ${pending.sessionCount} 个会话及其全部消息。磁盘上的项目目录不会被删除。`
          : '磁盘上的项目目录不会被删除。'
      "
      :warning="
        pending.deletedCount > 0
          ? `此外，回收站里还有 ${pending.deletedCount} 个属于该工作区的会话会被一并永久删除。`
          : undefined
      "
      confirm-text="删除"
      tone="danger"
      @confirm="confirmPending"
      @cancel="pending = null"
    />

    <!-- 新建工作区弹窗 -->
    <NewWorkspaceDialog
      v-if="showNewWs"
      @close="showNewWs = false"
      @created="loadDeletedCounts()"
    />
  </aside>
</template>

<style scoped>
.sidebar {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--bg-bar);
  border-right: 1px solid var(--line);
}

.side-top {
  flex: none;
  padding: 12px;
}

.btn-new-ws {
  width: 100%;
  height: 36px;
  border-radius: 8px;
  background: var(--green);
  color: #fff;
  border: none;
  font-family: var(--font);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  transition: background 0.15s;
}

.btn-new-ws:hover {
  background: var(--green-h);
}

.btn-new-ws svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
}

.side-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding-bottom: 8px;
}

.ws {
  margin-bottom: 6px;
}

.ws-head {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 7px 8px;
  margin: 0 8px;
  border-radius: 7px;
  cursor: pointer;
  transition: background 0.12s;
}

.ws-head:hover {
  background: var(--bg-hover);
}

.ws-tri {
  width: 14px;
  height: 14px;
  flex: none;
  display: grid;
  place-items: center;
  color: var(--t4);
  transition: transform 0.15s;
}

.ws-tri svg {
  width: 10px;
  height: 10px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2.4;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.ws-tri.closed {
  transform: rotate(-90deg);
}

.ws-ico {
  width: 15px;
  height: 15px;
  flex: none;
  display: grid;
  place-items: center;
  color: var(--t3);
}

.ws-ico svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
}

.ws-name {
  flex: 1;
  min-width: 0;
  font-size: 13px;
  font-weight: 500;
  line-height: 1;
  color: var(--t1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ws-rename {
  flex: 1;
  min-width: 0;
  height: 22px;
  padding: 0 6px;
  border: 1px solid var(--green);
  border-radius: 5px;
  background: var(--surface);
  font-family: var(--font);
  font-size: 13px;
  color: var(--t1);
  outline: none;
}

.ws-count {
  font-size: 11px;
  line-height: 16px;
  color: var(--t4);
  flex: none;
  background: var(--bg-soft);
  border-radius: 8px;
  padding: 0 6px;
}

.pin-dot {
  color: var(--green);
  margin-right: 3px;
}

.ws-btns {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: none;
}

.row-btn {
  width: 24px;
  height: 24px;
  border: none;
  background: none;
  border-radius: 5px;
  color: var(--t3);
  cursor: pointer;
  display: grid;
  place-items: center;
  opacity: 0;
  transition: opacity 0.12s, background 0.12s, color 0.12s;
}

.row-btn svg {
  width: 15px;
  height: 15px;
}

.row-btn.add {
  color: var(--green);
}

.row-btn.add svg {
  stroke: currentColor;
  fill: none;
  stroke-width: 2.2;
  stroke-linecap: round;
}

.row-btn.more svg {
  fill: currentColor;
}

.ws-head:hover .row-btn,
.chat-item:hover .row-btn,
.chat-item.on .row-btn {
  opacity: 1;
}

.row-btn.add:hover {
  background: var(--green-soft);
}

.row-btn.more:hover {
  background: var(--line);
  color: var(--t1);
}

.ws-body {
  padding-top: 1px;
}

.chat-item {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 1px 8px 1px 26px;
  padding: 7px 8px;
  border-radius: 7px;
  cursor: pointer;
  transition: background 0.12s;
}

.chat-item:hover {
  background: var(--bg-hover);
}

.chat-item.on {
  background: var(--green-soft);
}

.chat-item.on .chat-title {
  color: var(--green);
  font-weight: 500;
}

.chat-title {
  flex: 1;
  min-width: 0;
  font-size: 12.5px;
  color: var(--t1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rename-input {
  flex: 1;
  min-width: 0;
  height: 22px;
  padding: 0 6px;
  border: 1px solid var(--green);
  border-radius: 5px;
  background: var(--surface);
  font-family: var(--font);
  font-size: 12.5px;
  color: var(--t1);
  outline: none;
}

.chat-time {
  font-size: 10.5px;
  color: var(--t4);
  flex: none;
}

.chat-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex: none;
}

.chat-dot.live {
  background: var(--green);
  animation: pulse 1.2s infinite;
}

.chat-dot.idle {
  background: var(--line-3);
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.3;
  }
}

.ws-empty,
.side-empty {
  font-size: 12px;
  color: var(--t4);
  padding: 10px 20px;
  line-height: 1.6;
}

.ws-empty {
  padding-left: 26px;
}

/* 底部 */
.side-foot {
  flex: none;
  margin-top: auto;
  padding: 8px;
  border-top: 1px solid var(--line);
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.foot-btn {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  height: 34px;
  padding: 0 9px;
  border: none;
  background: none;
  border-radius: 7px;
  font-family: var(--font);
  font-size: 12.5px;
  color: var(--t2);
  cursor: pointer;
  text-align: left;
  transition: background 0.12s, color 0.12s;
}

.foot-btn:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.foot-btn svg {
  width: 15px;
  height: 15px;
  flex: none;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.7;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.fb-badge {
  margin-left: auto;
  font-size: 10.5px;
  line-height: 16px;
  color: var(--t4);
  background: var(--bg-soft);
  border-radius: 8px;
  padding: 0 6px;
}

/* 下拉菜单 */
.menu-pop {
  position: fixed;
  z-index: 300;
  width: 190px;
  transform: translateX(-100%);
  background: var(--surface);
  border: 1px solid var(--line-2);
  border-radius: 9px;
  padding: 5px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  animation: menu-in 0.12s ease;
}

@keyframes menu-in {
  from {
    opacity: 0;
    transform: translate(-100%, -4px);
  }
  to {
    opacity: 1;
    transform: translate(-100%, 0);
  }
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 7px 9px;
  border: none;
  background: none;
  border-radius: 6px;
  font-family: var(--font);
  font-size: 12.5px;
  color: var(--t1);
  cursor: pointer;
  text-align: left;
}

.menu-item:hover {
  background: var(--bg-hover);
}

.menu-item svg {
  width: 14px;
  height: 14px;
  stroke: var(--t3);
  fill: none;
  stroke-width: 1.7;
  flex: none;
}

.menu-item.danger {
  color: var(--red);
}

.menu-item.danger svg {
  stroke: var(--red);
}

.menu-item.danger:hover {
  background: var(--danger-bg);
}

/* 详情弹窗 */
.mask {
  position: fixed;
  inset: 0;
  z-index: 200;
  background: rgba(20, 20, 24, 0.32);
  display: flex;
  align-items: center;
  justify-content: center;
}

.detail-box {
  width: 420px;
  max-width: calc(100vw - 48px);
  background: var(--surface);
  border-radius: 14px;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.18);
  overflow: hidden;
}

.detail-title {
  padding: 20px 22px 14px;
  font-size: 15px;
  font-weight: 600;
  color: var(--t1);
  border-bottom: 1px solid var(--line);
}

.detail-body {
  padding: 14px 22px 18px;
}

.detail-row {
  display: flex;
  gap: 14px;
  padding: 7px 0;
  font-size: 12.5px;
}

.dk {
  width: 72px;
  flex: none;
  color: var(--t4);
}

.dv {
  flex: 1;
  min-width: 0;
  color: var(--t1);
  word-break: break-all;
}

.detail-foot {
  display: flex;
  justify-content: flex-end;
  padding: 14px 22px;
  background: var(--bg-bar);
  border-top: 1px solid var(--line);
}

.btn {
  height: 34px;
  padding: 0 18px;
  border-radius: 8px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  font-family: var(--font);
  font-size: 13px;
  color: var(--t1);
  cursor: pointer;
}

.btn:hover {
  background: var(--bg-hover);
}
</style>
