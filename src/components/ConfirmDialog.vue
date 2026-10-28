<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'

/**
 * 统一的二次确认弹窗。
 *
 * 原生对话框无法定制文案层级，也不方便展示"连带影响"这类需要强调的信息。
 */
const props = withDefaults(
  defineProps<{
    /** 标题，直接陈述后果，例如「删除工作区「X」？」 */
    title: string
    /** 正文说明 */
    message: string
    /** 额外警告（红框），用于强调不可逆的连带影响 */
    warning?: string
    /** 确认按钮文案，写动作而不是「确定」 */
    confirmText?: string
    /** 取消按钮文案 */
    cancelText?: string
    /** danger = 不可恢复（红）；warn = 可恢复（黄）；info = 中性（绿） */
    tone?: 'danger' | 'warn' | 'info'
  }>(),
  {
    warning: undefined,
    confirmText: '删除',
    cancelText: '取消',
    tone: 'danger'
  }
)

const emit = defineEmits<{ confirm: []; cancel: [] }>()

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault()
    emit('cancel')
  } else if (e.key === 'Enter') {
    e.preventDefault()
    emit('confirm')
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="mask" @click.self="emit('cancel')">
    <div class="dialog" role="alertdialog" aria-modal="true">
      <div class="body">
        <div class="icon" :class="props.tone">
          <!-- danger：不可恢复 -->
          <svg v-if="props.tone === 'danger'" viewBox="0 0 24 24">
            <path d="M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13" />
            <path d="M10.5 10v6M13.5 10v6" />
          </svg>
          <!-- warn：可恢复 -->
          <svg v-else-if="props.tone === 'warn'" viewBox="0 0 24 24">
            <path d="M4.5 6.5h15M9.5 6.5V4.5h5v2M6.5 6.5l1 13h9l1-13" />
          </svg>
          <!-- info：中性 -->
          <svg v-else viewBox="0 0 24 24">
            <circle cx="12" cy="12" r="9" />
            <path d="M12 11v5M12 8v.01" />
          </svg>
        </div>
        <div class="text">
          <div class="title">{{ props.title }}</div>
          <div class="desc">{{ props.message }}</div>
          <div v-if="props.warning" class="extra">{{ props.warning }}</div>
        </div>
      </div>
      <div class="foot">
        <button class="btn" @click="emit('cancel')">{{ props.cancelText }}</button>
        <button
          class="btn"
          :class="props.tone === 'warn' ? 'primary' : 'danger'"
          @click="emit('confirm')"
        >
          {{ props.confirmText }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask {
  position: fixed;
  inset: 0;
  z-index: 200;
  background: rgba(10, 10, 14, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  animation: fade 0.14s ease;
}

@keyframes fade {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

.dialog {
  width: 420px;
  max-width: calc(100vw - 48px);
  background: var(--surface);
  border-radius: 14px;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.18), 0 2px 8px rgba(0, 0, 0, 0.06);
  overflow: hidden;
  animation: pop 0.16s ease;
}

@keyframes pop {
  from {
    opacity: 0;
    transform: scale(0.97);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.body {
  display: flex;
  gap: 14px;
  padding: 22px 24px 20px;
}

.icon {
  width: 38px;
  height: 38px;
  border-radius: 10px;
  flex: none;
  display: grid;
  place-items: center;
  background: var(--danger-bg);
}

.icon svg {
  width: 19px;
  height: 19px;
  stroke: var(--red);
  fill: none;
  stroke-width: 1.9;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.icon.warn {
  background: var(--warn-bg);
}

.icon.warn svg {
  stroke: var(--warn-t);
}

.icon.info {
  background: var(--green-soft);
}

.icon.info svg {
  stroke: var(--green);
}

.text {
  flex: 1;
  min-width: 0;
  padding-top: 1px;
}

.title {
  font-size: 14px;
  font-weight: 600;
  color: var(--t1);
  margin-bottom: 7px;
  word-break: break-word;
}

.desc {
  font-size: 12.5px;
  color: var(--t2);
  line-height: 1.65;
  word-break: break-word;
}

.extra {
  margin-top: 11px;
  padding: 9px 11px;
  border-radius: 7px;
  background: var(--danger-bg);
  border: 1px solid var(--danger-line);
  font-size: 11.5px;
  color: var(--red);
  line-height: 1.6;
}

.foot {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 14px 24px;
  background: var(--bg-bar);
  border-top: 1px solid var(--line);
}

.btn {
  height: 34px;
  padding: 0 18px;
  border-radius: 8px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  font-size: 13px;
  color: var(--t1);
  transition: background 0.14s;
}

.btn:hover {
  background: var(--bg-hover);
}

.btn.danger {
  background: var(--red);
  border-color: var(--red);
  color: #fff;
  font-weight: 500;
}

.btn.danger:hover {
  background: var(--danger-t);
}

.btn.primary {
  background: var(--green);
  border-color: var(--green);
  color: #fff;
  font-weight: 500;
}

.btn.primary:hover {
  background: var(--green-h);
}
</style>
