<script setup lang="ts">
import { computed } from 'vue'
import type { ContextUsageDetail } from '@shared/types'

const props = defineProps<{
  detail: ContextUsageDetail | null
  loading: boolean
}>()

const emit = defineEmits<{ close: []; compact: [] }>()

const percent = computed(() => {
  const p = props.detail?.percentage ?? 0
  return p > 0 && p < 1 ? p.toFixed(1) : String(Math.round(p))
})

const barWidth = computed(() => Math.max(props.detail?.percentage ?? 0, 0.5))

const KIND_LABEL: Record<string, string> = {
  used: '',
  deferred: '（窗口外）',
  free: '（剩余）',
  buffer: '（预留）'
}

/** /context 返回的分类里，free 是剩余空间，单独展示更直观 */
const usedRows = computed(() =>
  (props.detail?.categories ?? [])
    .filter((c) => c.tokens > 0 && c.kind !== 'free')
    .sort((a, b) => b.tokens - a.tokens)
)

const freeRow = computed(() =>
  (props.detail?.categories ?? []).find((c) => c.kind === 'free')
)

function fmt(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return String(n)
}
</script>

<template>
  <div class="scrim" @click.self="emit('close')">
    <div class="pop">
      <div class="phead">
        <span class="ptitle">上下文使用情况</span>
        <span class="sp" />
        <button class="x" @click="emit('close')">✕</button>
      </div>

      <div v-if="loading" class="loading">读取中…</div>

      <template v-else-if="detail">
        <div class="big">
          <span class="pct">{{ percent }}%</span>
          <span class="tokens">{{ fmt(detail.totalTokens) }} / {{ fmt(detail.maxTokens) }} tokens</span>
        </div>

        <div class="bar">
          <i :style="{ width: `${Math.min(barWidth, 100)}%` }" :class="{ warn: Number(percent) > 80 }" />
        </div>

        <div class="model">模型：{{ detail.model }}</div>

        <div v-if="usedRows.length" class="cats">
          <div v-for="c in usedRows" :key="c.name" class="cat">
            <span class="cname">{{ c.name }}<em v-if="KIND_LABEL[c.kind]">{{ KIND_LABEL[c.kind] }}</em></span>
            <span class="ctokens">{{ fmt(c.tokens) }}</span>
          </div>
        </div>
        <p v-else class="nocat">正在读取分类明细…</p>

        <div v-if="freeRow" class="free-row">
          <span>剩余空间</span>
          <span>{{ fmt(freeRow.tokens) }}</span>
        </div>

        <div class="notice">
          <span>压缩会把历史对话摘要成一段更短的上下文，之后仍可继续对话，但细节会丢失。</span>
        </div>

        <button class="compact-btn" @click="emit('compact')">压缩上下文</button>
      </template>

      <div v-else class="loading">暂无上下文数据，先发一条消息试试。</div>
    </div>
  </div>
</template>

<style scoped>
.scrim {
  position: absolute;
  inset: 0;
  z-index: 40;
  background: rgba(0, 0, 0, 0.16);
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding-bottom: 110px;
}

.pop {
  width: 380px;
  background: var(--surface);
  border: 1px solid var(--line-2);
  border-radius: 10px;
  box-shadow: 0 16px 44px rgba(0, 0, 0, 0.16);
  padding: 14px 16px 16px;
}

.phead {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
}

.ptitle {
  font-size: 13px;
  font-weight: 500;
}

.phead .sp {
  flex: 1;
}

.x {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  color: var(--t3);
  font-size: 11px;
}

.x:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.loading {
  padding: 18px 0;
  text-align: center;
  font-size: 12px;
  color: var(--t4);
}

.big {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 8px;
}

.pct {
  font-size: 24px;
  font-weight: 600;
  letter-spacing: -0.5px;
}

.tokens {
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--t3);
}

.bar {
  height: 10px;
  border-radius: 5px;
  background: #ececee;
  overflow: hidden;
  margin-bottom: 10px;
}

.bar i {
  display: block;
  height: 100%;
  background: var(--green);
  border-radius: 4px;
  transition: width 0.3s;
}

.bar i.warn {
  background: #d99a45;
}

.model {
  font-size: 11.5px;
  color: var(--t4);
  margin-bottom: 12px;
}

.cats {
  border-top: 1px solid var(--line);
  padding-top: 10px;
  margin-bottom: 12px;
  max-height: 180px;
  overflow-y: auto;
}

.cat {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 4px 0;
  font-size: 12px;
}

.cname {
  flex: 1;
  color: var(--t2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ctokens {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--t3);
}

.nocat {
  font-size: 12px;
  color: var(--t4);
  padding: 10px 0;
  text-align: center;
}

.cname em {
  font-style: normal;
  font-size: 10px;
  color: var(--t4);
  margin-left: 4px;
}

.free-row {
  display: flex;
  justify-content: space-between;
  padding: 8px 0 0;
  border-top: 1px solid var(--line);
  margin-bottom: 10px;
  font-size: 12px;
  color: var(--t3);
}

.notice {
  padding: 9px 11px;
  border-radius: 7px;
  background: var(--surface-2);
  border: 1px solid var(--line);
  font-size: 11.5px;
  color: var(--t3);
  line-height: 1.6;
  margin-bottom: 12px;
}

.compact-btn {
  width: 100%;
  height: 32px;
  border-radius: 7px;
  border: 1px solid var(--line-3);
  background: var(--surface-2);
  font-size: 12.5px;
  color: var(--t1);
}

.compact-btn:hover {
  background: var(--bg-hover);
  border-color: #bfc3c7;
}
</style>
