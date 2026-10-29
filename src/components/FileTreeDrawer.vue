<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import type { TreeNode } from '@/api/workspace'
import * as wsApi from '@/api/workspace'
import { useToast } from '@/composables/useToast'
import FileTreeNode from './FileTreeNode.vue'

/**
 * 工作区文件树抽屉。
 *
 * 只按需展开：每次调用只列一层目录。大项目一次递归会把界面卡死，
 * 而且用户要的文件通常在浅层。
 */
const props = defineProps<{
  workspaceId: string
  rootPath: string
}>()

const emit = defineEmits<{ close: []; 'open-file': [path: string] }>()

const toast = useToast()
const showIgnored = ref(false)
const loading = ref(false)
const error = ref<string | null>(null)

/** 相对工作区根的路径 → 该层的节点 */
const children = ref<Map<string, TreeNode[]>>(new Map())
/** 已展开的相对路径 */
const expanded = ref<Set<string>>(new Set())
/** 正在加载的相对路径，避免重复请求 */
const pending = ref<Set<string>>(new Set())
const selected = ref<string | null>(null)

const rootChildren = computed(() => children.value.get('') ?? [])

async function loadLevel(rel: string): Promise<void> {
  if (pending.value.has(rel)) return
  pending.value = new Set(pending.value).add(rel)
  loading.value = true
  try {
    const list = await wsApi.listDir(props.workspaceId, rel, showIgnored.value)
    children.value = new Map(children.value).set(rel, list)
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    toast.error(error.value)
  } finally {
    const next = new Set(pending.value)
    next.delete(rel)
    pending.value = next
    loading.value = false
  }
}

function childrenOf(rel: string): TreeNode[] | undefined {
  return children.value.get(rel)
}

function toggle(payload: { node: TreeNode; rel: string }): void {
  const { node, rel } = payload
  if (!node.isDir) {
    selected.value = rel
    emit('open-file', node.path)
    return
  }
  const next = new Set(expanded.value)
  if (next.has(rel)) {
    next.delete(rel)
  } else {
    next.add(rel)
    if (!children.value.has(rel)) void loadLevel(rel)
  }
  expanded.value = next
}

/**
 * 重新拉取已展开的层级。
 *
 * 保留展开状态——用户展开了几层目录，刷新后应该还是那几层。
 * 只对「当前仍展开的路径」重新请求，没展开过的不用管。
 */
async function refresh(): Promise<void> {
  const toLoad = ['', ...expanded.value]
  const next = new Map<string, TreeNode[]>()
  for (const rel of toLoad) {
    try {
      next.set(rel, await wsApi.listDir(props.workspaceId, rel, showIgnored.value))
    } catch {
      // 某一层没了（目录被删）就跳过，不整棵树报错
    }
  }
  children.value = next
  // 展开过但已经不存在了的路径，从 expanded 里剔掉
  expanded.value = new Set([...expanded.value].filter((r) => next.has(r)))
}

async function toggleIgnored(): Promise<void> {
  showIgnored.value = !showIgnored.value
  // 忽略开关会改变可见项，展开状态也一并重置
  children.value = new Map()
  expanded.value = new Set()
  await refresh()
}

watch(
  () => props.workspaceId,
  () => {
    void refresh()
  },
  { immediate: true }
)

defineExpose({ refresh })
</script>

<template>
  <aside class="drawer">
    <div class="dr-head">
      <span class="dr-title">文件</span>
      <span class="dr-tools">
        <button
          class="dr-btn"
          :class="{ on: showIgnored }"
          :title="showIgnored ? '隐藏依赖与构建目录' : '显示依赖与构建目录'"
          @click="toggleIgnored"
        >
          <svg viewBox="0 0 24 24">
            <path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12z" />
            <circle cx="12" cy="12" r="3" />
          </svg>
        </button>
        <button class="dr-btn" title="刷新" @click="refresh">
          <svg viewBox="0 0 24 24"><path d="M20 11a8 8 0 10-2.3 5.7" /><path d="M20 5v6h-6" /></svg>
        </button>
        <button class="dr-btn" title="收起" @click="emit('close')">
          <svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18" /></svg>
        </button>
      </span>
    </div>

    <div class="dr-path" :title="props.rootPath">{{ props.rootPath }}</div>

    <div class="dr-tree">
      <p v-if="error" class="dr-msg err">{{ error }}</p>
      <p v-else-if="loading && !rootChildren.length" class="dr-msg">加载中…</p>
      <p v-else-if="!rootChildren.length" class="dr-msg">这个目录是空的</p>

      <template v-else>
        <FileTreeNode
          v-for="node in rootChildren"
          :key="node.path"
          :node="node"
          :rel="node.name"
          :depth="0"
          :expanded="expanded"
          :children-of="childrenOf"
          :selected="selected"
          @toggle="toggle"
          @open="(p) => emit('open-file', p)"
        />
      </template>
    </div>
  </aside>
</template>

<style scoped>
.drawer {
  width: 280px;
  flex: none;
  display: flex;
  flex-direction: column;
  min-height: 0;
  border-left: 1px solid var(--line);
  background: var(--surface);
}

.dr-head {
  flex: none;
  height: 44px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 8px 0 12px;
  border-bottom: 1px solid var(--line);
}

.dr-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--t2);
}

.dr-tools {
  margin-left: auto;
  display: flex;
  gap: 2px;
}

.dr-btn {
  width: 24px;
  height: 24px;
  border-radius: 5px;
  display: grid;
  place-items: center;
  color: var(--t4);
}

.dr-btn:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.dr-btn.on {
  background: var(--green-soft);
  color: var(--green-t);
}

.dr-btn svg {
  width: 14px;
  height: 14px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.7;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.dr-path {
  flex: none;
  padding: 8px 12px;
  font-family: var(--mono);
  font-size: 10.5px;
  color: var(--t4);
  border-bottom: 1px solid var(--line);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.dr-tree {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  /* 底部留白：窗口矮的时候最后一项会贴着视口边缘 */
  padding: 6px 0 14px;
}

.dr-msg {
  padding: 18px 14px;
  font-size: 12px;
  color: var(--t4);
  line-height: 1.6;
}

.dr-msg.err {
  color: var(--red);
}

.node {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  height: 26px;
  padding-right: 10px;
  font-size: 12.5px;
  color: var(--t2);
  text-align: left;
  white-space: nowrap;
}

.node:hover {
  background: var(--bg-hover);
}

.node.on {
  background: var(--green-soft);
  color: var(--green-t);
}

.node .tri {
  width: 12px;
  height: 12px;
  flex: none;
  color: var(--t4);
  display: grid;
  place-items: center;
}

.node .tri svg {
  width: 9px;
  height: 9px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.node .tri.leaf {
  visibility: hidden;
}

.node .ni {
  width: 14px;
  height: 14px;
  flex: none;
  display: grid;
  place-items: center;
}

.node .ni svg {
  width: 14px;
  height: 14px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.node .nm {
  overflow: hidden;
  text-overflow: ellipsis;
}

.node .sz {
  margin-left: auto;
  font-size: 10.5px;
  color: var(--t4);
  flex: none;
  padding-left: 8px;
}

.node.d1 {
  padding-left: 10px;
}

.node.d2 {
  padding-left: 26px;
}

.node.d3 {
  padding-left: 42px;
}
</style>
