<script setup lang="ts">
import { onMounted, ref } from 'vue'

const FIELDS: { key: string; label: string; hint: string }[] = [
  { key: 'ANTHROPIC_MODEL', label: '主模型', hint: 'ANTHROPIC_MODEL' },
  { key: 'ANTHROPIC_DEFAULT_OPUS_MODEL', label: 'Opus 映射', hint: 'ANTHROPIC_DEFAULT_OPUS_MODEL' },
  { key: 'ANTHROPIC_DEFAULT_SONNET_MODEL', label: 'Sonnet 映射', hint: 'ANTHROPIC_DEFAULT_SONNET_MODEL' },
  { key: 'ANTHROPIC_DEFAULT_HAIKU_MODEL', label: 'Haiku 映射', hint: 'ANTHROPIC_DEFAULT_HAIKU_MODEL' },
  { key: 'CLAUDE_CODE_EFFORT_LEVEL', label: '推理强度', hint: 'CLAUDE_CODE_EFFORT_LEVEL' },
  { key: 'MAX_THINKING_TOKENS', label: '思考预算', hint: 'MAX_THINKING_TOKENS' }
]

const models = ref<Record<string, string>>({})
const saving = ref(false)
const message = ref<string | null>(null)

async function load(): Promise<void> {
  models.value = await window.api.config.readModels()
}

async function save(): Promise<void> {
  saving.value = true
  try {
    await window.api.config.saveModels({ ...models.value })
    message.value = '已保存，新会话生效'
    setTimeout(() => (message.value = null), 3000)
  } catch (e) {
    message.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">模型</h1>
    <p class="st-subx">模型只能在这里修改，会话内不提供切换。这些值实际存放在 <code>settings.json</code> 的 <code>env</code> 中。</p>

    <div class="st-blockx">
      <div class="st-cardx">
        <div v-for="f in FIELDS" :key="f.key" class="st-rowx">
          <div class="st-l">
            <div class="st-n">{{ f.label }}</div>
            <div class="st-d mono">{{ f.hint }}</div>
          </div>
          <div class="st-r">
            <input v-model="models[f.key]" class="model-input" :placeholder="f.key === 'ANTHROPIC_MODEL' ? 'yyy[1m]' : ''" />
          </div>
        </div>
      </div>
    </div>

    <div class="save-row">
      <span class="sp" />
      <button class="st-btn pri" :disabled="saving" @click="save">
        {{ saving ? '保存中…' : '保存' }}
      </button>
    </div>
    <p v-if="message" class="st-err ok">{{ message }}</p>
  </div>
</template>

<style scoped>
.model-input {
  width: 220px;
  height: 34px;
  padding: 0 9px;
  border: 1px solid var(--line-3);
  border-radius: var(--r);
  background: var(--surface-2);
  font-family: var(--mono);
  font-size: 12.5px;
  color: var(--t1);
}

.model-input:focus {
  border-color: #bfc3c7;
}

.save-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
}

.save-row .sp {
  flex: 1;
}

.st-err.ok {
  color: var(--green-t);
}
</style>
