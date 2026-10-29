<script setup lang="ts">
import type { TreeNode } from '@/api/workspace'

/**
 * 文件树的一行。递归渲染子节点，避免模板里写死层数。
 *
 * 只有展开时才去问父组件要子节点（父组件负责缓存与请求）。
 */
const props = defineProps<{
  node: TreeNode
  /** 相对工作区根的路径，例如 `src/utils` */
  rel: string
  depth: number
  expanded: Set<string>
  childrenOf: (rel: string) => TreeNode[] | undefined
  selected: string | null
}>()

const emit = defineEmits<{
  toggle: [payload: { node: TreeNode; rel: string }]
  open: [path: string]
}>()

function fmtSize(n?: number): string {
  if (n === undefined) return ''
  if (n >= 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
  if (n >= 1024) return `${Math.round(n / 1024)} KB`
  return `${n} B`
}

const isOpen = () => props.expanded.has(props.rel)
</script>

<template>
  <button
    class="node"
    :class="{ on: props.selected === props.rel }"
    :style="{ paddingLeft: `${10 + props.depth * 16}px` }"
    :title="props.node.path"
    @click="emit('toggle', { node: props.node, rel: props.rel })"
  >
    <span class="tri" :class="{ leaf: !props.node.isDir }">
      <svg v-if="props.node.isDir" viewBox="0 0 24 24">
        <path :d="isOpen() ? 'M6 9l6 6 6-6' : 'M9 6l6 6-6 6'" />
      </svg>
    </span>
    <span class="ni">
      <svg v-if="props.node.isDir" viewBox="0 0 24 24">
        <path d="M3.5 7.5h6l1.8 2h9.2v9h-17v-11z" />
      </svg>
      <svg v-else viewBox="0 0 24 24">
        <path d="M6 3.5h7l5 5v12H6z" />
        <path d="M13 3.5v5h5" />
      </svg>
    </span>
    <span class="nm">{{ props.node.name }}</span>
    <span v-if="!props.node.isDir" class="sz">{{ fmtSize(props.node.size) }}</span>
  </button>

  <template v-if="props.node.isDir && isOpen()">
    <FileTreeNode
      v-for="c in childrenOf(props.rel) ?? []"
      :key="c.path"
      :node="c"
      :rel="`${props.rel}/${c.name}`"
      :depth="props.depth + 1"
      :expanded="props.expanded"
      :children-of="childrenOf"
      :selected="props.selected"
      @toggle="emit('toggle', $event)"
      @open="emit('open', $event)"
    />
  </template>
</template>

<style scoped>
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

.tri {
  width: 13px;
  height: 13px;
  flex: none;
  color: var(--t3);
  display: grid;
  place-items: center;
}

.tri svg {
  width: 11px;
  height: 11px;
  stroke: currentColor;
  fill: none;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.tri.leaf {
  visibility: hidden;
}

.ni {
  width: 14px;
  height: 14px;
  flex: none;
  display: grid;
  place-items: center;
  margin-left: 1px;
}

.ni svg {
  width: 14px;
  height: 14px;
  stroke: currentColor;
  fill: none;
  stroke-width: 1.6;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.nm {
  overflow: hidden;
  text-overflow: ellipsis;
}

.sz {
  margin-left: auto;
  font-size: 10.5px;
  color: var(--t4);
  flex: none;
  padding-left: 8px;
}
</style>
