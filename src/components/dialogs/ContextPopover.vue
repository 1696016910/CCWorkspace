<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  percent: number
  tokens: number
  max: number
  compacting?: boolean
  /** 对话进行中：压缩会开第二个 claude 操作同一会话，必须禁用 */
  streaming?: boolean
  /** 触发按钮的位置。弹窗贴在它上方，不再靠容器居中猜位置。 */
  anchor?: { left: number; top: number; right: number } | null
  /** false = 这个会话还没测过（新建、还没发消息），显示「暂无数据」而不是 0% */
  hasData?: boolean
}>()

const emit = defineEmits<{ close: []; compact: [] }>()

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

const POP_WIDTH = 380

/**
 * 由触发按钮的位置算出弹窗坐标。
 *
 * 原来靠「对话区居中 + 固定 110px」定位，窗口一宽一窄就错位——
 * 那个 110px 还是输入框高度的猜测值。
 */
const pos = computed(() => {
  const a = props.anchor
  if (!a) return { left: '50%', bottom: '110px', transform: 'translateX(-50%)' }
  // 右对齐按钮右边缘，但不超出窗口
  const maxLeft = window.innerWidth - POP_WIDTH - 12
  const left = Math.max(12, Math.min(a.right - POP_WIDTH, maxLeft))
  return {
    left: `${left}px`,
    // 贴按钮上沿，留 8px 间隙。用 top 而不是 bottom：按钮有高度，
    // 按 bottom 算会让弹窗压住按钮本身。
    bottom: `${window.innerHeight - a.top + 8}px`,
    transform: 'none'
  }
})
</script>

<template>
  <div class="scrim" @click.self="emit('close')">
    <div class="pop" :style="pos">
      <div class="phead">
        <span class="ptitle">上下文使用情况</span>
        <span class="sp" />
        <button class="x" @click="emit('close')">✕</button>
      </div>

      <div v-if="props.hasData === false" class="nodata">
        这个会话还没有上下文数据。发一条消息后就会显示。
      </div>

      <div v-else class="big">
        <span class="pct">{{ pct }}%</span>
        <span class="tokens">{{ fmt(tokens) }} / {{ fmt(max) }} tokens</span>
      </div>

      <div v-if="props.hasData !== false" class="bar">
        <i :style="{ width: Math.min(percent, 100) + '%' }" :class="{ warn: percent > 80 }" />
      </div>

      <div v-if="props.hasData !== false" class="rows">
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

      <p v-if="props.streaming" class="busy-hint">对话进行中，结束后可压缩</p>

      <button
        class="compact-btn"
        :disabled="compacting || props.streaming"
        @click="emit('compact')"
      >
        {{ compacting ? '压缩中…' : '压缩上下文' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.scrim {
  position: fixed;
  inset: 0;
  z-index: 40;
  background: rgba(0, 0, 0, 0.16);
}

.pop {
  /* fixed 才能与 getBoundingClientRect 的视口坐标对齐。
     absolute 是相对 .scrim（即对话区），会让弹窗整体右移一个侧栏宽度。 */
  position: fixed;
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
  background: var(--track);
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

.compact-btn {
  width: 100%;
  height: 32px;
  border-radius: 7px;
  border: 1px solid var(--line-3);
  background: var(--surface-2);
  font-size: 12.5px;
  color: var(--t1);
}

.compact-btn:hover:not(:disabled) {
  background: var(--bg-hover);
  border-color: var(--add-row-line);
}

.compact-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.nodata {
  padding: 14px 0 16px;
  font-size: 12.5px;
  color: var(--t4);
  line-height: 1.7;
}

.busy-hint {
  margin-bottom: 8px;
  font-size: 11.5px;
  color: var(--t4);
}

.notice {
  margin-bottom: 12px;
  padding: 9px 11px;
  border-radius: 7px;
  background: var(--surface-2);
  border: 1px solid var(--line);
  font-size: 11.5px;
  color: var(--t3);
  line-height: 1.6;
}
</style>
