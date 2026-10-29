<script setup lang="ts">
import { onMounted, ref } from 'vue'
import * as api from '@/api/skill'
import type { SkillInfo } from '@/api/types'
import { useToast } from '@/composables/useToast'
import ConfirmDialog from '../dialogs/ConfirmDialog.vue'
import DocEditor from '../dialogs/DocEditor.vue'

/**
 * Skill 分区。
 *
 * 原来的启用开关已删除：它只写应用自己的数据库，claude 子进程从不读取，
 * 开或关对实际行为没有任何影响。保留一个不生效的开关比没有开关更糟。
 */
const toast = useToast()
const list = ref<SkillInfo[]>([])
const loading = ref(true)

const editing = ref<{ name: string; source: string; isNew: boolean } | null>(null)
const pendingDelete = ref<SkillInfo | null>(null)

async function load(): Promise<void> {
  loading.value = true
  try {
    list.value = await api.list()
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  } finally {
    loading.value = false
  }
}

function startNew(): void {
  editing.value = { name: '', source: 'user', isNew: true }
}

function startEdit(s: SkillInfo): void {
  editing.value = { name: s.name, source: s.source, isNew: false }
}

async function loadDoc(): Promise<string> {
  const e = editing.value
  if (!e || e.isNew) {
    return '---\nname: \ndescription: \n---\n\n'
  }
  return api.read(e.name, e.source)
}

async function saveDoc(content: string, name: string): Promise<void> {
  const e = editing.value
  if (!e) return
  await api.write(name, e.source, content)
  editing.value = null
  toast.success('已保存')
  await load()
}

async function confirmDelete(): Promise<void> {
  const s = pendingDelete.value
  pendingDelete.value = null
  if (!s) return
  try {
    await api.remove(s.name, s.source)
    toast.success('已删除')
    await load()
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  }
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">Skill</h1>
    <p class="st-subx">扫描 ~/.claude/skills 与工作区下的 .claude/skills。</p>

    <p v-if="loading" class="hint">加载中…</p>
    <p v-else-if="list.length === 0" class="hint">未找到任何 Skill</p>

    <div v-else class="grid">
      <div v-for="s in list" :key="s.source + s.name" class="card">
        <div class="cn">{{ s.name }}</div>
        <div class="cd">{{ s.description || '（无描述）' }}</div>
        <div class="cf">
          <span class="dot" :class="{ proj: s.source === 'project' }" />
          <span>{{ s.source === 'project' ? '项目' : '用户' }}</span>
          <span class="sp" />
          <button class="mini" title="查看" @click="startEdit(s)">查看</button>
          <button class="mini del" title="删除" @click="pendingDelete = s">删除</button>
        </div>
      </div>
    </div>

    <button class="add-row" @click="startNew">＋ 新建 Skill</button>

    <DocEditor
      v-if="editing"
      :title="editing.isNew ? '新建 Skill' : `Skill：${editing.name}`"
      :name="editing.name"
      :name-editable="editing.isNew"
      :loader="loadDoc"
      @save="saveDoc"
      @close="editing = null"
    />

    <ConfirmDialog
      v-if="pendingDelete"
      :title="`删除 Skill「${pendingDelete.name}」？`"
      :message="
        pendingDelete.extraFiles
          ? `该目录下还有 ${pendingDelete.extraFiles} 个附属文件，会一并删除。`
          : '该 Skill 目录会被整体删除。'
      "
      warning="删除后无法恢复。"
      confirm-text="删除"
      tone="danger"
      @confirm="confirmDelete"
      @cancel="pendingDelete = null"
    />
  </div>
</template>

<style scoped>
.hint {
  font-size: 12px;
  color: var(--t4);
}

.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(clamp(220px, 24vw, 300px), 1fr));
  gap: 10px;
}

.card {
  display: flex;
  flex-direction: column;
  padding: 13px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: var(--surface);
  transition: border-color 0.12s, box-shadow 0.12s;
}

.card:hover {
  border-color: var(--line-3);
  box-shadow: 0 1px 3px rgba(31, 31, 35, 0.05);
}

.cn {
  font-size: 12.5px;
  font-weight: 500;
  margin-bottom: 4px;
}

.cd {
  font-size: 11px;
  color: var(--t4);
  line-height: 1.5;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  min-height: 33px;
}

.cf {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-top: 9px;
  padding-top: 8px;
  border-top: 1px solid var(--line);
  font-size: 11px;
  color: var(--t4);
}

.dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--green);
  flex: none;
}

.dot.proj {
  background: var(--blue);
}

.sp {
  flex: 1;
}

.mini {
  height: 20px;
  padding: 0 7px;
  border-radius: 4px;
  font-size: 11px;
  color: var(--t3);
}

.mini:hover {
  background: var(--bg-hover);
  color: var(--t1);
}

.mini.del:hover {
  color: var(--red);
}

.add-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  width: 100%;
  height: 32px;
  margin-top: 10px;
  border: 1px dashed var(--line-3);
  border-radius: 8px;
  font-size: 12px;
  color: var(--t3);
  background: var(--add-row-bg);
}

.add-row:hover {
  border-color: var(--add-row-line);
  color: var(--t2);
  background: var(--bg-soft);
}
</style>
