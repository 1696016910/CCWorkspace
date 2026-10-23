<script setup lang="ts">
import { onErrorCaptured, ref } from 'vue'

const error = ref<Error | null>(null)
const detail = ref('')

onErrorCaptured((e) => {
  error.value = e instanceof Error ? e : new Error(String(e))
  detail.value = error.value.stack ?? ''
  // 阻止错误继续冒泡，避免整个应用白屏
  return false
})

function retry(): void {
  error.value = null
  detail.value = ''
}
</script>

<template>
  <div v-if="error" class="boundary">
    <div class="box">
      <div class="ico">!</div>
      <h2>这个区域出错了</h2>
      <p class="msg">{{ error.message }}</p>
      <details v-if="detail" class="detail">
        <summary>技术细节</summary>
        <pre>{{ detail }}</pre>
      </details>
      <button class="btn" @click="retry">重试</button>
    </div>
  </div>
  <slot v-else />
</template>

<style scoped>
.boundary {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 40px 24px;
  min-height: 0;
}

.box {
  max-width: 460px;
  text-align: center;
}

.ico {
  width: 40px;
  height: 40px;
  margin: 0 auto 14px;
  border-radius: 50%;
  background: rgba(217, 83, 79, 0.14);
  color: var(--red);
  display: grid;
  place-items: center;
  font-weight: 600;
  font-size: 18px;
}

h2 {
  font-size: 16px;
  font-weight: 600;
  margin-bottom: 8px;
}

.msg {
  font-size: 12.5px;
  color: var(--t3);
  line-height: 1.6;
  margin-bottom: 14px;
  word-break: break-word;
}

.detail {
  text-align: left;
  margin-bottom: 14px;
}

.detail summary {
  cursor: pointer;
  font-size: 11.5px;
  color: var(--t3);
  margin-bottom: 6px;
}

.detail pre {
  max-height: 180px;
  overflow: auto;
  padding: 10px;
  border-radius: 6px;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  font-family: var(--mono);
  font-size: 11px;
  color: var(--t3);
  white-space: pre-wrap;
}

.btn {
  height: 32px;
  padding: 0 18px;
  border-radius: 7px;
  background: var(--green);
  color: #fff;
  font-size: 12.5px;
}

.btn:hover {
  background: var(--green-h);
}
</style>
