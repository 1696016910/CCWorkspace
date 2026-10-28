<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  percent: number
  tokens: number
  max: number
}>()

const emit = defineEmits<{ close: [] }>()

const pct = computed(() => {
  const p = props.percent
  return p > 0 && p < 1 ? p.toFixed(1) : String(Math.round(p))
})

function fmt(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return String(n)
}

const free = computed(() => Math.max(props.max - props.tokens, 0))
</script>

<template>
  <div class="scrim" @click.self="emit('close')">
    <div class="pop">
      <div class="phead">
        <span class="ptitle">上下文使用情况</span>
        <span class="sp" />
        <button class="x" @click="emit('close')">✕</button>
      </div>

      <div class="big">
        <span class="pct">{{ pct }}%</span>
        <span class="tokens">{{ fmt(tokens) }} / {{ fmt(max) }} tokens</span>
      </div>

      <div class="bar">
        <i :style="{ width: Math.min(percent, 100) + '%' }" :class="{ warn: percent > 80 }" />
      </div>

      <div class="rows">
        <div class="row">
          <span>已用上下文</span>
          <span class="mono">{{ fmt(tokens) }}</span>
        </div>
        <div class="row">
          <span>剩余空间</span>
          <span class="mono">{{ fmt(free) }}</span>
        </div>
      </div>

      <div class="notice">
        <span>压缩会把历史对话摘要成一段更短的上下文，之后仍可继续对话，但细节会丢失。</span>
      </div>
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

.sp {
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
  margin-bottom: 12px;
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

.rows {
  border-top: 1px solid var(--line);
  padding-top: 10px;
  margin-bottom: 12px;
}

.row {
  display: flex;
  justify-content: space-between;
  padding: 4px 0;
  font-size: 12px;
  color: var(--t2);
}

.mono {
  font-family: var(--mono);
  font-size: 11px;
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
}
</style>
