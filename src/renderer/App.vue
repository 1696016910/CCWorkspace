<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import Sidebar from './components/Sidebar.vue'
import ChatPane from './components/ChatPane.vue'
import SettingsPage from './components/SettingsPage.vue'
import TrashDialog from './components/TrashDialog.vue'
import ErrorBoundary from './components/ErrorBoundary.vue'
import ToastHost from './components/ToastHost.vue'
import { useWorkspaceStore } from './stores/workspace'
import { useSessionStore } from './stores/session'

const ws = useWorkspaceStore()
const sessions = useSessionStore()
const settingsOpen = ref(false)
const trashOpen = ref(false)

/** Ctrl+, 打开设置；Ctrl+K 聚焦输入框 */
function onKeydown(e: KeyboardEvent): void {
  const mod = e.ctrlKey || e.metaKey
  if (mod && e.key === ',') {
    e.preventDefault()
    settingsOpen.value = true
  }
}

onMounted(async () => {
  document.documentElement.dataset.theme = await window.api.theme.get()
  window.addEventListener('keydown', onKeydown)
  await ws.load()
  await sessions.load()
  const first = sessions.list[0]
  if (first) sessions.select(first.id)
})

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown)
})

/**
 * 关闭设置时同步刷新主界面数据。
 * 设置页可能增删了工作区、改了主题，主界面需要重新读取。
 */
async function closeSettings(): Promise<void> {
  settingsOpen.value = false
  await Promise.all([ws.load(), sessions.load()])
  document.documentElement.dataset.theme = await window.api.theme.get()
}
</script>

<template>
  <div class="app">
    <div class="workspace">
      <ErrorBoundary>
        <Sidebar @open-settings="settingsOpen = true" @open-trash="trashOpen = true" />
      </ErrorBoundary>
      <ErrorBoundary>
        <ChatPane />
      </ErrorBoundary>
    </div>

    <ErrorBoundary v-if="settingsOpen">
      <SettingsPage @close="closeSettings" />
    </ErrorBoundary>
    <ErrorBoundary v-if="trashOpen">
      <TrashDialog @close="trashOpen = false" @restored="sessions.load()" />
    </ErrorBoundary>
    <ToastHost />
  </div>
</template>

<style scoped>
.app {
  height: 100vh;
  display: grid;
  grid-template-rows: 1fr;
  background: var(--bg);
}

.workspace {
  display: grid;
  grid-template-columns: 246px minmax(0, 1fr);
  min-height: 0;
}
</style>
