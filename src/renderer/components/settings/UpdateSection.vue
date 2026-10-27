<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { UpdateStatus } from '@shared/types'

const status = ref<UpdateStatus | null>(null)
const prefix = ref('')
const prefixSaving = ref(false)
const prefixMessage = ref<string | null>(null)
const busy = ref(false)

let off: (() => void) | null = null

async function load(): Promise<void> {
  status.value = await window.api.update.status()
  prefix.value = await window.api.update.getFeed()
}

function apply(next: UpdateStatus): void {
  status.value = next
  if (next.state !== 'downloading' && next.state !== 'checking') busy.value = false
}

async function check(): Promise<void> {
  busy.value = true
  try {
    apply(await window.api.update.check())
  } finally {
    busy.value = false
  }
}

async function download(): Promise<void> {
  busy.value = true
  try {
    apply(await window.api.update.download())
  } finally {
    busy.value = false
  }
}

async function install(): Promise<void> {
  await window.api.update.install()
}

async function savePrefix(): Promise<void> {
  prefixSaving.value = true
  prefixMessage.value = null
  try {
    await window.api.update.setFeed(prefix.value)
    // 重新取一次状态，让上方「更新地址」立刻反映新的前缀
    status.value = await window.api.update.status()
    prefixMessage.value = '已保存，下次检查更新时生效'
    setTimeout(() => (prefixMessage.value = null), 3000)
  } catch (e) {
    prefixMessage.value = e instanceof Error ? e.message : String(e)
  } finally {
    prefixSaving.value = false
  }
}

/** 设置页不提供 markdown，链接跳转交给主进程 */
function openReleases(): void {
  void window.api.update.openReleases()
}

const stateText = computed(() => {
  const s = status.value
  if (!s) return '读取中…'
  switch (s.state) {
    case 'checking':
      return '正在检查…'
    case 'available':
      return `发现新版本 ${s.version}`
    case 'downloading':
      return `正在下载 ${s.percent ?? 0}%`
    case 'downloaded':
      return `新版本 ${s.version} 已下载，重启后生效`
    case 'error':
      return s.message ?? '检查失败'
    default:
      return s.message ?? '已是最新版本'
  }
})

const isError = computed(() => status.value?.state === 'error')

onMounted(async () => {
  off = window.api.update.onEvent(apply)
  await load()
})

onUnmounted(() => off?.())
</script>

<template>
  <div>
    <h1 class="st-h1x">更新</h1>
    <p class="st-subx">
      更新包发布在公开的 GitHub Releases，应用启动 5 秒后静默检查一次。网络不通时可在此填写代理前缀。
    </p>

    <div class="st-blockx">
      <h3>版本</h3>
      <div class="st-cardx">
        <div class="st-rowx">
          <div class="st-l">
            <div class="st-n">当前版本</div>
            <div class="st-d mono">v{{ status?.currentVersion ?? '—' }}</div>
          </div>
          <div class="st-r">
            <button class="st-btn pri" :disabled="busy" @click="check">
              {{ busy ? '检查中…' : '检查更新' }}
            </button>
          </div>
        </div>

        <div class="st-rowx">
          <div class="st-l">
            <div class="st-n">状态</div>
            <div class="st-d" :class="{ bad: isError }">{{ stateText }}</div>
          </div>
          <div class="st-r">
            <button v-if="status?.state === 'available'" class="st-btn pri" @click="download">
              下载更新
            </button>
            <button v-if="status?.state === 'downloaded'" class="st-btn pri" @click="install">
              重启并安装
            </button>
            <button v-if="isError" class="st-btn" @click="openReleases">打开发布页</button>
          </div>
        </div>

        <div v-if="status?.state === 'downloading'" class="st-rowx">
          <div class="st-l">
            <div class="st-bar">
              <div class="st-bar-fill" :style="{ width: `${status.percent ?? 0}%` }" />
            </div>
          </div>
        </div>

        <div class="st-rowx">
          <div class="st-l">
            <div class="st-n">更新地址</div>
            <div class="st-d mono wrap">{{ status?.feed ?? '—' }}</div>
          </div>
        </div>
      </div>
    </div>

    <div class="st-blockx">
      <h3>代理</h3>
      <div class="st-cardx">
        <div class="st-rowx">
          <div class="st-l">
            <div class="st-n">代理前缀</div>
            <div class="st-d">留空表示直连 GitHub。填写后拼接在 GitHub 地址前，例如 https://your-proxy.example.com</div>
          </div>
          <div class="st-r feed-r">
            <input
              v-model="prefix"
              class="feed-input"
              placeholder="留空直连"
              spellcheck="false"
            />
            <button class="st-btn pri" :disabled="prefixSaving" @click="savePrefix">
              {{ prefixSaving ? '保存中…' : '保存' }}
            </button>
          </div>
        </div>
        <div v-if="prefixMessage" class="st-rowx">
          <div class="st-l">
            <div class="st-d" :class="prefixMessage.includes('已保存') ? 'ok' : 'bad'">
              {{ prefixMessage }}
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.feed-input {
  width: 280px;
  height: 34px;
  padding: 0 9px;
  border: 1px solid var(--line-3);
  border-radius: var(--r);
  background: var(--surface-2);
  font-family: var(--mono);
  font-size: 12.5px;
  color: var(--t1);
}

.feed-input:focus {
  border-color: #bfc3c7;
}

.st-d.mono {
  font-family: var(--mono);
  font-size: 11px;
  word-break: break-all;
}

.st-d.bad {
  color: var(--red);
}

.st-bar {
  height: 6px;
  border-radius: 3px;
  background: var(--bg-soft);
  overflow: hidden;
}

.st-bar-fill {
  height: 100%;
  border-radius: 3px;
  background: var(--green);
  transition: width 0.2s ease;
}

.feed-r {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: none;
}

.st-d.ok {
  color: var(--green-t);
}
</style>
