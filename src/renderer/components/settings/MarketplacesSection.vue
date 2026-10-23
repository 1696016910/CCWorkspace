<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { MarketplaceInfo } from '@shared/types'

const list = ref<MarketplaceInfo[]>([])
const newName = ref('')
const newRepo = ref('')
const busy = ref(false)
const error = ref<string | null>(null)

async function load(): Promise<void> {
  try {
    list.value = await window.api.marketplace.list()
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

async function add(): Promise<void> {
  busy.value = true
  try {
    list.value = await window.api.marketplace.add(newName.value, newRepo.value)
    newName.value = ''
    newRepo.value = ''
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = false
  }
}

async function remove(m: MarketplaceInfo): Promise<void> {
  if (m.builtin) return
  busy.value = true
  try {
    list.value = await window.api.marketplace.remove(m.name)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    busy.value = false
  }
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">插件仓库</h1>
    <p class="st-subx">插件从这里安装，写入 <code>settings.json</code> 的 <code>extraKnownMarketplaces</code>。</p>

    <div class="st-cardx">
      <div v-for="m in list" :key="m.name" class="st-rowx">
        <div class="st-l">
          <div class="st-n">{{ m.name }}</div>
          <div class="st-d mono">{{ m.source }}</div>
        </div>
        <div class="st-r">
          <span v-if="m.builtin" class="tag">内置</span>
          <button v-else class="st-btn dgr" :disabled="busy" @click="remove(m)">移除</button>
        </div>
      </div>
      <div v-if="list.length === 0" class="st-empty-row">暂无仓库</div>
    </div>

    <div class="add-form">
      <input v-model="newName" placeholder="仓库名称，如 my-plugins" />
      <input v-model="newRepo" placeholder="owner/repo" />
      <button class="st-btn pri" :disabled="busy || !newName || !newRepo" @click="add">添加</button>
    </div>

    <div class="st-notice info">添加后，其中的插件会出现在「插件」页的安装列表里。</div>
    <p v-if="error" class="st-err">{{ error }}</p>
  </div>
</template>

<style scoped>
.tag {
  font-size: 11px;
  color: var(--t3);
  border: 1px solid var(--line-3);
  border-radius: 4px;
  padding: 1px 7px;
}

.add-form {
  display: flex;
  gap: 8px;
  margin-top: 10px;
}

.add-form input {
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--line-3);
  border-radius: var(--r);
  background: #fcfcfd;
  font-size: 12px;
}

.add-form input:first-child {
  width: 200px;
}

.add-form input:nth-child(2) {
  flex: 1;
  font-family: var(--mono);
  font-size: 11.5px;
}

.add-form input:focus {
  border-color: #bfc3c7;
}
</style>
