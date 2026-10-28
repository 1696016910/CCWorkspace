<script setup lang="ts">
import { onMounted, ref } from 'vue'
import * as api from '@/api/config'
import type { EnvEntry } from '@/api/types'
import { useToast } from '@/composables/useToast'

const toast = useToast()
const entries = ref<EnvEntry[]>([])
const saving = ref(false)
const healthWarning = ref<string | null>(null)

async function load(): Promise<void> {
  try {
    entries.value = await api.readEnv()
    healthWarning.value = await api.health()
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  }
}

async function save(): Promise<void> {
  saving.value = true
  try {
    await api.saveEnv(entries.value.map((e) => ({ key: e.key, value: e.value })))
    toast.success('已保存，已备份到 ~/.claude/backups/')
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  } finally {
    saving.value = false
  }
}

function addRow(): void {
  entries.value.push({ key: '', value: '' })
}

function removeRow(i: number): void {
  entries.value.splice(i, 1)
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">环境变量</h1>
    <p class="st-subx">
      写入 <code>~/.claude/settings.json</code> 的 <code>env</code>，保存前自动备份。修改后需要重启会话才生效。
    </p>

    <div v-if="healthWarning" class="st-notice warn">{{ healthWarning }}</div>

    <div class="env-table">
      <div class="env-head">
        <div>键</div>
        <div>值</div>
        <div></div>
      </div>
      <div v-for="(e, i) in entries" :key="i" class="env-tr">
        <input v-model="e.key" class="k" placeholder="KEY_NAME" />
        <input v-model="e.value" placeholder="value" />
        <div class="act">
          <button class="mbtn del" @click="removeRow(i)">✕</button>
        </div>
      </div>
      <div v-if="entries.length === 0" class="env-empty">还没有环境变量</div>
    </div>

    <button class="add-row" @click="addRow">＋ 添加变量</button>

    <div class="st-notice info">已在运行的会话不会读取新变量，请关闭后重新打开。</div>

    <div class="save-row">
      <span class="hint">保存前备份到 ~/.claude/backups/</span>
      <span class="sp" />
      <button class="st-btn pri" :disabled="saving" @click="save">
        {{ saving ? '保存中…' : '保存' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.env-table {
  border: 1px solid var(--line-2);
  border-radius: 10px;
  overflow: hidden;
  background: var(--surface);
}

.env-head,
.env-tr {
  display: grid;
  grid-template-columns: minmax(250px, 320px) minmax(0, 1fr) 60px;
  align-items: center;
}

.env-head {
  padding: 8px 12px;
  background: var(--bg-bar);
  border-bottom: 1px solid var(--line-2);
  font-size: 11.5px;
  color: var(--t3);
}

.env-tr {
  border-bottom: 1px solid var(--line);
}

.env-tr:last-child {
  border-bottom: none;
}

.env-tr:hover {
  background: var(--surface-2);
}

.env-tr input {
  width: 100%;
  padding: 10px 8px;
  font-family: var(--mono);
  font-size: 12px;
  color: var(--t2);
}

.env-tr input.k {
  color: var(--blue);
  padding-left: 12px;
}

.env-tr input:focus {
  color: var(--t1);
}

.act {
  display: flex;
  justify-content: center;
}

.mbtn {
  width: 21px;
  height: 21px;
  border-radius: 4px;
  color: #a0a0a7;
  font-size: 10.5px;
  display: grid;
  place-items: center;
}

.mbtn:hover {
  background: var(--bg-hover);
  color: var(--t2);
}

.mbtn.del:hover {
  color: var(--red);
}

.env-empty {
  padding: 22px;
  text-align: center;
  font-size: 12px;
  color: var(--t4);
}

.add-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  width: 100%;
  height: 32px;
  margin-top: 8px;
  border: 1px dashed var(--line-3);
  border-radius: 8px;
  font-size: 12px;
  color: var(--t3);
  background: #fcfcfd;
}

.add-row:hover {
  border-color: #bfc3c7;
  color: var(--t2);
  background: var(--bg-soft);
}

.save-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
}

.save-row .hint {
  font-size: 11.5px;
  color: var(--t4);
}

.save-row .sp {
  flex: 1;
}
</style>
