<script setup lang="ts">
import { ref } from 'vue'
import WorkspaceSection from './settings/WorkspaceSection.vue'
import EnvSection from './settings/EnvSection.vue'
import PermissionsSection from './settings/PermissionsSection.vue'
import ModelsSection from './settings/ModelsSection.vue'
import SkillsSection from './settings/SkillsSection.vue'
import PluginsSection from './settings/PluginsSection.vue'
import MarketplacesSection from './settings/MarketplacesSection.vue'

const emit = defineEmits<{
  close: []
  minimize: []
  maximize: []
  'close-app': []
}>()

const TABS = [
  { id: 'ws', label: '工作区', group: '工作' },
  { id: 'env', label: '环境变量', group: '配置' },
  { id: 'perm', label: '权限', group: '配置' },
  { id: 'model', label: '模型', group: '配置' },
  { id: 'skills', label: 'Skill', group: '扩展' },
  { id: 'plugins', label: '插件', group: '扩展' },
  { id: 'market', label: '插件仓库', group: '扩展' }
] as const

type TabId = (typeof TABS)[number]['id']

const active = ref<TabId>('ws')

const groups = [
  { name: '工作', ids: ['ws'] as TabId[] },
  { name: '配置', ids: ['env', 'perm', 'model'] as TabId[] },
  { name: '扩展', ids: ['skills', 'plugins', 'market'] as TabId[] }
]

function labelOf(id: TabId): string {
  return TABS.find((t) => t.id === id)?.label ?? ''
}
</script>

<template>
  <div class="settings">
    <div class="st-bar">
      <button class="back" type="button" @click.stop.prevent="emit('close')" title="返回主界面">
        <span class="arrow">←</span>
        <span class="back-label">返回</span>
      </button>
      <span class="st-title">设置</span>
      <span class="sp" />
      <div class="wctl">
        <button title="最小化" @click="emit('minimize')">—</button>
        <button title="最大化" @click="emit('maximize')">□</button>
        <button class="close" title="关闭" @click="emit('close-app')">✕</button>
      </div>
    </div>
    <div class="st-body">
      <nav class="st-nav">
        <template v-for="g in groups" :key="g.name">
          <div class="st-grp">{{ g.name }}</div>
          <button
            v-for="id in g.ids"
            :key="id"
            class="st-item"
            :class="{ on: active === id }"
            @click="active = id"
          >
            {{ labelOf(id) }}
          </button>
        </template>
      </nav>
      <main class="st-main">
        <div class="st-inner">
          <WorkspaceSection v-if="active === 'ws'" />
          <EnvSection v-else-if="active === 'env'" />
          <PermissionsSection v-else-if="active === 'perm'" />
          <ModelsSection v-else-if="active === 'model'" />
          <SkillsSection v-else-if="active === 'skills'" />
          <PluginsSection v-else-if="active === 'plugins'" />
          <MarketplacesSection v-else-if="active === 'market'" />
        </div>
      </main>
    </div>
  </div>
</template>

<style scoped>
.settings {
  position: fixed;
  inset: 0;
  z-index: 100;
  background: var(--bg);
  display: grid;
  grid-template-rows: 36px 1fr;
}

.st-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 10px;
  background: var(--bg-bar);
  border-bottom: 1px solid var(--line);
}

.st-bar {
  -webkit-app-region: drag;
}

.st-bar .back {
  display: flex;
  align-items: center;
  gap: 5px;
  height: 26px;
  padding: 0 10px 0 8px;
  border-radius: 5px;
  border: 1px solid var(--line-2);
  background: #fff;
  color: var(--t1);
  font-size: 12.5px;
  -webkit-app-region: no-drag;
  cursor: pointer;
}

.st-bar .back:hover {
  background: var(--bg-hover);
  border-color: var(--line-3);
}

.st-bar .arrow {
  font-size: 13px;
  line-height: 1;
}

.wctl {
  display: flex;
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
  color: var(--t1);
}

.wctl button.close:hover {
  background: #e81123;
  color: #fff;
}

.st-bar .st-title {
  font-size: 12.5px;
  font-weight: 500;
}

.st-bar .sp {
  flex: 1;
}

.st-body {
  display: grid;
  grid-template-columns: 210px minmax(0, 1fr);
  min-height: 0;
  overflow: hidden;
}

.st-nav {
  background: var(--bg-bar);
  border-right: 1px solid var(--line);
  padding: 12px 8px;
  overflow-y: auto;
}

.st-grp {
  padding: 12px 10px 5px;
  font-size: 11px;
  color: var(--t4);
}

.st-item {
  display: flex;
  align-items: center;
  height: 31px;
  padding: 0 10px;
  border-radius: var(--r);
  font-size: 12.5px;
  color: var(--t2);
  width: 100%;
  text-align: left;
  transition: background 0.12s, color 0.12s;
}

.st-item:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.st-item.on {
  background: var(--bg-select);
  color: var(--t1);
  font-weight: 500;
}

.st-main {
  overflow-y: auto;
  padding: 24px 30px 40px;
}

.st-inner {
  max-width: 960px;
}
</style>
