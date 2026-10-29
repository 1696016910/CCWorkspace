<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { useSessionStore } from '@/stores/session'
import ConfirmDialog from './ConfirmDialog.vue'
import type { SessionSummary } from '@/api/types'

const emit = defineEmits<{ close: []; restored: [] }>()

const sessions = useSessionStore()
const loading = ref(true)
const pendingPurge = ref<SessionSummary | null>(null)
const confirmingId = ref<string | null>(null)

async function load(): Promise<void> {
  loading.value = true
  try {
    await sessions.loadDeleted()
  } finally {
    loading.value = false
  }
}

async function restore(id: string): Promise<void> {
  await sessions.restore(id)
  emit('restored')
}

async function confirmPurge(): Promise<void> {
  const s = pendingPurge.value
  pendingPurge.value = null
  if (!s) return
  confirmingId.value = s.id
  try {
    await sessions.purge(s.id)
  } finally {
    confirmingId.value = null
  }
}

function fmt(iso: string): string {
  const d = new Date(iso)
  const p = (n: number): string => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`
}

function onEsc(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close')
}

onMounted(() => {
  void load()
  window.addEventListener('keydown', onEsc)
})
onUnmounted(() => window.removeEventListener('keydown', onEsc))
</script>

<template>
  <div class="scrim" @click.self="emit('close')">
    <div class="dialog">
      <div class="dhead">
        <span class="dtitle">已删除的会话</span>
        <span class="sp" />
        <button class="x" @click="emit('close')">✕</button>
      </div>

      <div class="dbody">
        <p v-if="loading" class="hint">加载中…</p>
        <p v-else-if="sessions.deleted.length === 0" class="empty">没有已删除的会话</p>
        <div v-else>
          <div v-for="s in sessions.deleted" :key="s.id" class="row">
            <div class="info">
              <div class="t">{{ s.title || '(新会话)' }}</div>
              <div class="meta">{{ s.messageCount }} 条消息 · {{ fmt(s.updatedAt) }}</div>
            </div>
            <button class="btn" @click="restore(s.id)">恢复</button>
            <button
              class="btn dgr"
              :disabled="confirmingId === s.id"
              @click="pendingPurge = s"
            >
              {{ confirmingId === s.id ? '删除中…' : '彻底删除' }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <ConfirmDialog
      v-if="pendingPurge"
      title="彻底删除？"
      :message="`「${pendingPurge.title || '(新会话)'}」及其全部消息将被永久删除，无法恢复。`"
      confirm-text="永久删除"
      tone="danger"
      @confirm="confirmPurge"
      @cancel="pendingPurge = null"
    />
  </div>
</template>

<style scoped>
.scrim {
  position: fixed;
  inset: 0;
  z-index: 110;
  background: rgba(10, 10, 14, 0.45);
  display: grid;
  place-items: center;
}

.dialog {
  width: 520px;
  max-height: 70vh;
  background: var(--bg);
  border: 1px solid var(--line-2);
  border-radius: 10px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.dhead {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--line);
}

.dtitle {
  font-size: 13.5px;
  font-weight: 500;
}

.sp {
  flex: 1;
}

.x {
  width: 24px;
  height: 24px;
  border-radius: 5px;
  color: var(--t3);
}

.x:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.dbody {
  flex: 1;
  overflow-y: auto;
  padding: 6px 0;
}

.hint,
.empty {
  padding: 28px;
  text-align: center;
  font-size: 12.5px;
  color: var(--t4);
}

.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 11px 16px;
  border-bottom: 1px solid var(--line);
}

.row:last-child {
  border-bottom: none;
}

.info {
  flex: 1;
  min-width: 0;
}

.t {
  font-size: 12.5px;
  color: var(--t1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  font-size: 11px;
  color: var(--t4);
  margin-top: 2px;
}

.btn {
  height: 26px;
  padding: 0 11px;
  border-radius: var(--r);
  border: 1px solid var(--line-3);
  background: var(--surface-2);
  font-size: 12px;
  color: var(--t1);
}

.btn:hover {
  background: var(--bg-hover);
}

.btn.dgr {
  color: var(--red);
  border-color: var(--danger-line);
}

.btn.dgr:hover {
  background: var(--danger-soft);
}
</style>
