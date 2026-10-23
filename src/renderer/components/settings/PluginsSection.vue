<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { PluginInfo } from '@shared/types'

const plugins = ref<PluginInfo[]>([])
const loading = ref(true)
const busy = ref<string | null>(null)
const error = ref<string | null>(null)

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

async function uninstall(p: PluginInfo): Promise<void> {
  if (!window.confirm(`确定卸载 ${p.name}？`)) return
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
            <button class="mini del" :disabled="busy === p.id" @click="uninstall(p)" title="卸载">✕</button>
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

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
  gap: 9px;
}

.card {
  display: flex;
  gap: 11px;
  padding: 13px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: #fff;
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
