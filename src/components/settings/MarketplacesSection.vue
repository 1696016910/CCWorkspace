<script setup lang="ts">
import { onMounted, ref } from 'vue'
import * as api from '@/api/plugin'
import type { MarketplaceInfo } from '@/api/types'
import { useToast } from '@/composables/useToast'

const toast = useToast()
const list = ref<MarketplaceInfo[]>([])
const name = ref('')
const repo = ref('')
const busy = ref(false)

async function load(): Promise<void> {
  try {
    list.value = await api.listMarketplaces()
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  }
}

async function add(): Promise<void> {
  busy.value = true
  try {
    list.value = await api.addMarketplace(name.value, repo.value)
    name.value = ''
    repo.value = ''
    toast.success('已添加仓库')
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  } finally {
    busy.value = false
  }
}

async function remove(n: string): Promise<void> {
  try {
    list.value = await api.removeMarketplace(n)
    toast.info('已移除仓库')
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  }
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">插件仓库</h1>
    <p class="st-subx">添加第三方插件来源。移除后已安装的插件不会被卸载。</p>

    <div class="card">
      <div v-for="(m, i) in list" :key="m.name" class="row" :class="{ last: i === list.length - 1 }">
        <div class="left">
          <div class="nm">{{ m.name }}</div>
          <div class="src">{{ m.source }}</div>
        </div>
        <span v-if="m.builtin" class="builtin">内置</span>
        <button v-else class="st-btn dgr" @click="remove(m.name)">移除</button>
      </div>
    </div>

    <div class="add-block">
      <h3>添加仓库</h3>
      <div class="add-row">
        <input v-model="name" placeholder="仓库名称" />
        <input v-model="repo" placeholder="github.com/owner/repo" class="wide" />
        <button class="st-btn pri" :disabled="busy" @click="add">添加</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.card {
  border: 1px solid var(--line-2);
  border-radius: 10px;
  background: var(--surface);
  overflow: hidden;
}

.row {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 14px 16px;
  border-bottom: 1px solid var(--line);
}

.row.last {
  border-bottom: none;
}

.row:hover {
  background: var(--surface-2);
}

.left {
  flex: 1;
  min-width: 0;
}

.nm {
  font-size: 13px;
  color: var(--t1);
}

.src {
  font-size: 11.5px;
  color: var(--t4);
  margin-top: 2px;
}

.builtin {
  font-size: 11.5px;
  color: var(--t4);
}

.st-btn.dgr {
  color: var(--red);
}

.add-block {
  margin-top: 18px;
}

.add-block h3 {
  font-size: 12.5px;
  font-weight: 500;
  color: var(--t2);
  margin-bottom: 8px;
}

.add-row {
  display: flex;
  gap: 8px;
}

.add-row input {
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--line-2);
  border-radius: var(--r);
  background: var(--surface);
  font-size: 12px;
  flex: 0 0 180px;
}

.add-row input.wide {
  flex: 1;
  font-family: var(--mono);
  font-size: 11.5px;
}
</style>
