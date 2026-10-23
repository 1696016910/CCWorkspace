<script setup lang="ts">
import { useWorkspaceStore } from '@/stores/workspace'
import { useSessionStore } from '@/stores/session'

const ws = useWorkspaceStore()
const sessions = useSessionStore()

/** 用于在移除确认里告知会连带删除多少会话 */
function sessionCountOf(workspaceId: string): number {
  return sessions.list.filter((s) => s.workspaceId === workspaceId).length
}
</script>

<template>
  <div>
    <h1 class="st-h1x">工作区</h1>
    <p class="st-subx">工作区是一个项目目录。会话都属于某个工作区；没有指定时使用默认工作区。</p>

    <div class="st-blockx">
      <h3>默认工作区</h3>
      <div class="st-cardx">
        <div class="st-rowx">
          <div class="st-l">
            <div class="st-n">新建会话时的默认归属</div>
            <div class="st-d">未显式选择工作区时使用</div>
          </div>
          <div class="st-r">
            <span class="st-sel">{{ ws.current?.name ?? '未设置' }}</span>
          </div>
        </div>
      </div>
    </div>

    <div class="st-blockx">
      <h3>已添加 · {{ ws.list.length }}</h3>
      <div class="st-cardx">
        <div v-for="w in ws.list" :key="w.id" class="st-rowx">
          <div class="st-l">
            <div class="st-n">{{ w.name }}</div>
            <div class="st-d mono">{{ w.path }}</div>
          </div>
          <div class="st-r">
            <span v-if="ws.defaultId === w.id" class="st-tag-default">默认</span>
            <button v-else class="st-btn" @click="ws.setDefault(w.id)">设为默认</button>
            <button class="st-btn dgr" @click="ws.remove(w.id, sessionCountOf(w.id))">移除</button>
          </div>
        </div>
        <div v-if="ws.list.length === 0" class="st-empty-row">还没有工作区</div>
      </div>
      <button class="st-add-row" @click="ws.pickAndAdd()">＋ 添加工作区</button>
    </div>

    <div class="st-notice warn">
      移除工作区只影响应用内的记录：磁盘上的项目目录不会被删除，但该工作区下的会话与消息会一并删除且无法恢复。
    </div>
    <p v-if="ws.error" class="st-err">{{ ws.error }}</p>
  </div>
</template>
