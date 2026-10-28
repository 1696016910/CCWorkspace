<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { renderMarkdown } from '@/utils/markdown'
import * as fileApi from '@/api/file'

const props = defineProps<{ path: string }>()
const emit = defineEmits<{ close: [] }>()

const loading = ref(true)
const error = ref<string | null>(null)
const content = ref('')
const mode = ref<'preview' | 'code'>('preview')
const meta = ref<fileApi.FileContent | null>(null)

const isMarkdown = computed(() => /\.(md|markdown)$/i.test(props.path))

const name = computed(() => props.path.split(/[\/]/).pop() ?? props.path)

const extLabel = computed(() => {
  const e = meta.value?.ext ?? ''
  const m = /\.(\w+)$/.exec(props.path)
  return m ? m[1].toUpperCase() : e ? e.slice(1).toUpperCase() : 'TEXT'
})

const previewHtml = computed(() => renderMarkdown(content.value))

const lines = computed(() => content.value.split('\n'))

async function load(): Promise<void> {
  loading.value = true
  error.value = null
  try {
    const d = await fileApi.read(props.path)
    meta.value = d
    content.value = d.text
    // Markdown 默认预览，其余直接看源码
    mode.value = /^(md|markdown)$/i.test(d.ext) ? 'preview' : 'code'
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

onMounted(load)

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close')
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="scrim" @click.self="emit('close')">
    <div class="viewer">
      <div class="vhead">
        <span class="vicon">{{ extLabel }}</span>
        <div class="vtitle">
          <div class="vname">{{ name }}</div>
          <div class="vpath" :title="path">{{ path }}</div>
        </div>
        <span class="sp" />
        <div v-if="isMarkdown" class="seg">
          <button :class="{ on: mode === 'preview' }" @click="mode = 'preview'">预览</button>
          <button :class="{ on: mode === 'code' }" @click="mode = 'code'">源码</button>
        </div>
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

      <div v-else class="vmd">
        <!-- eslint-disable-next-line vue/no-v-html -- 已经过 DOMPurify 净化 -->
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

.vclose {
  width: 26px;
  height: 26px;
  border-radius: 5px;
  color: var(--t3);
  font-size: 12px;
}

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
</style>
