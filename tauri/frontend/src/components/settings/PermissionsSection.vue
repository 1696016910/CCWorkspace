<script setup lang="ts">
import { onMounted, ref } from 'vue'
import * as api from '@/api/config'
import { useToast } from '@/composables/useToast'

const toast = useToast()
const defaultMode = ref('default')
const allow = ref('')
const deny = ref('')
const saving = ref(false)

const MODES = ['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions']

async function load(): Promise<void> {
  try {
    const p = await api.readPermissions()
    defaultMode.value = p.defaultMode
    allow.value = p.allow.join('\n')
    deny.value = p.deny.join('\n')
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  }
}

function splitLines(t: string): string[] {
  return t
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
}

async function save(): Promise<void> {
  saving.value = true
  try {
    await api.savePermissions({
      defaultMode: defaultMode.value,
      allow: splitLines(allow.value),
      deny: splitLines(deny.value)
    })
    toast.success('已保存')
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  } finally {
    saving.value = false
  }
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">权限</h1>
    <p class="st-subx">控制 Claude 执行工具时的确认行为。默认模式决定新会话的起点。</p>

    <div class="card">
      <div class="row">
        <span>默认模式</span>
        <select v-model="defaultMode" class="sel">
          <option v-for="m in MODES" :key="m" :value="m">{{ m }}</option>
        </select>
      </div>
    </div>

    <div class="block">
      <h3>允许规则（每行一条）</h3>
      <textarea v-model="allow" rows="4" placeholder="Bash(git diff:*)" />
    </div>

    <div class="block">
      <h3>拒绝规则（每行一条）</h3>
      <textarea v-model="deny" rows="4" placeholder="Read(./.env)" />
    </div>

    <div class="save-row">
      <span class="sp" />
      <button class="st-btn pri" :disabled="saving" @click="save">
        {{ saving ? '保存中…' : '保存' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.card {
  border: 1px solid var(--line-2);
  border-radius: 10px;
  background: var(--surface);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  font-size: 13px;
}

.sel {
  height: 30px;
  padding: 0 8px;
  border: 1px solid var(--line-2);
  border-radius: var(--r);
  background: var(--surface);
  font-size: 12.5px;
  color: var(--t1);
}

.block {
  margin-top: 18px;
}

.block h3 {
  font-size: 12.5px;
  font-weight: 500;
  color: var(--t2);
  margin-bottom: 8px;
}

.block textarea {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: var(--surface);
  font-family: var(--mono);
  font-size: 12px;
  line-height: 1.7;
  color: var(--t1);
  resize: vertical;
}

.block textarea::placeholder {
  color: var(--t4);
}

.save-row {
  display: flex;
  gap: 10px;
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
}

.save-row .sp {
  flex: 1;
}
</style>
