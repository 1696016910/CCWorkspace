<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useWorkspaceStore } from '@/stores/workspace'
import { useSessionStore } from '@/stores/session'
import ConfirmDialog from '../ConfirmDialog.vue'
import NewWorkspaceDialog from '../NewWorkspaceDialog.vue'

const ws = useWorkspaceStore()
const sessions = useSessionStore()

const showNewWs = ref(false)
const pending = ref<{ id: string; name: string; active: number; trashed: number } | null>(null)

/** 用于在移除确认里告知会连带删除多少会话 */
function sessionCountOf(workspaceId: string): number {
  return sessions.list.filter((s) => s.workspaceId === workspaceId).length
}

/**
 * 回收站里的会话也会被级联删除。
 * 它的数量不在 sessions.list 里，必须单独查，否则确认框会少报。
 */
const deletedByWorkspace = ref<Record<string, number>>({})

async function loadDeletedCounts(): Promise<void> {
  try {
    const deleted = await window.api.session.listDeleted()
    const map: Record<string, number> = {}
    for (const s of deleted) map[s.workspaceId] = (map[s.workspaceId] ?? 0) + 1
    deletedByWorkspace.value = map
  } catch {
    // 回收站读取失败不阻断主流程
  }
}

onMounted(loadDeletedCounts)

function askRemove(id: string, name: string): void {
  pending.value = {
    id,
    name,
    active: sessionCountOf(id),
    trashed: deletedByWorkspace.value[id] ?? 0
  }
}

async function confirmRemove(): Promise<void> {
  const p = pending.value
  pending.value = null
  if (!p) return
  await ws.removeConfirmed(p.id)
  await loadDeletedCounts()
}
</script>

<template>
  <div>
    <h1 class="st-h1x">工作区</h1>
    <p class="st-subx">工作区是一个项目目录。每个工作区下可以建多个会话，会话属于它所在的工作区。</p>

    <div class="st-blockx">
      <h3>已添加 · {{ ws.list.length }}</h3>
      <div class="st-cardx">
        <div v-for="w in ws.list" :key="w.id" class="st-rowx">
          <div class="st-l">
            <div class="st-n">
              <span v-if="w.pinnedAt" class="pin-mark" title="已置顶">•</span>{{ w.name }}
            </div>
            <div class="st-d mono">{{ w.path }}</div>
          </div>
          <div class="st-r">
            <button class="st-btn" @click="ws.pin(w.id, !w.pinnedAt)">
              {{ w.pinnedAt ? '取消置顶' : '置顶' }}
            </button>
            <button class="st-btn dgr" @click="askRemove(w.id, w.name)">移除</button>
          </div>
        </div>
        <div v-if="ws.list.length === 0" class="st-empty-row">还没有工作区</div>
      </div>
      <button class="st-add-row" @click="showNewWs = true">＋ 添加工作区</button>
    </div>

    <div class="st-notice warn">
      移除工作区只影响应用内的记录：磁盘上的项目目录不会被删除，但该工作区下的会话与消息会一并删除且无法恢复。
    </div>
    <p v-if="ws.error" class="st-err">{{ ws.error }}</p>

    <ConfirmDialog
      v-if="pending"
      :title="`删除工作区「${pending.name}」？`"
      :message="
        pending.active > 0
          ? `将同时删除该工作区下的 ${pending.active} 个会话及其全部消息。磁盘上的项目目录不会被删除。`
          : '磁盘上的项目目录不会被删除。'
      "
      :warning="
        pending.trashed > 0
          ? `此外，回收站里还有 ${pending.trashed} 个属于该工作区的会话会被一并永久删除。`
          : undefined
      "
      confirm-text="删除"
      tone="danger"
      @confirm="confirmRemove"
      @cancel="pending = null"
    />

    <NewWorkspaceDialog
      v-if="showNewWs"
      @close="showNewWs = false"
      @created="loadDeletedCounts()"
    />
  </div>
</template>

<style scoped>
.pin-mark {
  color: var(--green);
  margin-right: 4px;
}
</style>
