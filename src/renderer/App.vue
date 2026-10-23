<script setup lang="ts">
import { onMounted, ref } from 'vue'
import Sidebar from './components/Sidebar.vue'
import ChatPane from './components/ChatPane.vue'
import SettingsPage from './components/SettingsPage.vue'
import TrashDialog from './components/TrashDialog.vue'
import { useWorkspaceStore } from './stores/workspace'
import { useSessionStore } from './stores/session'

const ws = useWorkspaceStore()
const sessions = useSessionStore()
const settingsOpen = ref(false)
const trashOpen = ref(false)

/* 窗口控制：模板中直接写 window 会被 Vue 的实例代理遮蔽，故包成方法 */
const win = {
  minimize: (): void => void window.api.window.minimize(),
  maximize: (): void => void window.api.window.maximize(),
  close: (): void => void window.api.window.close()
}

onMounted(async () => {
  await ws.load()
  await sessions.load()
  const first = sessions.list[0]
  if (first) sessions.select(first.id)
})
</script>

<template>
  <div class="app">
    <header class="titlebar">
      <div class="tb-logo">C</div>
      <span class="tb-name">Claude 工作台</span>
      <span class="tb-path">{{ ws.titlePath }}</span>
      <span class="sp" />
      <div class="wctl">
        <button title="最小化" @click="win.minimize">—</button>
        <button title="最大化" @click="win.maximize">□</button>
        <button class="close" title="关闭" @click="win.close">✕</button>
      </div>
    </header>

    <div class="workspace">
      <Sidebar @open-settings="settingsOpen = true" @open-trash="trashOpen = true" />
      <ChatPane />
    </div>

    <SettingsPage
      v-if="settingsOpen"
      @close="settingsOpen = false"
      @minimize="win.minimize"
      @maximize="win.maximize"
      @close-app="win.close"
    />
    <TrashDialog v-if="trashOpen" @close="trashOpen = false" @restored="sessions.load()" />
  </div>
</template>

<style scoped>
.app {
  height: 100vh;
  display: grid;
  grid-template-rows: 36px 1fr;
  background: var(--bg);
}

.titlebar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 10px;
  background: var(--bg-bar);
  border-bottom: 1px solid var(--line);
  -webkit-app-region: drag;
}

.tb-logo {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  flex: none;
  background: var(--green);
  color: #fff;
  display: grid;
  place-items: center;
  font-size: 10px;
  font-weight: 600;
}

.tb-name {
  font-size: 12.5px;
  font-weight: 500;
}

.tb-path {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--t4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 420px;
}

.sp {
  flex: 1;
}

.wctl {
  display: flex;
  -webkit-app-region: no-drag;
}

.wctl button {
  -webkit-app-region: no-drag;
}

.wctl button {
  width: 32px;
  height: 24px;
  display: grid;
  place-items: center;
  color: var(--t3);
  border-radius: 4px;
  font-size: 11px;
}

.wctl button:hover {
  background: var(--bg-hover);
}

.wctl button.close:hover {
  background: #e81123;
  color: #fff;
}

.workspace {
  display: grid;
  grid-template-columns: 246px minmax(0, 1fr);
  min-height: 0;
}
</style>
