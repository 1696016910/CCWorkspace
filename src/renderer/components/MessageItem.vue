<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ChatMessage } from '@shared/types'
import { extractFilePaths, renderMarkdown } from '@/utils/markdown'

const props = defineProps<{ message: ChatMessage }>()

const expanded = ref(false)
const copied = ref(false)

async function copyText(): Promise<void> {
  try {
    await navigator.clipboard.writeText(props.message.text)
    copied.value = true
    setTimeout(() => (copied.value = false), 1500)
  } catch {
    // 剪贴板不可用时静默失败，不打断阅读
  }
}

const isUser = computed(() => props.message.role === 'user')
const hasTools = computed(() => (props.message.tools?.length ?? 0) > 0)
const html = computed(() => renderMarkdown(props.message.text))
const files = computed(() => extractFilePaths(props.message.text))

function time(iso?: string): string {
  if (!iso) return ''
  const d = new Date(iso)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

async function openFile(path: string): Promise<void> {
  try {
    await window.api.file.open(path)
  } catch {
    // 文件不存在时忽略，避免打断阅读
  }
}
</script>

<template>
  <div v-if="isUser" class="msg-user">
    <div class="bubble">{{ message.text }}</div>
    <button class="user-copy" :title="copied ? '已复制' : '复制'" @click="copyText">
      {{ copied ? '✓' : '⧉' }}
    </button>
  </div>

  <div v-else class="msg-ai">
    <div class="ai-role">
      <span class="who">Claude</span>
      <span class="tm">{{ time(message.timestamp) }}</span>
    </div>

    <button
      v-if="hasTools"
      class="done-chip"
      :class="{ open: expanded }"
      @click="expanded = !expanded"
    >
      <span class="chev">›</span> 已处理 · {{ message.tools?.length }} 步
    </button>
    <div v-if="hasTools && expanded" class="steps">
      <div v-for="(t, i) in message.tools" :key="i" class="step">
        <span class="si">{{ t.name.slice(0, 1) }}</span>
        <span class="sname">{{ t.name }}</span>
        <span class="sarg">{{ t.target }}</span>
      </div>
    </div>

    <!-- eslint-disable-next-line vue/no-v-html -- 已经过 DOMPurify 净化 -->
    <div class="ai-text md" v-html="html" />

    <div class="msg-actions">
      <button class="act" :title="copied ? '已复制' : '复制'" @click="copyText">
        <svg viewBox="0 0 24 24"><rect x="8" y="8" width="11" height="11" rx="2" /><path d="M15 5.5H6A1.5 1.5 0 004.5 7v9" /></svg>
        <span>{{ copied ? '已复制' : '复制' }}</span>
      </button>
    </div>

    <div v-if="files.length" class="files">
      <button v-for="f in files" :key="f" class="file-chip" :title="f" @click="openFile(f)">
        <svg viewBox="0 0 24 24"><path d="M6 3.5h7l5 5v12H6z" /><path d="M13 3.5v5h5" /></svg>
        {{ f.split(/[\\/]/).pop() }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.user-copy {
  align-self: flex-end;
  margin-left: 6px;
  width: 22px;
  height: 22px;
  border-radius: 4px;
  color: var(--t4);
  font-size: 11px;
  opacity: 0;
  transition: opacity 0.12s, background 0.12s;
}

.msg-user:hover .user-copy {
  opacity: 1;
}

.user-copy:hover {
  background: var(--bg-hover);
  color: var(--t2);
}

.msg-actions {
  display: flex;
  gap: 4px;
  margin-top: 10px;
  opacity: 0;
  transition: opacity 0.14s;
}

.msg-ai:hover .msg-actions {
  opacity: 1;
}

.act {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 8px;
  border-radius: 5px;
  font-size: 11.5px;
  color: var(--t3);
}

.act:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.act svg {
  width: 12px;
  height: 12px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.msg-user {
  display: flex;
  justify-content: flex-end;
  margin-bottom: 30px;
  animation: msg-in 0.22s ease both;
}

.msg-user .bubble {
  background: var(--bg-bubble);
  border-radius: 8px;
  padding: 11px 15px;
  font-size: 13.5px;
  line-height: 20px;
  color: var(--t1);
  max-width: 76%;
  word-break: break-word;
  white-space: pre-wrap;
}

.msg-ai {
  margin-bottom: 30px;
  animation: msg-in 0.22s ease both;
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
  background: var(--surface);
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

/* Markdown 正文 */
.ai-text {
  font-size: 13.5px;
  line-height: 23px;
  color: var(--t1);
  word-break: break-word;
}

.ai-text :deep(p) {
  margin: 0 0 12px;
}

.ai-text :deep(p:last-child) {
  margin-bottom: 0;
}

.ai-text :deep(h1),
.ai-text :deep(h2),
.ai-text :deep(h3),
.ai-text :deep(h4) {
  margin: 18px 0 8px;
  font-weight: 600;
  line-height: 1.4;
}

.ai-text :deep(h1) {
  font-size: 18px;
}

.ai-text :deep(h2) {
  font-size: 16px;
}

.ai-text :deep(h3) {
  font-size: 14.5px;
}

.ai-text :deep(h4) {
  font-size: 13.5px;
}

.ai-text :deep(ul),
.ai-text :deep(ol) {
  margin: 0 0 12px;
  padding-left: 22px;
}

.ai-text :deep(li) {
  margin-bottom: 4px;
}

.ai-text :deep(li > p) {
  margin: 0;
}

.ai-text :deep(code) {
  font-family: var(--mono);
  font-size: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  border-radius: 4px;
  padding: 1px 5px;
}

.ai-text :deep(pre) {
  background: var(--bg-soft);
  border: 1px solid var(--line);
  border-radius: 8px;
  padding: 12px 14px;
  overflow-x: auto;
  margin: 0 0 12px;
}

.ai-text :deep(pre code) {
  background: none;
  border: none;
  padding: 0;
  font-size: 12px;
  line-height: 1.7;
}

.ai-text :deep(blockquote) {
  border-left: 3px solid var(--line-3);
  padding: 2px 0 2px 14px;
  margin: 0 0 12px;
  color: var(--t2);
}

.ai-text :deep(table) {
  border-collapse: collapse;
  margin: 0 0 12px;
  font-size: 12.5px;
  width: 100%;
}

.ai-text :deep(th),
.ai-text :deep(td) {
  border: 1px solid var(--line-2);
  padding: 6px 10px;
  text-align: left;
}

.ai-text :deep(th) {
  background: var(--bg-soft);
  font-weight: 500;
}

.ai-text :deep(a) {
  color: var(--blue);
  text-decoration: none;
}

.ai-text :deep(a:hover) {
  text-decoration: underline;
}

.ai-text :deep(hr) {
  border: none;
  border-top: 1px solid var(--line);
  margin: 16px 0;
}

/* 文件链接 */
.files {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 10px;
}

.file-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 10px;
  border-radius: 6px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--t2);
  transition: all 0.12s;
}

.file-chip:hover {
  border-color: var(--green);
  color: var(--green-t);
}

.file-chip svg {
  width: 12px;
  height: 12px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}
</style>
