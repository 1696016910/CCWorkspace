<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { renderMarkdown } from '@/utils/markdown'

const props = defineProps<{ path: string }>()
const emit = defineEmits<{ close: [] }>()

interface FileData {
  path: string
  name: string
  ext: string
  size: number
  content: string
}

const data = ref<FileData | null>(null)
const error = ref<string | null>(null)
const loading = ref(true)
const mode = ref<'code' | 'preview'>('code')
const copied = ref(false)

const MD_EXTS = new Set(['.md', '.markdown', '.mdx'])
const isMarkdown = computed(() => data.value && MD_EXTS.has(data.value.ext))

/** 行号 + 内容，用于代码显示 */
const lines = computed(() => (data.value?.content ?? '').split('\n'))

const previewHtml = computed(() =>
  isMarkdown.value ? renderMarkdown(data.value?.content ?? '') : ''
)

async function load(): Promise<void> {
  loading.value = true
  error.value = null
  try {
    data.value = await window.api.file.read(props.path)
    // Markdown 默认直接看渲染结果
    mode.value = isMarkdown.value ? 'preview' : 'code'
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function copyAll(): Promise<void> {
  if (!data.value) return
  try {
    await navigator.clipboard.writeText(data.value.content)
    copied.value = true
    setTimeout(() => (copied.value = false), 1500)
  } catch {
    // 剪贴板不可用时忽略
  }
}

/** 代码块复制按钮（Markdown 预览内的委托） */
async function onPreviewClick(e: MouseEvent): Promise<void> {
  const target = e.target as HTMLElement
  if (!target.matches('[data-copy]')) return
  const block = target.closest('.code-block')
  const code = block?.querySelector('code')?.textContent ?? ''
  if (!code) return
  try {
    await navigator.clipboard.writeText(code)
    const prev = target.textContent
    target.textContent = '已复制'
    setTimeout(() => (target.textContent = prev), 1400)
  } catch {
    // 忽略
  }
}

function extLabel(): string {
  if (!data.value) return ''
  const e = data.value.ext.replace('.', '').toUpperCase()
  return e || 'TEXT'
}

onMounted(load)
watch(() => props.path, load)
</script>

<template>
  <div class="scrim" @click.self="emit('close')">
    <div class="viewer">
      <div class="vhead">
        <span class="vicon">{{ extLabel() }}</span>
        <div class="vtitle">
          <div class="vname">{{ data?.name ?? path.split(/[\\/]/).pop() }}</div>
          <div class="vpath" :title="path">{{ path }}</div>
        </div>
        <span class="sp" />
        <div v-if="isMarkdown" class="seg">
          <button :class="{ on: mode === 'preview' }" @click="mode = 'preview'">预览</button>
          <button :class="{ on: mode === 'code' }" @click="mode = 'code'">源码</button>
        </div>
        <button class="vbtn" :title="copied ? '已复制' : '复制全文'" @click="copyAll">
          {{ copied ? '✓' : '⧉' }}
        </button>
        <button class="vclose" title="关闭" @click="emit('close')">✕</button>
      </div>

      <div v-if="loading" class="vstate">读取中…</div>
      <div v-else-if="error" class="vstate err">{{ error }}</div>

      <div v-else-if="mode === 'code'" class="vcode">
        <div v-for="(line, i) in lines" :key="i" class="vline">
          <span class="vno">{{ i + 1 }}</span>
          <span class="vtxt">{{ line }}</span>
        </div>
      </div>

      <div v-else class="vmd" @click="onPreviewClick">
        <!-- eslint-disable-next-line vue/no-v-html -- 已过 DOMPurify -->
        <div class="md-body" v-html="previewHtml" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.scrim {
  position: fixed;
  inset: 0;
  z-index: 150;
  background: rgba(0, 0, 0, 0.35);
  display: grid;
  place-items: center;
  padding: 36px;
}

.viewer {
  width: min(1000px, 100%);
  height: min(760px, 100%);
  background: var(--bg);
  border: 1px solid var(--line-2);
  border-radius: 10px;
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.35);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.vhead {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--line);
  background: var(--bg-bar);
}

.vicon {
  width: 28px;
  height: 28px;
  border-radius: 6px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  display: grid;
  place-items: center;
  font-family: var(--mono);
  font-size: 9px;
  color: var(--t2);
  flex: none;
}

.vtitle {
  min-width: 0;
}

.vname {
  font-size: 12.5px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.vpath {
  font-family: var(--mono);
  font-size: 10px;
  color: var(--t4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}

.sp {
  flex: 1;
}

.seg {
  display: flex;
  gap: 2px;
  padding: 2px;
  border-radius: 6px;
  background: var(--surface-2);
  border: 1px solid var(--line);
}

.seg button {
  height: 22px;
  padding: 0 10px;
  border-radius: 4px;
  font-size: 11px;
  color: var(--t3);
}

.seg button.on {
  background: var(--bg-select);
  color: var(--t1);
}

.vbtn,
.vclose {
  width: 26px;
  height: 26px;
  border-radius: 5px;
  color: var(--t3);
  font-size: 12px;
}

.vbtn:hover,
.vclose:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.vstate {
  flex: 1;
  display: grid;
  place-items: center;
  font-size: 12.5px;
  color: var(--t4);
}

.vstate.err {
  color: var(--red);
}

.vcode {
  flex: 1;
  overflow: auto;
  padding: 12px 0;
  font-family: var(--mono);
  font-size: 12px;
  line-height: 1.75;
}

.vline {
  display: flex;
  padding-right: 20px;
}

.vline:hover {
  background: rgba(127, 127, 127, 0.06);
}

.vno {
  width: 52px;
  flex: none;
  text-align: right;
  padding-right: 16px;
  color: var(--t4);
  font-size: 11px;
  user-select: none;
}

.vtxt {
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--t1);
}

.vmd {
  flex: 1;
  overflow: auto;
  padding: 26px 34px;
}

.md-body {
  max-width: 760px;
  margin: 0 auto;
  font-size: 13.5px;
  line-height: 1.75;
  color: var(--t1);
}

/* Markdown 排版 */
.md-body :deep(h1) {
  font-size: 22px;
  font-weight: 600;
  margin: 0 0 16px;
  letter-spacing: -0.3px;
}

.md-body :deep(h2) {
  font-size: 16px;
  font-weight: 600;
  margin: 26px 0 10px;
  padding-bottom: 6px;
  border-bottom: 1px solid var(--line);
}

.md-body :deep(h3) {
  font-size: 14px;
  font-weight: 600;
  margin: 18px 0 8px;
}

.md-body :deep(p) {
  margin: 0 0 12px;
  color: var(--t2);
}

.md-body :deep(ul),
.md-body :deep(ol) {
  margin: 0 0 12px;
  padding-left: 20px;
}

.md-body :deep(li) {
  margin-bottom: 5px;
  color: var(--t2);
}

.md-body :deep(code) {
  font-family: var(--mono);
  font-size: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  border-radius: 4px;
  padding: 1px 5px;
}

.md-body :deep(blockquote) {
  border-left: 3px solid var(--line-3);
  padding: 2px 0 2px 14px;
  margin: 0 0 12px;
  color: var(--t3);
}

.md-body :deep(table) {
  border-collapse: collapse;
  width: 100%;
  margin: 0 0 12px;
  font-size: 12.5px;
}

.md-body :deep(th),
.md-body :deep(td) {
  border: 1px solid var(--line-2);
  padding: 6px 10px;
  text-align: left;
}

.md-body :deep(th) {
  background: var(--bg-soft);
  font-weight: 500;
}

/* 代码块（与消息内一致） */
.md-body :deep(.code-block) {
  margin: 0 0 14px;
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
  background: var(--bg-soft);
}

.md-body :deep(.code-head) {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  border-bottom: 1px solid var(--line);
  background: var(--surface-2);
}

.md-body :deep(.code-lang) {
  font-family: var(--mono);
  font-size: 10px;
  color: var(--t4);
  text-transform: uppercase;
}

.md-body :deep(.code-copy) {
  margin-left: auto;
  height: 20px;
  padding: 0 8px;
  border-radius: 4px;
  font-size: 10.5px;
  color: var(--t3);
  border: 1px solid var(--line-2);
  background: var(--surface);
}

.md-body :deep(.code-copy:hover) {
  color: var(--t1);
  border-color: var(--line-3);
}

.md-body :deep(pre) {
  margin: 0;
  padding: 12px 14px;
  overflow-x: auto;
}

.md-body :deep(pre code) {
  background: none;
  border: none;
  padding: 0;
  font-size: 12px;
  line-height: 1.7;
  color: var(--t1);
}
</style>
