<script setup lang="ts">
import { useToast } from '@/composables/useToast'

const { items } = useToast()
</script>

<template>
  <div class="toasts">
    <div v-for="t in items" :key="t.id" class="toast" :class="t.kind">
      <span class="ico">{{ t.kind === 'success' ? '✓' : t.kind === 'error' ? '!' : '·' }}</span>
      <span class="text">{{ t.text }}</span>
    </div>
  </div>
</template>

<style scoped>
.toasts {
  position: fixed;
  left: 50%;
  bottom: 28px;
  transform: translateX(-50%);
  display: flex;
  flex-direction: column;
  gap: 8px;
  z-index: 300;
  pointer-events: none;
}

.toast {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 9px 14px;
  border-radius: 8px;
  background: var(--surface);
  border: 1px solid var(--line-2);
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.14);
  font-size: 12.5px;
  color: var(--t1);
  animation: toast-in 0.2s ease both;
}

.toast .ico {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  font-size: 10px;
  flex: none;
  background: var(--bg-soft);
  color: var(--t3);
}

.toast.success .ico {
  background: var(--green-soft);
  color: var(--green-t);
}

.toast.error .ico {
  background: rgba(217, 83, 79, 0.14);
  color: var(--red);
}

@keyframes toast-in {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}
</style>
