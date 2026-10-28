<script setup lang="ts">
import { onMounted, ref } from 'vue'
import * as app from '@/api/app'

const version = ref('—')

onMounted(async () => {
  try {
    version.value = await app.version()
  } catch {
    version.value = '未知'
  }
})
</script>

<template>
  <div>
    <h1 class="st-h1x">更新</h1>
    <p class="st-subx">
      Tauri 版是单文件程序，更新方式是下载新版本替换。各实现有独立产物。
    </p>

    <div class="card">
      <div class="row">
        <span>当前版本</span>
        <span class="mono">v{{ version }}</span>
      </div>
      <div class="row last">
        <span>发布地址</span>
        <span class="mono link">github.com/1696016910/CCWorkspace/releases</span>
      </div>
    </div>

    <div class="st-notice info">
      产物命名：ClaudeWorkbench-Node / Rust / Tauri-&lt;版本&gt;-x64.exe
    </div>
  </div>
</template>

<style scoped>
.card {
  border: 1px solid var(--line-2);
  border-radius: 10px;
  background: var(--surface);
  overflow: hidden;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
}

.row.last {
  border-bottom: none;
}

.mono {
  font-family: var(--mono);
  font-size: 12px;
  color: var(--t2);
}

.link {
  color: var(--blue);
}
</style>
