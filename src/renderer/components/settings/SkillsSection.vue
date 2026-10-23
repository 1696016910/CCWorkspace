<script setup lang="ts">
import { onMounted, ref } from 'vue'
import type { SkillInfo } from '@shared/types'

const skills = ref<SkillInfo[]>([])
const loading = ref(true)
const error = ref<string | null>(null)

async function load(): Promise<void> {
  loading.value = true
  try {
    skills.value = await window.api.skill.list()
    error.value = null
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function toggle(s: SkillInfo): Promise<void> {
  try {
    skills.value = await window.api.skill.toggle(s.name, !s.enabled)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

function initials(name: string): string {
  return name.slice(0, 2).toUpperCase()
}

const SOURCE_LABEL: Record<SkillInfo['source'], string> = {
  user: '用户级',
  project: '项目级',
  plugin: '插件'
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">Skill</h1>
    <p class="st-subx">
      扫描 <code>~/.claude/skills</code> 与项目内的 <code>.claude/skills</code>。开关只影响新会话，不影响已在运行的会话。
    </p>

    <p v-if="loading" class="hint">加载中…</p>
    <p v-else-if="error" class="st-err">{{ error }}</p>

    <div v-else-if="skills.length === 0" class="empty-state">
      <p class="en">没有发现 Skill</p>
      <p class="ed">在 ~/.claude/skills 或项目内 .claude/skills 下放置 SKILL.md 即可</p>
    </div>

    <div v-else class="grid">
      <div v-for="s in skills" :key="s.name" class="card">
        <div class="ico">{{ initials(s.name) }}</div>
        <div class="body">
          <div class="cn">{{ s.name }}</div>
          <div class="cv">{{ SOURCE_LABEL[s.source] }}</div>
          <div class="cd">{{ s.description || '（无描述）' }}</div>
          <div class="cf">
            <span class="dot" :class="{ off: !s.enabled }" />
            <span :class="{ ok: s.enabled }">{{ s.enabled ? '启用' : '禁用' }}</span>
            <span class="sp" />
            <span class="toggle" :class="{ on: s.enabled }" @click="toggle(s)" />
          </div>
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
  grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
  gap: 9px;
}

.card {
  display: flex;
  gap: 11px;
  padding: 13px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: #fff;
  transition: border-color 0.12s, box-shadow 0.12s;
}

.card:hover {
  border-color: var(--line-3);
  box-shadow: 0 1px 3px rgba(31, 31, 35, 0.05);
}

.ico {
  width: 30px;
  height: 30px;
  border-radius: 6px;
  flex: none;
  background: var(--bg-soft);
  border: 1px solid var(--line);
  display: grid;
  place-items: center;
  font-family: var(--mono);
  font-size: 9.5px;
  color: var(--t2);
}

.body {
  flex: 1;
  min-width: 0;
}

.cn {
  font-size: 12.5px;
  font-weight: 500;
}

.cv {
  font-size: 11px;
  color: var(--t4);
  margin-top: 1px;
}

.cd {
  font-size: 11.5px;
  color: var(--t3);
  line-height: 1.55;
  margin-top: 5px;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
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

.cf .dot {
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--green);
  flex: none;
}

.cf .dot.off {
  background: #c6c6cb;
}

.cf .ok {
  color: var(--green-t);
}

.cf .sp {
  flex: 1;
}

.empty-state {
  padding: 56px 20px;
  text-align: center;
}

.empty-state .en {
  font-size: 13px;
  color: var(--t2);
  margin-bottom: 4px;
}

.empty-state .ed {
  font-size: 12px;
  color: var(--t4);
}
</style>
