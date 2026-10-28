<script setup lang="ts">
import { ref } from 'vue'
import { useWorkspaceStore } from '@/stores/workspace'
import { useSessionStore } from '@/stores/session'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '../dialogs/ConfirmDialog.vue'
import NewWorkspaceDialog from '../dialogs/NewWorkspaceDialog.vue'

const ws = useWorkspaceStore()
const sessions = useSessionStore()
const toast = useToast()

const showNew = ref(false)
const renamingId = ref<string | null>(null)
const draft = ref('')
const pendingRemove = ref<{ id: string; name: string } | null>(null)

function startRename(id: string, name: string): void {
  renamingId.value = id
  draft.value = name
}

async function commitRename(id: string): Promise<void> {
  if (renamingId.value !== id) return
  const name = draft.value.trim()
  renamingId.value = null
  if (!name) return
  await ws.rename(id, name)
  toast.success('已重命名')
}

async function confirmRemove(): Promise<void> {
  const p = pendingRemove.value
  pendingRemove.value = null
  if (!p) return
  await ws.remove(p.id)
  await sessions.load()
  toast.info('已删除工作区')
}
</script>

<template>
  <div>
    <h1 class="st-h1x">工作区</h1>
    <p class="st-subx">
      工作区是一个项目目录，每个工作区下可以建多个会话。移除工作区会同时删除它下面的会话与消息。
    </p>

    <div class="card">
      <div v-if="ws.list.length === 0" class="empty">还没有工作区</div>
      <div v-for="w in ws.list" :key="w.id" class="row">
        <div class="left">
          <div class="name">
            <input
              v-if="renamingId === w.id"
              v-model="draft"
              class="rename"
              @keydown.enter="commitRename(w.id)"
              @keydown.esc="renamingId = null"
              @blur="commitRename(w.id)"
            />
            <template v-else>{{ w.name }}</template>
          </div>
          <div class="path">{{ w.path }}</div>
        </div>
        <div class="acts">
          <button class="st-btn" @click="ws.togglePin(w.id)">
            {{ w.pinnedAt ? '取消置顶' : '置顶' }}
          </button>
          <button class="st-btn" @click="startRename(w.id, w.name)">重命名</button>
          <button class="st-btn dgr" @click="pendingRemove = { id: w.id, name: w.name }">
            移除
          </button>
        </div>
      </div>
    </div>

    <button class="add-row" @click="showNew = true">＋ 添加工作区</button>

    <ConfirmDialog
      v-if="pendingRemove"
      :title="`删除工作区「${pendingRemove.name}」？`"
      message="该工作区下的所有会话与消息都会被删除。"
      warning="磁盘上的项目目录不会被删除。"
      confirm-text="删除工作区"
      tone="danger"
      @confirm="confirmRemove"
      @cancel="pendingRemove = null"
    />
    <NewWorkspaceDialog v-if="showNew" @close="showNew = false" @created="ws.load()" />
  </div>
</template>

<style scoped>
.card {
  border: 1px solid var(--line-2);
  border-radius: 10px;
  background: var(--surface);
  overflow: hidden;
}

.empty {
  padding: 22px;
  text-align: center;
  font-size: 12px;
  color: var(--t4);
}

.row {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--line);
}

.row:last-child {
  border-bottom: none;
}

.row:hover {
  background: var(--surface-2);
}

.left {
  flex: 1;
  min-width: 0;
}

.name {
  font-size: 13px;
  color: var(--t1);
}

.path {
  font-size: 11.5px;
  color: var(--t4);
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.rename {
  border: 1px solid var(--green);
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 13px;
  width: 100%;
}

.acts {
  display: flex;
  gap: 8px;
  flex: none;
}

.st-btn.dgr {
  color: var(--red);
}

.st-btn.dgr:hover {
  background: var(--danger-soft);
}

.add-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  width: 100%;
  height: 32px;
  margin-top: 8px;
  border: 1px dashed var(--line-3);
  border-radius: 8px;
  font-size: 12px;
  color: var(--t3);
  background: #fcfcfd;
}

.add-row:hover {
  border-color: #bfc3c7;
  color: var(--t2);
  background: var(--bg-soft);
}
</style>
