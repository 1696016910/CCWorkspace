<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { EnvEntry } from '@shared/types'

const entries = ref<EnvEntry[]>([])
const saving = ref(false)
const message = ref<string | null>(null)
const isError = ref(false)
const testResult = ref<{ ok: boolean; message: string; ms: number } | null>(null)
const testing = ref(false)

async function load(): Promise<void> {
  try {
    entries.value = await window.api.config.readEnv()
  } catch (e) {
    flash(e instanceof Error ? e.message : String(e), true)
  }
}

async function save(): Promise<void> {
  saving.value = true
  try {
    // 响应式代理无法被结构化克隆，先转成普通对象再传
    await window.api.config.saveEnv(entries.value.map((e) => ({ key: e.key, value: e.value })))
    flash('已保存，已备份到 ~/.claude/backups/', false)
  } catch (e) {
    flash(e instanceof Error ? e.message : String(e), true)
  } finally {
    saving.value = false
  }
}

async function test(): Promise<void> {
  testing.value = true
  try {
    testResult.value = await window.api.config.testConnection()
  } catch (e) {
    testResult.value = { ok: false, message: e instanceof Error ? e.message : String(e), ms: 0 }
  } finally {
    testing.value = false
  }
}

function flash(text: string, err: boolean): void {
  message.value = text
  isError.value = err
  setTimeout(() => {
    if (message.value === text) message.value = null
  }, 4000)
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

    <button class="st-add-row" @click="addRow">＋ 添加变量</button>

    <div class="st-notice info">已在运行的会话不会读取新变量，请关闭后重新打开。</div>

    <div v-if="testResult" class="test-result" :class="{ bad: !testResult.ok }">
      <span>{{ testResult.message }}</span>
      <span v-if="testResult.ms > 0" class="ms">{{ testResult.ms }}ms</span>
    </div>

    <div class="save-row">
      <span class="hint">保存前备份到 ~/.claude/backups/</span>
      <span class="sp" />
      <button class="st-btn" :disabled="testing" @click="test">
        {{ testing ? '检测中…' : '连接体检' }}
      </button>
      <button class="st-btn pri" :disabled="saving" @click="save">
        {{ saving ? '保存中…' : '保存' }}
      </button>
    </div>

    <p v-if="message" class="st-err" :class="{ ok: !isError }">{{ message }}</p>
  </div>
</template>

<style scoped>
.env-table {
  border: 1px solid var(--line-2);
  border-radius: 8px;
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
  padding: 8px 6px;
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--t2);
}

.env-tr input.k {
  color: #2f5da8;
  padding-left: 12px;
}

.env-tr input:focus {
  color: var(--t1);
}

.act {
  display: flex;
  justify-content: center;
  gap: 1px;
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

.test-result {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
  padding: 9px 12px;
  border-radius: 8px;
  font-size: 12px;
  background: var(--surface-2);
  border: 1px solid #d6e8dc;
  color: var(--green-t);
}

.test-result.bad {
  background: #fdf5f5;
  border-color: #f2dedd;
  color: #8a4b47;
}

.test-result .ms {
  margin-left: auto;
  font-family: var(--mono);
  font-size: 11px;
  opacity: 0.7;
}

.st-err.ok {
  color: var(--green-t);
}

.st-btn:disabled {
  opacity: 0.6;
  cursor: default;
}
</style>
