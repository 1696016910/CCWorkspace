<script setup lang="ts">
import { ref, watch } from 'vue'
import type { StoredAttachment } from '@/api/types'
import * as fileApi from '@/api/file'

/**
 * 已发送消息里的附件回显。
 *
 * 用户发完消息后仍能看见带了什么、路径在哪 —— 否则事后无法追溯。
 * 图片按路径重读缩略图；粘贴的截图没有路径，只显示文件名与大小。
 */
const props = defineProps<{
  items: StoredAttachment[]
}>()

const emit = defineEmits<{ 'open-file': [path: string]; 'preview-image': [src: string] }>()

/** path → data URL。加载失败的记为空串，卡片退回图标态。 */
const thumbs = ref<Map<string, string>>(new Map())

function fmtSize(n: number): string {
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
  if (n >= 1024) return `${Math.round(n / 1024)} KB`
  return `${n} B`
}

/**
 * 路径显示：中间省略，保留盘符与文件名。
 *
 * C:\Users\Me\Downloads\ttt\sub\test.sql  →  C:\Users\...\sub\test.sql
 * 太短就原样显示，太长又只有两段就只保留文件名。
 */
function shortPath(p?: string): string {
  if (!p) return ''
  const sep = p.includes('\\') ? '\\' : '/'
  const parts = p.split(sep).filter(Boolean)
  if (parts.length <= 3) return p
  const head = parts[0]
  const tail = parts.slice(-2).join(sep)
  return `${head}${sep}...${sep}${tail}`
}

/** 有路径的图片才去读缩略图；没路径的（粘贴的截图）直接跳过 */
async function loadThumbs(): Promise<void> {
  for (const a of props.items) {
    if (a.kind !== 'image' || !a.path || thumbs.value.has(a.path)) continue
    try {
      const img = await fileApi.readImage(a.path)
      thumbs.value = new Map(thumbs.value).set(a.path, img.preview)
    } catch {
      // 读不到就保持图标态，不打断渲染
      thumbs.value = new Map(thumbs.value).set(a.path, '')
    }
  }
}

function open(a: StoredAttachment): void {
  if (a.kind === 'image' && a.path) {
    const src = thumbs.value.get(a.path)
    if (src) {
      emit('preview-image', src)
      return
    }
  }
  if (a.path) emit('open-file', a.path)
}

// 消息是异步加载的，items 可能在组件挂载后才变。直接调一次不够，
// 必须监听变化——否则缩略图永远停留在图标态。
watch(() => props.items, () => void loadThumbs(), { immediate: true })
</script>

<template>
  <div class="atts">
    <button
      v-for="(a, i) in props.items"
      :key="`${a.name}-${i}`"
      class="att"
      :title="a.path ?? a.name"
      @click="open(a)"
    >
      <img
        v-if="a.kind === 'image' && a.path && thumbs.get(a.path)"
        class="att-img"
        :src="thumbs.get(a.path)"
        alt=""
      />
      <span v-else class="att-ico">
        <svg v-if="a.kind === 'image'" viewBox="0 0 24 24">
          <rect x="3.5" y="5.5" width="17" height="13" rx="2" />
          <circle cx="8.5" cy="10" r="1.5" />
          <path d="M4 16l4.5-4.5L13 16l2.5-2.5L20 18" />
        </svg>
        <svg v-else viewBox="0 0 24 24">
          <path d="M6 3.5h7l5 5v12H6z" />
          <path d="M13 3.5v5h5" />
        </svg>
      </span>
      <span class="att-body">
        <span class="att-name">{{ a.name }}</span>
        <span v-if="a.path" class="att-path">{{ shortPath(a.path) }}</span>
        <span v-else class="att-path">（无本地路径）</span>
      </span>
      <span class="att-size">{{ fmtSize(a.size) }}</span>
    </button>
  </div>
</template>

<style scoped>
.atts {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
  align-items: flex-end;
}

.att {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  max-width: 340px;
  padding: 8px 11px;
  border: 1px solid var(--line-2);
  border-radius: 9px;
  background: var(--surface);
  text-align: left;
  transition: border-color 0.12s, box-shadow 0.12s;
}

.att:hover {
  border-color: var(--line-3);
  box-shadow: var(--shadow-sm);
}

.att-ico {
  /* 与图片缩略图同尺寸：两者混排时卡片高度才一致 */
  width: 46px;
  height: 46px;
  flex: none;
  border-radius: 6px;
  background: var(--bg-soft);
  display: grid;
  place-items: center;
  color: var(--t3);
}

.att-ico svg {
  width: 17px;
  height: 17px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.att-img {
  width: 46px;
  height: 46px;
  flex: none;
  border-radius: 6px;
  object-fit: cover;
  border: 1px solid var(--line);
  background: var(--bg-soft);
}

.att-body {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.att-name {
  font-size: 12.5px;
  color: var(--t1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.att-path {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--t4);
  margin-top: 1px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.att-size {
  font-size: 11px;
  color: var(--t4);
  flex: none;
}
</style>
