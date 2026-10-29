<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useToast } from '@/composables/useToast'
import { useWorkspaceStore } from '@/stores/workspace'
import { useSessionStore } from '@/stores/session'
import { useChatStore } from '@/stores/chat'
import ConfirmDialog from './dialogs/ConfirmDialog.vue'
import NewWorkspaceDialog from './dialogs/NewWorkspaceDialog.vue'
import type { SessionSummary } from '@/api/types'
import * as themeUtil from '@/utils/theme'

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

/* ── 下拉菜单（挂 body 上，避免被 overflow 裁掉） ── */
interface MenuItem {
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
  const MENU_W = 190
  // 优先从按钮右侧展开；右侧空间不够时改为左边对齐
  const right = rect.right + 4
  const x = right + MENU_W > window.innerWidth - 8 ? Math.max(8, rect.left - MENU_W - 4) : right
  menu.value = { x, y: rect.bottom + 4, items }
}

function closeMenu(): void {
  menu.value = null
}

async function runMenu(item: MenuItem): Promise<void> {
  closeMenu()
  await item.action()
}

const theme = ref<'light' | 'dark'>('light')

onMounted(async () => {
  theme.value = themeUtil.get()
  window.addEventListener('click', closeMenu)
  window.addEventListener('keydown', onGlobalKey)
  await loadDeletedCounts()
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
    sessions: sessions.byWorkspace(w.id)
  }))
)

/* ── 新建会话 ── */
async function newChatIn(workspaceId: string): Promise<void> {
  // 只开一个空白对话页，不落库：输入第一条消息后才成为真正的会话，
  // 那时左侧列表才会出现它。所以这里不该提示「已新建」。
  await sessions.create(workspaceId)
}

/* ── 删除（先弹确认） ── */
function askRemoveSession(s: SessionSummary): void {
  pending.value = { kind: 'session', id: s.id, title: displayTitle(sessions.list, s) }
}

async function confirmPending(): Promise<void> {
  const p = pending.value
  pending.value = null
  if (!p) return

  if (p.kind === 'session') {
    if (chat.isStreaming(p.id)) await chat.stop(p.id)
    chat.forget(p.id)
    await sessions.remove(p.id)
    toast.info('已删除 1 个会话')
  } else {
    await ws.remove(p.id)
    toast.info('已删除 1 个工作区')
  }
}

function askRemoveWorkspace(g: Group): void {
  pending.value = {
    kind: 'workspace',
    id: g.workspaceId,
    name: g.name,
    sessionCount: g.sessions.length,
    deletedCount: deletedCountOf(g.workspaceId)
  }
}

/** 回收站里属于该工作区的会话数（会级联永久删除） */
const deletedByWorkspace = ref<Record<string, number>>({})

function deletedCountOf(workspaceId: string): number {
  return deletedByWorkspace.value[workspaceId] ?? 0
}

async function loadDeletedCounts(): Promise<void> {
  try {
    await sessions.loadDeleted()
    const map: Record<string, number> = {}
    for (const s of sessions.deleted) map[s.workspaceId] = (map[s.workspaceId] ?? 0) + 1
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

function startRename(id: string, currentTitle: string): void {
  renamingId.value = id
  renameDraft.value = currentTitle
  void nextTick(() => {
    document.querySelector<HTMLInputElement>('.chat-rename')?.select()
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
    { label: '重命名', icon: 'edit', action: () => startWsRename(g.workspaceId, g.name) },
    { label: '详情', icon: 'info', action: () => showWorkspaceDetail(g) },
    {
      label: g.pinned ? '取消置顶' : '置顶',
      icon: 'pin',
      action: () => ws.togglePin(g.workspaceId)
    },
    { label: '删除工作区', icon: 'trash', danger: true, action: () => askRemoveWorkspace(g) }
  ]
}

function sessionMenu(s: SessionSummary): MenuItem[] {
  const title = displayTitle(sessions.list, s)
  const pinned = s.pinnedAt !== null
  return [
    { label: '重命名', icon: 'edit', action: () => startRename(s.id, s.title) },
    {
      label: pinned ? '取消置顶' : '置顶',
      icon: 'pin',
      action: () => sessions.togglePin(s.id)
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
function toggleTheme(): void {
  theme.value = theme.value === 'light' ? 'dark' : 'light'
  themeUtil.set(theme.value)
}

/** 未命名的会话按顺序编号，避免列表里出现多个同名条目 */
/**
 * 列表里显示的标题。
 *
 * 后端存的空串表示"还没生成过标题"，与 '(新会话)' 是两种表示，
 * 必须一起判断，否则空标题的会话会绕过编号，出现多个同名条目。
 */
function isUnnamed(s: SessionSummary): boolean {
  return s.title === '' || s.title === '(新会话)'
}

function displayTitle(list: SessionSummary[], s: SessionSummary): string {
  if (!isUnnamed(s)) return s.title
  const unnamed = list.filter(isUnnamed)
  if (unnamed.length <= 1) return '(新会话)'
  const idx = unnamed.findIndex((x) => x.id === s.id)
  return `新会话 ${idx + 1}`
}

/** 相对时间：几分钟前 / 几小时前 / 昨天 */
function relTime(iso: string): string {
  const t = new Date(iso).getTime()
  if (Number.isNaN(t)) return ''
  const diff = Date.now() - t
  const m = Math.floor(diff / 60000)
  if (m < 1) return '刚刚'
  if (m < 60) return `${m}分钟前`
  const h = Math.floor(m / 60)
  if (h < 24) return `${h}小时前`
  const d = Math.floor(h / 24)
  if (d === 1) return '昨天'
  if (d < 30) return `${d}天前`
  return `${Math.floor(d / 30)}个月前`
}

const ICONS: Record<string, string> = {
  edit: 'M4 16.5V20h3.5L18 9.5 14.5 6 4 16.5zM20 5.5l-1.5-1.5-1.6 1.6 1.5 1.5L20 5.5z',
  info: 'M12 3a9 9 0 100 18 9 9 0 000-18zM12 11v5M12 8v.01',
  pin: 'M12 3l1.6 4.6H19l-4.3 3.2 1.7 5.2-4.4-3.3-4.4 3.3 1.7-5.2L5 7.6h5.4L12 3z',
  trash: 'M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13M10.5 10v6M13.5 10v6',
  copy: 'M8 8h11v11H8zM15 5.5H6A1.5 1.5 0 004.5 7v9'
}
</script>

<template>
  <aside class="sidebar">
    <div class="side-top">
      <button class="btn-new-ws" @click="showNewWs = true">
        <svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
        新建工作区
      </button>
    </div>

    <div class="side-list">
      <div v-for="g in groups" :key="g.workspaceId" class="ws">
        <div class="ws-head" @click="sessions.activeId ? null : null">
          <span class="ws-tri">
            <svg viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" /></svg>
          </span>
          <span class="ws-ico">
            <svg viewBox="0 0 24 24">
              <path d="M3.5 7.5h6l1.8 2h9.2v9h-17v-11z" />
            </svg>
          </span>

          <input
            v-if="renamingWsId === g.workspaceId"
            v-model="wsRenameDraft"
            class="ws-rename ws-name"
            @click.stop
            @keydown.enter="commitWsRename(g.workspaceId)"
            @keydown.esc="renamingWsId = null"
            @blur="commitWsRename(g.workspaceId)"
          />
          <span v-else class="ws-name" :title="g.path">{{ g.name }}</span>

          <span class="ws-count">{{ g.sessions.length }}</span>
          <div class="ws-btns">
            <button class="row-btn add" title="新建会话" @click.stop="newChatIn(g.workspaceId)">
              <svg viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg>
            </button>
            <button class="row-btn more" title="更多" @click.stop="openMenu($event, workspaceMenu(g))">
              <svg viewBox="0 0 24 24">
                <circle cx="5" cy="12" r="1.6" />
                <circle cx="12" cy="12" r="1.6" />
                <circle cx="19" cy="12" r="1.6" />
              </svg>
            </button>
          </div>
        </div>

        <div>
          <div
            v-for="s in g.sessions"
            :key="s.id"
            class="chat-item"
            :class="{ on: sessions.activeId === s.id }"
            @click="sessions.select(s.id)"
            @dblclick="startRename(s.id, s.title)"
          >
            <input
              v-if="renamingId === s.id"
              v-model="renameDraft"
              class="chat-rename chat-title"
              @click.stop
              @keydown.enter="commitRename"
              @keydown.esc="renamingId = null"
              @blur="commitRename"
            />
            <span
              v-else
              class="chat-title"
              :class="{ unread: chat.unseen.has(s.id) }"
            >{{ displayTitle(sessions.list, s) }}</span>

            <span class="chat-time">{{ relTime(s.updatedAt) }}</span>
            <span
              class="chat-dot"
              :class="chat.isStreaming(s.id) ? 'live' : chat.unseen.has(s.id) ? 'unseen' : 'idle'"
              :title="chat.isStreaming(s.id) ? '正在生成' : chat.unseen.has(s.id) ? '有新回复' : ''"
            />
            <button class="row-btn more" title="更多" @click.stop="openMenu($event, sessionMenu(s))">
              <svg viewBox="0 0 24 24">
                <circle cx="5" cy="12" r="1.6" />
                <circle cx="12" cy="12" r="1.6" />
                <circle cx="19" cy="12" r="1.6" />
              </svg>
            </button>
          </div>
          <div v-if="g.sessions.length === 0" class="ws-empty">暂无会话，点上方 + 新建</div>
        </div>
      </div>
    </div>

    <div class="side-foot">
      <button class="foot-btn" @click="toggleTheme">
        <svg viewBox="0 0 24 24">
          <path d="M20 14.5A8.5 8.5 0 019.5 4a8.5 8.5 0 1010.5 10.5z" />
        </svg>
        {{ theme === 'dark' ? '亮色模式' : '暗色模式' }}
      </button>
      <button class="foot-btn" @click="emit('open-trash')">
        <svg viewBox="0 0 24 24">
          <path d="M12 3a9 9 0 100 18 9 9 0 000-18zM8 12h8" />
        </svg>
        回收站
        <span v-if="sessions.deleted.length" class="fb-badge">{{ sessions.deleted.length }}</span>
      </button>
      <button class="foot-btn" @click="emit('open-settings')">
        <svg viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="3" />
          <path d="M19 12a7 7 0 00-.1-1.2l2-1.5-2-3.4-2.3 1a7 7 0 00-2-1.2L14.2 3H9.8l-.4 2.7a7 7 0 00-2 1.2l-2.3-1-2 3.4 2 1.5A7 7 0 004 12c0 .4 0 .8.1 1.2l-2 1.5 2 3.4 2.3-1a7 7 0 002 1.2l.4 2.7h4.4l.4-2.7a7 7 0 002-1.2l2.3 1 2-3.4-2-1.5c.1-.4.1-.8.1-1.2z" />
        </svg>
        设置
      </button>
    </div>

    <!-- 下拉菜单 -->
    <Teleport to="body">
      <div v-if="menu" class="menu-pop" :style="{ left: menu.x + 'px', top: menu.y + 'px' }" @click.stop>
        <button
          v-for="it in menu.items"
          :key="it.label"
          class="menu-item"
          :class="{ danger: it.danger }"
          @click="runMenu(it)"
        >
          <svg viewBox="0 0 24 24"><path :d="ICONS[it.icon]" /></svg>
          {{ it.label }}
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
      :message="`该工作区下的 ${pending.sessionCount} 个会话都会被删除。`"
      :warning="
        pending.deletedCount > 0
          ? `此外，回收站里还有 ${pending.deletedCount} 个属于该工作区的会话会被一并永久删除。`
          : undefined
      "
      confirm-text="删除工作区"
      tone="danger"
      @confirm="confirmPending"
      @cancel="pending = null"
    />

    <NewWorkspaceDialog
      v-if="showNewWs"
      @close="showNewWs = false"
      @created="ws.load()"
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
  font-size: 13px;
  font-weight: 500;
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
}

.ws-tri svg {
  width: 10px;
  height: 10px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2.4;
  stroke-linecap: round;
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
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.ws-btns {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: none;
}

.row-btn {
  width: 24px;
  height: 24px;
  border-radius: 5px;
  color: var(--t3);
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
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.chat-title.unread {
  font-weight: 600;
  color: var(--t1);
}

.chat-dot.unseen {
  background: var(--warn-t);
  animation: pulse 2s infinite;
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

.ws-empty {
  font-size: 12px;
  color: var(--t4);
  padding: 10px 26px;
}

.ws-rename,
.chat-rename {
  border: 1px solid var(--green);
  border-radius: 4px;
  padding: 0 4px;
  background: var(--surface);
  color: var(--t1);
  font-size: 12.5px;
}

.side-foot {
  flex: none;
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
  border-radius: 7px;
  font-size: 12.5px;
  color: var(--t2);
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

/* 菜单 */
.menu-pop {
  position: fixed;
  z-index: 300;
  width: 190px;
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
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.menu-item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 7px 9px;
  border-radius: 6px;
  font-size: 12.5px;
  color: var(--t1);
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
  background: rgba(10, 10, 14, 0.45);
  display: grid;
  place-items: center;
}

.detail-box {
  width: 420px;
  max-width: calc(100vw - 48px);
  background: var(--surface);
  border-radius: 12px;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.18);
  overflow: hidden;
}

.detail-title {
  padding: 18px 20px 12px;
  font-size: 14px;
  font-weight: 600;
}

.detail-body {
  padding: 0 20px 16px;
}

.detail-row {
  display: flex;
  gap: 16px;
  padding: 6px 0;
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
  padding: 12px 20px;
  background: var(--bg-bar);
  border-top: 1px solid var(--line);
}

.btn {
  height: 32px;
  padding: 0 16px;
  border-radius: 7px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  font-size: 12.5px;
}

.btn:hover {
  background: var(--bg-hover);
}
</style>
