<script setup lang="ts">
import { onMounted, ref } from 'vue'
import * as api from '@/api/config'
import { useToast } from '@/composables/useToast'

const toast = useToast()
const values = ref<Record<string, string>>({})
const saving = ref(false)

const KEYS: { key: string; label: string }[] = [
  { key: 'ANTHROPIC_MODEL', label: '主模型' },
  { key: 'ANTHROPIC_DEFAULT_OPUS_MODEL', label: 'Opus 映射' },
  { key: 'ANTHROPIC_DEFAULT_SONNET_MODEL', label: 'Sonnet 映射' },
  { key: 'ANTHROPIC_DEFAULT_HAIKU_MODEL', label: 'Haiku 映射' },
  { key: 'CLAUDE_CODE_EFFORT_LEVEL', label: '推理强度' },
  { key: 'MAX_THINKING_TOKENS', label: '思考预算' }
]

async function load(): Promise<void> {
  try {
    const m = await api.readModels()
    const next: Record<string, string> = {}
    for (const { key } of KEYS) next[key] = m[key] ?? ''
    values.value = next
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  }
}

async function save(): Promise<void> {
  saving.value = true
  try {
    await api.saveModels(values.value)
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
    <h1 class="st-h1x">模型</h1>
    <p class="st-subx">
      模型只能在这里修改，会话内不提供切换。这些值存放在 settings.json 的 env 中。
    </p>

    <div class="card">
      <div v-for="({ key, label }, i) in KEYS" :key="key" class="row" :class="{ last: i === KEYS.length - 1 }">
        <div class="left">
          <div class="label">{{ label }}</div>
          <div class="key">{{ key }}</div>
        </div>
        <input v-model="values[key]" class="inp" />
      </div>
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

.label {
  font-size: 13px;
  color: var(--t1);
}

.key {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--t4);
  margin-top: 2px;
}

.inp {
  width: 260px;
  flex: none;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--line-2);
  border-radius: var(--r);
  background: var(--surface);
  font-family: var(--mono);
  font-size: 12px;
  color: var(--t1);
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
