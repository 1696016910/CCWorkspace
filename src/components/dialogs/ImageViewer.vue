<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue'

/** 图片大图预览。只用于消息里带过的图片附件，不承担文件内容预览。 */
const props = defineProps<{ src: string; alt?: string }>()
const emit = defineEmits<{ close: [] }>()

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close')
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="scrim" @click.self="emit('close')">
    <div class="box">
      <div class="bar">
        <span class="name">{{ props.alt || '图片' }}</span>
        <span class="sp" />
        <button class="x" title="关闭" @click="emit('close')">✕</button>
      </div>
      <div class="stage">
        <img :src="props.src" :alt="props.alt ?? ''" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.scrim {
  position: fixed;
  inset: 0;
  z-index: 130;
  background: rgba(10, 10, 14, 0.72);
  display: grid;
  place-items: center;
  padding: 40px;
}

.box {
  max-width: min(1000px, 92vw);
  max-height: 88vh;
  display: flex;
  flex-direction: column;
  background: var(--surface);
  border-radius: 12px;
  overflow: hidden;
  box-shadow: var(--shadow-lg);
}

.bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-bottom: 1px solid var(--line);
  flex: none;
}

.name {
  font-size: 12px;
  color: var(--t2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sp {
  flex: 1;
}

.x {
  width: 22px;
  height: 22px;
  border-radius: 5px;
  color: var(--t3);
  font-size: 11px;
  flex: none;
}

.x:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.stage {
  flex: 1;
  min-height: 0;
  display: grid;
  place-items: center;
  background: var(--bg-soft);
  padding: 12px;
}

.stage img {
  max-width: 100%;
  max-height: calc(88vh - 90px);
  object-fit: contain;
  border-radius: 6px;
}
</style>
