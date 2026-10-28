<script setup lang="ts">
import { onMounted, ref } from 'vue'
import * as api from '@/api/skill'
import type { SkillInfo } from '@/api/types'
import { useToast } from '@/composables/useToast'

const toast = useToast()
const list = ref<SkillInfo[]>([])
const loading = ref(true)

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

async function toggle(s: SkillInfo): Promise<void> {
  try {
    list.value = await api.toggle(s.name, !s.enabled)
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  }
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">Skill</h1>
    <p class="st-subx">扫描 ~/.claude/skills。开关只影响新会话。</p>

    <p v-if="loading" class="hint">加载中…</p>
    <p v-else-if="list.length === 0" class="hint">未找到任何 Skill</p>

    <div v-else class="grid">
      <div v-for="s in list" :key="s.name" class="card">
        <div class="cn">{{ s.name }}</div>
        <div class="cd">{{ s.description || '（无描述）' }}</div>
        <div class="cf">
          <span class="dot" :class="{ off: !s.enabled }" />
          <span :class="{ ok: s.enabled }">{{ s.enabled ? '已启用' : '已禁用' }}</span>
          <span class="sp" />
          <button class="mini" @click="toggle(s)">
            <span class="toggle" :class="{ on: s.enabled }" />
          </button>
        </div>
      </div>
    </div>
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

.dot.off {
  background: #c6c6cb;
}

.ok {
  color: var(--green-t);
}

.sp {
  flex: 1;
}

.mini {
  display: grid;
  place-items: center;
  width: 34px;
  height: 20px;
  border-radius: 4px;
}

.mini:hover {
  background: var(--bg-hover);
}
</style>
