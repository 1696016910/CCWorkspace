<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ChatMessage } from '@shared/types'

const props = defineProps<{ message: ChatMessage }>()

const expanded = ref(false)

const isUser = computed(() => props.message.role === 'user')
const hasTools = computed(() => (props.message.tools?.length ?? 0) > 0)

function time(iso?: string): string {
  if (!iso) return ''
  const d = new Date(iso)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}
</script>

<template>
  <div v-if="isUser" class="msg-user">
    <div class="bubble">{{ message.text }}</div>
  </div>

  <div v-else class="msg-ai">
    <div class="ai-role">
      <span class="who">Claude</span>
      <span class="tm">{{ time(message.timestamp) }}</span>
    </div>

    <button v-if="hasTools" class="done-chip" :class="{ open: expanded }" @click="expanded = !expanded">
      <span class="chev">›</span> 已处理 · {{ message.tools?.length }} 步
    </button>
    <div v-if="hasTools && expanded" class="steps">
      <div v-for="(t, i) in message.tools" :key="i" class="step">
        <span class="si">{{ t.name.slice(0, 1) }}</span>
        <span class="sname">{{ t.name }}</span>
        <span class="sarg">{{ t.target }}</span>
      </div>
    </div>

    <div class="ai-text">{{ message.text }}</div>
  </div>
</template>

<style scoped>
.msg-user {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 30px;
}

.msg-user .bubble {
  background: var(--bg-bubble);
  border-radius: 8px;
  padding: 11px 15px;
  font-size: 13.5px;
  line-height: 20px;
  color: #28282b;
  max-width: 76%;
  word-break: break-word;
  white-space: pre-wrap;
}

.msg-ai {
  margin-bottom: 30px;
}

.ai-role {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
  font-size: 11px;
  color: var(--t3);
}

.ai-role .who {
  color: var(--t2);
  font-weight: 500;
}

.ai-role .tm {
  font-family: var(--mono);
  font-size: 9.5px;
  color: var(--t4);
}

.done-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 27px;
  padding: 0 10px;
  margin-bottom: 14px;
  border-radius: 7px;
  border: 1px solid var(--line-2);
  background: #fff;
  font-size: 12.5px;
  color: var(--t2);
  transition: background 0.12s;
}

.done-chip:hover {
  background: var(--bg-soft);
}

.done-chip .chev {
  font-size: 9px;
  color: var(--t3);
  transition: transform 0.15s;
}

.done-chip.open .chev {
  transform: rotate(90deg);
}

.steps {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 14px;
  padding-left: 2px;
}

.step {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--t2);
}

.step .si {
  width: 17px;
  height: 17px;
  border-radius: 4px;
  flex: none;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  display: grid;
  place-items: center;
  font-size: 9px;
  color: var(--t3);
}

.step .sname {
  flex: none;
}

.step .sarg {
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--t3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ai-text {
  font-size: 13.5px;
  line-height: 23px;
  color: var(--t1);
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
