<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import WorkspaceSection from './settings/WorkspaceSection.vue'
import EnvSection from './settings/EnvSection.vue'
import PermissionsSection from './settings/PermissionsSection.vue'
import ModelsSection from './settings/ModelsSection.vue'
import SkillsSection from './settings/SkillsSection.vue'
import PluginsSection from './settings/PluginsSection.vue'
import MarketplacesSection from './settings/MarketplacesSection.vue'
import UpdateSection from './settings/UpdateSection.vue'

const emit = defineEmits<{ close: [] }>()

/** 每个分区配一个图标，纯文字导航要逐行读，有图标能靠形状定位 */
const TABS = [
  { id: 'ws', label: '工作区', group: '工作', icon: 'folder' },
  { id: 'env', label: '环境变量', group: '配置', icon: 'list' },
  { id: 'perm', label: '权限', group: '配置', icon: 'shield' },
  { id: 'model', label: '模型', group: '配置', icon: 'box' },
  { id: 'skills', label: 'Skill', group: '扩展', icon: 'spark' },
  { id: 'plugins', label: '插件', group: '扩展', icon: 'grid' },
  { id: 'market', label: '插件仓库', group: '扩展', icon: 'globe' },
  { id: 'update', label: '更新', group: '关于', icon: 'up' }
] as const

type TabId = (typeof TABS)[number]['id']

const active = ref<TabId>('ws')

function onEsc(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close')
}

onMounted(() => window.addEventListener('keydown', onEsc))
onUnmounted(() => window.removeEventListener('keydown', onEsc))

const groups = [
  { name: '工作', ids: ['ws'] as TabId[] },
  { name: '配置', ids: ['env', 'perm', 'model'] as TabId[] },
  { name: '扩展', ids: ['skills', 'plugins', 'market'] as TabId[] },
  { name: '关于', ids: ['update'] as TabId[] }
]

function labelOf(id: TabId): string {
  return TABS.find((t) => t.id === id)?.label ?? ''
}

/** 卡片型分区的容器要更宽，否则 auto-fill 只能排两列 */
const isCardLayout = computed(() =>
  active.value === 'skills' || active.value === 'plugins' || active.value === 'market'
)

function iconOf(id: TabId): string {
  return TABS.find((t) => t.id === id)?.icon ?? ''
}

const ICONS: Record<string, string> = {
  folder: 'M3.5 7.5h6l1.8 2h9.2v9h-17v-11z',
  list: 'M4 7h16M4 12h16M4 17h10',
  shield: 'M12 3l8 4v5c0 4.4-3.2 8.2-8 9-4.8-.8-8-4.6-8-9V7l8-4z',
  box: 'M5 5h14v14H5zM9 9h6M9 13h4',
  spark: 'M12 3l2.5 5.5L20 11l-5.5 2.5L12 19l-2.5-5.5L4 11l5.5-2.5z',
  grid: 'M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z',
  globe: 'M12 3a9 9 0 100 18 9 9 0 000-18zM3 12h18M12 3a15 15 0 010 18 15 15 0 010-18z',
  up: 'M12 18V8M8 12l4-4 4 4M3 12a9 9 0 1118 0 9 9 0 01-18 0z'
}
</script>

<template>
  <div class="settings">
    <div class="st-bar">
      <button class="back" type="button" @click.stop.prevent="emit('close')" title="返回主界面">
        <svg viewBox="0 0 24 24"><path d="M15 18l-6-6 6-6" /></svg>
        返回
      </button>
      <span class="st-title">设置</span>
      <span class="sp" />
      <button class="close-btn" title="关闭设置" @click="emit('close')">
        <svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" /></svg>
      </button>
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
            <svg viewBox="0 0 24 24"><path :d="ICONS[iconOf(id)]" /></svg>
            {{ labelOf(id) }}
          </button>
        </template>
      </nav>
      <main class="st-main">
        <div class="st-inner" :class="{ wide: isCardLayout }">
          <WorkspaceSection v-if="active === 'ws'" />
          <EnvSection v-else-if="active === 'env'" />
          <PermissionsSection v-else-if="active === 'perm'" />
          <ModelsSection v-else-if="active === 'model'" />
          <SkillsSection v-else-if="active === 'skills'" />
          <PluginsSection v-else-if="active === 'plugins'" />
          <MarketplacesSection v-else-if="active === 'market'" />
          <UpdateSection v-else-if="active === 'update'" />
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
  grid-template-rows: 52px 1fr;
}

.st-bar {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 16px;
  background: var(--bg-bar);
  border-bottom: 1px solid var(--line);
}

.st-bar .back {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 12px 0 9px;
  border-radius: 8px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  font-family: var(--font);
  color: var(--t1);
  font-size: 13px;
  cursor: pointer;
  transition: background 0.14s, border-color 0.14s;
  -webkit-app-region: no-drag;
}

.st-bar .back:hover {
  background: var(--bg-hover);
  border-color: var(--line-3);
}

.st-bar .back svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.st-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--t1);
}

.sp {
  flex: 1;
}

.close-btn {
  width: 32px;
  height: 32px;
  border: none;
  background: none;
  border-radius: 8px;
  color: var(--t3);
  cursor: pointer;
  display: grid;
  place-items: center;
  transition: background 0.14s, color 0.14s;
}

.close-btn:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.close-btn svg {
  width: 15px;
  height: 15px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
}

.st-body {
  display: grid;
  grid-template-columns: 232px 1fr;
  min-height: 0;
}

.st-nav {
  padding: 14px 10px;
  border-right: 1px solid var(--line);
  background: var(--bg-bar);
  overflow-y: auto;
}

.st-grp {
  font-size: 11px;
  font-weight: 500;
  color: var(--t4);
  padding: 12px 10px 6px;
}

.st-grp:first-child {
  padding-top: 2px;
}

.st-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  height: 34px;
  padding: 0 10px;
  border: none;
  background: none;
  border-radius: 7px;
  font-family: var(--font);
  font-size: 13px;
  color: var(--t2);
  text-align: left;
  cursor: pointer;
  transition: background 0.12s, color 0.12s;
}

.st-item:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.st-item.on {
  background: var(--green-soft);
  color: var(--green-t);
  font-weight: 500;
}

.st-item svg {
  width: 15px;
  height: 15px;
  flex: none;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.7;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.st-main {
  overflow-y: auto;
  min-width: 0;
}

.st-inner {
  max-width: 680px;
  padding: 28px 32px 40px;
}

/* 卡片型分区（Skill / 插件 / 仓库）用更宽的容器，
   否则 auto-fill 在 680px 里只能排下两列 */
.st-inner.wide {
  max-width: none;
}

@media (max-width: 900px) {
  .st-body {
    grid-template-columns: 190px 1fr;
  }

  .st-inner {
    padding: 22px 20px 32px;
  }
}
</style>
