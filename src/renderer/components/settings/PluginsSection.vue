<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { PluginInfo } from '@shared/types'

const plugins = ref<PluginInfo[]>([])
const loading = ref(true)
const busy = ref<string | null>(null)
const error = ref<string | null>(null)
const newPluginId = ref('')

async function load(): Promise<void> {
  loading.value = true
  try {
    plugins.value = await window.api.plugin.list()
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function toggle(p: PluginInfo): Promise<void> {
  busy.value = p.id
  try {
    plugins.value = await window.api.plugin.toggle(p.id, !p.enabled)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = null
  }
}

/* Electron 不支持 window.confirm，卸载用两次点击确认 */
const confirmingId = ref<string | null>(null)

async function uninstall(p: PluginInfo): Promise<void> {
  if (confirmingId.value !== p.id) {
    confirmingId.value = p.id
    setTimeout(() => {
      if (confirmingId.value === p.id) confirmingId.value = null
    }, 4000)
    return
  }
  confirmingId.value = null
  busy.value = p.id
  try {
    await window.api.plugin.uninstall(p.id)
    await load()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = null
  }
}

async function install(): Promise<void> {
  const id = newPluginId.value.trim()
  if (!id) return
  busy.value = id
  try {
    await window.api.plugin.install(id)
    newPluginId.value = ''
    await load()
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = null
  }
}

/** 插件 id 形如 name@marketplace；格式不对时禁用安装，避免无谓的 CLI 调用 */
const canInstall = computed(() => /^[\w.-]+@[\w.-]+$/.test(newPluginId.value.trim()))

function initials(name: string): string {
  return name.slice(0, 2).toUpperCase()
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">插件</h1>
    <p class="st-subx">
      开关写入 <code>settings.json</code> 的 <code>enabledPlugins</code>。安装与卸载通过 <code>claude plugin</code> 执行。
    </p>

    <div class="install-row">
      <input
        v-model="newPluginId"
        placeholder="插件 ID，如 xxx@marketplace"
        @keydown.enter="install"
      />
      <button class="st-btn pri" :disabled="!canInstall || busy !== null" @click="install">
        {{ busy ? '处理中…' : '安装' }}
      </button>
    </div>

    <p v-if="loading" class="hint">加载中…</p>
    <p v-else-if="error" class="st-err">{{ error }}</p>

    <div v-else-if="plugins.length === 0" class="empty-state">
      <p class="en">还没有安装插件</p>
      <p class="ed">在「插件仓库」中添加仓库后即可安装</p>
    </div>

    <div v-else class="grid">
      <div v-for="p in plugins" :key="p.id" class="card">
        <div class="ico">{{ initials(p.name) }}</div>
        <div class="body">
          <div class="cn">{{ p.name }}</div>
          <div class="cv">
            <span v-if="p.version">{{ p.version }} · </span>{{ p.marketplace }}
          </div>
          <div class="cf">
            <span class="dot" :class="{ off: !p.enabled }" />
            <span :class="{ ok: p.enabled }">{{ p.enabled ? '已启用' : '已禁用' }}</span>
            <span class="sp" />
            <button
              class="mini"
              :disabled="busy === p.id"
              @click="toggle(p)"
              :title="p.enabled ? '禁用' : '启用'"
            >
              <span class="toggle" :class="{ on: p.enabled }" />
            </button>
            <button
              class="mini del"
              :class="{ confirming: confirmingId === p.id }"
              :disabled="busy === p.id"
              @click="uninstall(p)"
              :title="confirmingId === p.id ? '再次点击确认卸载' : '卸载'"
            >
              {{ confirmingId === p.id ? '确认' : '✕' }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hint {
  font-size: 12px;
  color: var(--t4);
}

.install-row {
  display: flex;
  gap: 8px;
  margin-bottom: 14px;
}

.install-row input {
  flex: 1;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--line-3);
  border-radius: var(--r);
  background: var(--surface-2);
  font-family: var(--mono);
  font-size: 11.5px;
}

.install-row input:focus {
  border-color: #bfc3c7;
}

.grid {
  display: grid;
  /* 自适应列数：窗口宽就多排几列，窄了自动减少。
     minmax 的下限用 clamp 让卡片在极窄时也不会挤成一团 */
  grid-template-columns: repeat(auto-fill, minmax(clamp(220px, 24vw, 300px), 1fr));
  gap: 10px;
}

.card {
  display: flex;
  gap: 11px;
  padding: 13px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: var(--surface);
  transition: border-color 0.12s, box-shadow 0.12s;
}

.card:hover {
  border-color: var(--line-3);
  box-shadow: 0 1px 3px rgba(31, 31, 35, 0.05);
}

.ico {
  width: 30px;
  height: 30px;
  border-radius: 6px;
  flex: none;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  display: grid;
  place-items: center;
  font-family: var(--mono);
  font-size: 9.5px;
  color: var(--t2);
}

.body {
  flex: 1;
  min-width: 0;
}

.cn {
  font-size: 12.5px;
  font-weight: 500;
}

.cv {
  font-size: 11px;
  color: var(--t4);
  margin-top: 1px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cf {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 9px;
  padding-top: 8px;
  border-top: 1px solid var(--line);
  font-size: 11px;
  color: var(--t4);
}

.cf .dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--green);
  flex: none;
}

.cf .dot.off {
  background: #c6c6cb;
}

.cf .ok {
  color: var(--green-t);
}

.cf .sp {
  flex: 1;
}

.mini {
  display: grid;
  place-items: center;
  width: 24px;
  height: 20px;
  border-radius: 4px;
  color: var(--t4);
  font-size: 10px;
}

.mini:hover {
  background: var(--bg-hover);
}

.mini.del:hover {
  color: var(--red);
}

.mini.del.confirming {
  width: auto;
  padding: 0 6px;
  background: rgba(217, 83, 79, 0.12);
  color: var(--red);
  font-size: 11px;
}

.mini:disabled {
  opacity: 0.5;
  cursor: default;
}

.empty-state {
  padding: 56px 20px;
  text-align: center;
}

.empty-state .en {
  font-size: 13px;
  color: var(--t2);
  margin-bottom: 4px;
}

.empty-state .ed {
  font-size: 12px;
  color: var(--t4);
}
</style>
