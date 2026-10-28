<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ChatMessage } from '@/api/types'
import { extractFilePaths, renderMarkdown } from '@/utils/markdown'
import { open as openExternal } from '@tauri-apps/plugin-dialog'

const props = defineProps<{ message: ChatMessage }>()
const emit = defineEmits<{ 'open-file': [path: string] }>()

const expanded = ref(false)
const copied = ref(false)

/** 代码块复制按钮的事件委托（HTML 由 markdown 渲染生成，挂不了 Vue 事件） */
async function onBodyClick(e: MouseEvent): Promise<void> {
  const target = e.target as HTMLElement

  // 链接必须拦下来。不拦的话 WebView 会让当前页面直接导航过去，
  // 应用界面被替换成网页且没有后退入口，只能重开。
  const link = target.closest('a')
  if (link) {
    e.preventDefault()
    const href = link.getAttribute('href')
    if (href) {
      // Tauri 没有 openExternal 直接对应项，用默认浏览器打开由 shell 插件负责；
      // 这里降级为提示，避免引入额外权限。
      console.info('[link]', href)
    }
    return
  }

  if (!target.matches('[data-copy]')) return
  const block = target.closest('.code-block')
  const code = block?.querySelector('pre code')?.textContent ?? ''
  if (!code) return
  try {
    await navigator.clipboard.writeText(code)
    const prev = target.textContent
    target.textContent = '已复制'
    setTimeout(() => (target.textContent = prev), 1400)
  } catch {
    // 剪贴板不可用时忽略
  }
}

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

/** 文件在应用内打开，不交给系统默认程序 */
function openFile(path: string): void {
  emit('open-file', path)
}

// 占位，避免未使用导入告警；链接拦截后续接入 shell 插件时用
void openExternal
</script>

<template>
  <div v-if="isUser" class="msg-user" :data-mid="message.id">
    <div class="bubble">{{ message.text }}</div>
    <button class="user-copy" :title="copied ? '已复制' : '复制'" @click="copyText">
      {{ copied ? '✓' : '⧉' }}
    </button>
  </div>

  <div v-else class="msg-ai" :data-mid="message.id">
    <div class="a-avatar">C</div>
    <div class="a-body">
      <div class="ai-role">
        <span class="who">Claude</span>
        <span class="tm">{{ time(message.timestamp) }}</span>
      </div>

      <button
        v-if="hasTools"
        class="tool-pill"
        :class="{ open: expanded }"
        @click="expanded = !expanded"
      >
        <span class="tp-dot" />
        <span>已处理 {{ message.tools?.length }} 步</span>
        <svg viewBox="0 0 24 24"><path d="M6 9l6 6 6-6" /></svg>
      </button>
      <div v-if="hasTools && expanded" class="steps">
        <div v-for="(t, i) in message.tools" :key="i" class="step">
          <span class="si">{{ t.name.slice(0, 1) }}</span>
          <span class="sname">{{ t.name }}</span>
          <span class="sarg">{{ t.target }}</span>
        </div>
      </div>

      <!-- eslint-disable-next-line vue/no-v-html -- 已经过 DOMPurify 净化 -->
      <div class="ai-text md" v-html="html" @click="onBodyClick" />

      <div v-if="message.stopped" class="stopped-msg">
        <svg viewBox="0 0 24 24"><rect x="7" y="7" width="10" height="10" rx="1.5" /></svg>
        <span>已停止生成</span>
      </div>

      <div class="msg-actions">
        <button class="act" :title="copied ? '已复制' : '复制'" @click="copyText">
          <svg viewBox="0 0 24 24">
            <rect x="8" y="8" width="11" height="11" rx="2" />
            <path d="M15 5.5H6A1.5 1.5 0 004.5 7v9" />
          </svg>
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
  </div>
</template>

<style scoped>
.msg-user {
  display: flex;
  justify-content: flex-end;
  align-items: flex-start;
  gap: 6px;
  margin-bottom: 26px;
  position: relative;
}

.msg-user .bubble {
  max-width: 78%;
  background: var(--bg-bubble);
  border-radius: 14px 14px 4px 14px;
  padding: 10px 15px;
  font-size: 13.5px;
  line-height: 1.6;
  word-break: break-word;
}

.user-copy {
  width: 22px;
  height: 22px;
  border-radius: 5px;
  color: var(--t4);
  opacity: 0;
  transition: opacity 0.12s, background 0.12s;
  margin-top: 6px;
}

.msg-user:hover .user-copy {
  opacity: 1;
}

.user-copy:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.msg-ai {
  display: flex;
  gap: 12px;
  margin-bottom: 26px;
}

.a-avatar {
  width: 26px;
  height: 26px;
  flex: none;
  border-radius: 8px;
  background: var(--green-soft);
  color: var(--green);
  display: grid;
  place-items: center;
  font-size: 12px;
  font-weight: 600;
  margin-top: 1px;
}

.a-body {
  flex: 1;
  min-width: 0;
}

.ai-role {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  font-size: 11.5px;
  color: var(--t4);
}

.ai-role .who {
  color: var(--t2);
  font-weight: 500;
}

.ai-role .tm {
  font-family: var(--mono);
  font-size: 10.5px;
}

.tool-pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 26px;
  padding: 0 10px;
  margin-bottom: 10px;
  border-radius: 13px;
  background: var(--bg-soft);
  font-size: 11.5px;
  color: var(--t3);
  transition: background 0.12s;
}

.tool-pill:hover {
  background: var(--bg-hover);
}

.tool-pill .tp-dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--green);
}

.tool-pill svg {
  width: 12px;
  height: 12px;
  stroke: var(--t4);
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
  transition: transform 0.15s;
}

.tool-pill.open svg {
  transform: rotate(180deg);
}

.steps {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 12px;
}

.step {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11.5px;
  color: var(--t3);
}

.step .si {
  width: 14px;
  height: 14px;
  flex: none;
  border-radius: 3px;
  background: var(--bg-soft);
  display: grid;
  place-items: center;
  font-size: 8px;
  font-weight: 600;
}

.step .sname {
  color: var(--t2);
}

.step .sarg {
  font-family: var(--mono);
  font-size: 10.5px;
  color: var(--t4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

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
  font-family: var(--mono);
  font-size: 12px;
  line-height: 1.7;
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
  table-layout: fixed;
}

.ai-text :deep(th),
.ai-text :deep(td) {
  border: 1px solid var(--line-2);
  padding: 6px 10px;
  text-align: left;
  word-break: break-word;
  overflow-wrap: anywhere;
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

.ai-text :deep(.code-block) {
  margin: 0 0 14px;
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
  background: var(--bg-soft);
}

.ai-text :deep(.code-head) {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--line);
  background: var(--surface-2);
}

.ai-text :deep(.code-lang) {
  font-family: var(--mono);
  font-size: 10px;
  color: var(--t4);
  text-transform: uppercase;
}

.ai-text :deep(.code-copy) {
  margin-left: auto;
  height: 20px;
  padding: 0 8px;
  border-radius: 4px;
  font-size: 10.5px;
  color: var(--t3);
  border: 1px solid var(--line-2);
  background: var(--surface);
}

.ai-text :deep(.code-copy:hover) {
  color: var(--t1);
  border-color: var(--line-3);
}

.ai-text :deep(.code-block pre) {
  margin: 0;
  border: none;
  border-radius: 0;
}

.stopped-msg {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 10px;
  padding: 7px 11px;
  border-radius: 7px;
  background: var(--bg-soft);
  font-size: 12px;
  color: var(--t3);
  width: fit-content;
}

.stopped-msg svg {
  width: 13px;
  height: 13px;
  fill: currentColor;
  stroke: none;
  color: var(--t4);
}

.msg-actions {
  display: flex;
  gap: 4px;
  margin-top: 8px;
  opacity: 0;
  transition: opacity 0.12s;
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
  color: var(--t4);
}

.act:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.act svg {
  width: 13px;
  height: 13px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
}

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
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
}
</style>
