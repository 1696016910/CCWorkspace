<script setup lang="ts">
import { computed } from 'vue'

/**
 * 斜杠命令补全面板。
 *
 * 清单来自 CLI 的 init 事件（slash_commands 143 条，含用户 Skill、用户命令、
 * 项目命令、插件命令与内置命令），扫目录只能拿到子集。
 */
interface SlashItem {
  name: string
  description: string
  group: string
}

const props = defineProps<{
  query: string
  items: SlashItem[]
  ready: boolean
}>()

const emit = defineEmits<{ pick: [name: string]; close: [] }>()

interface Group {
  name: string
  /** 一级项（没有命名空间） */
  plain: SlashItem[]
  /** 二级项：`pua:yes` 归到 `pua` 组 */
  nested: Map<string, SlashItem[]>
}

const GROUPS = ['Skill', 'Agent', '命令', '插件命令', '内置命令']

const filtered = computed(() => {
  const q = props.query.trim().toLowerCase()
  if (!q) return props.items
  return props.items.filter(
    (i) => i.name.toLowerCase().includes(q) || i.description.toLowerCase().includes(q)
  )
})

/** 按来源分组，带冒号的名字再按前缀压成二级 */
const groups = computed<Group[]>(() => {
  const out: Group[] = []
  for (const g of GROUPS) {
    const list = filtered.value.filter((i) => i.group === g)
    if (!list.length) continue
    const plain: SlashItem[] = []
    const nested = new Map<string, SlashItem[]>()
    for (const i of list) {
      const at = i.name.indexOf(':')
      if (at > 0) {
        const prefix = i.name.slice(0, at)
        const arr = nested.get(prefix) ?? []
        arr.push(i)
        nested.set(prefix, arr)
      } else if (!nested.has(i.name)) {
        plain.push(i)
      }
    }
    out.push({ name: g, plain, nested })
  }
  return out
})

const isEmpty = computed(() => filtered.value.length === 0)
</script>

<template>
  <div class="slash">
    <div class="s-head">
      <span class="s-title">命令与 Skill</span>
      <span class="sp" />
      <span v-if="query" class="s-q">/{{ query }}</span>
      <button class="s-x" title="关闭" @click="emit('close')">✕</button>
    </div>

    <p v-if="!ready" class="s-hint">发一条消息后可用。命令清单由 CLI 在首轮对话时上报。</p>
    <p v-else-if="isEmpty" class="s-hint">没有匹配的命令</p>

    <div v-else class="s-list">
      <template v-for="g in groups" :key="g.name">
        <div class="s-grp">{{ g.name }}</div>
        <button
          v-for="i in g.plain"
          :key="i.name"
          class="s-item"
          :title="i.description || `/${i.name}`"
          @click="emit('pick', i.name)"
        >
          <span class="s-name">/{{ i.name }}</span>
          <span v-if="i.description" class="s-desc">{{ i.description }}</span>
        </button>
        <template v-for="[prefix, arr] in g.nested" :key="g.name + prefix">
          <div class="s-sub">{{ prefix }}</div>
          <button
            v-for="i in arr"
            :key="i.name"
            class="s-item nested"
            :title="i.description || `/${i.name}`"
            @click="emit('pick', i.name)"
          >
            <span class="s-name">/{{ i.name }}</span>
            <span v-if="i.description" class="s-desc">{{ i.description }}</span>
          </button>
        </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.slash {
  position: absolute;
  left: 0;
  right: 0;
  bottom: calc(100% + 8px);
  max-height: 320px;
  display: flex;
  flex-direction: column;
  background: var(--surface);
  border: 1px solid var(--line-2);
  border-radius: 10px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  z-index: 30;
  overflow: hidden;
}

.s-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--line);
  flex: none;
}

.s-title {
  font-size: 12px;
  font-weight: 500;
  color: var(--t2);
}

.sp {
  flex: 1;
}

.s-q {
  font-family: var(--mono);
  font-size: 11px;
  color: var(--t4);
}

.s-x {
  width: 20px;
  height: 20px;
  border-radius: 4px;
  color: var(--t3);
  font-size: 11px;
}

.s-x:hover {
  background: var(--bg-hover);
}

.s-hint {
  padding: 18px 14px;
  font-size: 12px;
  color: var(--t4);
  line-height: 1.6;
}

.s-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  /* 底部留白：最后一项贴着滚动区边缘会被裁掉半行 */
  padding: 6px 0 10px;
}

.s-grp {
  padding: 6px 12px 4px;
  font-size: 10.5px;
  color: var(--t4);
}

.s-sub {
  padding: 4px 12px 2px 22px;
  font-size: 10.5px;
  color: var(--t4);
  font-family: var(--mono);
}

.s-item {
  display: flex;
  align-items: baseline;
  gap: 10px;
  width: 100%;
  padding: 5px 12px;
  text-align: left;
  font-size: 12px;
  color: var(--t1);
}

.s-item.nested {
  padding-left: 28px;
}

.s-item:hover {
  background: var(--bg-hover);
}

.s-name {
  font-family: var(--mono);
  flex: none;
  color: var(--blue);
}

.s-desc {
  font-size: 11px;
  color: var(--t4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
