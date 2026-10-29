<script setup lang="ts">
import type { Attachment } from '@/api/types'

/**
 * 待发送附件条。
 *
 * 只在发送前显示，发出去即清空 —— 用户明确要求已发送的消息不保留附件。
 */
const props = defineProps<{
  items: Attachment[]
  /** 超过这个字节数标黄：base64 膨胀后可能超出 IPC 与 CLI 的承受范围 */
  largeBytes: number
}>()

const emit = defineEmits<{ remove: [id: string] }>()

function fmtSize(n: number): string {
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
  if (n >= 1024) return `${Math.round(n / 1024)} KB`
  return `${n} B`
}
</script>

<template>
  <div class="strip">
    <div
      v-for="a in props.items"
      :key="a.id"
      class="chip"
      :class="{ large: a.size > props.largeBytes }"
      :title="a.path ?? a.name"
    >
      <img v-if="a.kind === 'image' && a.preview" :src="a.preview" class="thumb" alt="" />
      <span v-else class="ficon">
        <svg viewBox="0 0 24 24">
          <path d="M6 3.5h7l5 5v12H6z" />
          <path d="M13 3.5v5h5" />
        </svg>
      </span>
      <span class="meta">
        <span class="nm">{{ a.name }}</span>
        <span class="sz">{{ fmtSize(a.size) }}</span>
      </span>
      <button class="rm" title="移除" @click="emit('remove', a.id)">✕</button>
    </div>
  </div>
</template>

<style scoped>
.strip {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 0 2px 10px;
}

.chip {
  position: relative;
  display: flex;
  align-items: center;
  gap: 9px;
  max-width: 200px;
  padding: 6px 26px 6px 6px;
  border: 1px solid var(--line-2);
  border-radius: 9px;
  background: var(--surface);
  transition: border-color 0.12s, box-shadow 0.12s;
}

.chip:hover {
  border-color: var(--line-3);
  box-shadow: var(--shadow-sm);
}

.chip.large {
  border-color: var(--warn-t);
  background: var(--warn-bg);
}

.thumb {
  width: 40px;
  height: 40px;
  object-fit: cover;
  border-radius: 6px;
  flex: none;
  background: var(--bg-soft);
  border: 1px solid var(--line);
}

.ficon {
  width: 40px;
  height: 40px;
  display: grid;
  place-items: center;
  border-radius: 5px;
  background: var(--bg-soft);
  color: var(--t3);
  flex: none;
}

.ficon svg {
  width: 16px;
  height: 16px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.meta {
  display: flex;
  flex-direction: column;
  min-width: 0;
  line-height: 1.35;
}

.nm {
  font-size: 11.5px;
  color: var(--t1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sz {
  font-size: 10.5px;
  color: var(--t4);
}

.chip.large .sz {
  color: var(--warn-t);
}

.rm {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 16px;
  height: 16px;
  border-radius: 4px;
  color: var(--t4);
  font-size: 9px;
  line-height: 1;
  display: grid;
  place-items: center;
  opacity: 0;
  transition: opacity 0.12s, background 0.12s;
}

.chip:hover .rm {
  opacity: 1;
}

.rm:hover {
  background: var(--bg-hover);
  color: var(--red);
}
</style>
