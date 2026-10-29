<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import * as api from '@/api/config'
import { useToast } from '@/composables/useToast'

/**
 * 权限分区。
 *
 * 允许/拒绝各自独立成表，逐条增删。原来是一个 textarea、每行一条规则：
 * 加一条要移动光标，改一条要小心别碰坏别的行，也完全没法提示重复。
 */
const toast = useToast()
const defaultMode = ref('default')
const allow = ref<string[]>([])
const deny = ref<string[]>([])
const saving = ref(false)

const MODES = ['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions']

async function load(): Promise<void> {
  try {
    const p = await api.readPermissions()
    defaultMode.value = p.defaultMode
    allow.value = p.allow
    deny.value = p.deny
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  }
}

/** 空行与非空行的下标，用来标红与阻止保存 */
function emptyIndexes(list: string[]): Set<number> {
  const out = new Set<number>()
  list.forEach((r, i) => {
    if (!r.trim()) out.add(i)
  })
  return out
}

/** 重复行的下标（后出现的那些） */
function dupIndexes(list: string[]): Set<number> {
  const seen = new Set<string>()
  const out = new Set<number>()
  list.forEach((r, i) => {
    const key = r.trim()
    if (!key) return
    if (seen.has(key)) out.add(i)
    else seen.add(key)
  })
  return out
}

const allowEmpty = computed(() => emptyIndexes(allow.value))
const allowDup = computed(() => dupIndexes(allow.value))
const denyEmpty = computed(() => emptyIndexes(deny.value))
const denyDup = computed(() => dupIndexes(deny.value))

const blocked = computed(
  () =>
    allowEmpty.value.size > 0 ||
    allowDup.value.size > 0 ||
    denyEmpty.value.size > 0 ||
    denyDup.value.size > 0
)

const blockReason = computed(() => {
  if (allowEmpty.value.size || denyEmpty.value.size) return '存在空规则，请填写或删除'
  if (allowDup.value.size || denyDup.value.size) return '存在重复规则，请删除多余的一条'
  return ''
})

async function save(): Promise<void> {
  if (blocked.value) return
  saving.value = true
  try {
    await api.savePermissions({
      defaultMode: defaultMode.value,
      allow: allow.value.map((s) => s.trim()).filter(Boolean),
      deny: deny.value.map((s) => s.trim()).filter(Boolean)
    })
    toast.success('已保存')
  } catch (e) {
    toast.error(e instanceof Error ? e.message : String(e))
  } finally {
    saving.value = false
  }
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">权限</h1>
    <p class="st-subx">控制 Claude 执行工具时的确认行为。默认模式决定新会话的起点。</p>

    <div class="card">
      <div class="row">
        <span>默认模式</span>
        <select v-model="defaultMode" class="sel">
          <option v-for="m in MODES" :key="m" :value="m">{{ m }}</option>
        </select>
      </div>
    </div>

    <div class="block">
      <h3>允许规则</h3>
      <div class="tbl">
        <div class="thead"><div>规则</div><div></div></div>
        <div v-for="(_, i) in allow" :key="`a${i}`" class="tr" :class="{ bad: allowEmpty.has(i) || allowDup.has(i) }">
          <input v-model="allow[i]" placeholder="Bash(git diff:*)" spellcheck="false" />
          <button class="mbtn del" title="删除" @click="allow.splice(i, 1)">✕</button>
        </div>
        <div v-if="!allow.length" class="empty">还没有允许规则</div>
      </div>
      <button class="add-row" @click="allow.push('')">＋ 添加允许规则</button>
    </div>

    <div class="block">
      <h3>拒绝规则</h3>
      <div class="tbl">
        <div class="thead"><div>规则</div><div></div></div>
        <div v-for="(_, i) in deny" :key="`d${i}`" class="tr" :class="{ bad: denyEmpty.has(i) || denyDup.has(i) }">
          <input v-model="deny[i]" placeholder="Read(./.env)" spellcheck="false" />
          <button class="mbtn del" title="删除" @click="deny.splice(i, 1)">✕</button>
        </div>
        <div v-if="!deny.length" class="empty">还没有拒绝规则</div>
      </div>
      <button class="add-row" @click="deny.push('')">＋ 添加拒绝规则</button>
    </div>

    <p v-if="blockReason" class="st-err">{{ blockReason }}</p>

    <div class="save-row">
      <span class="sp" />
      <button class="st-btn pri" :disabled="saving || blocked" @click="save">
        {{ saving ? '保存中…' : '保存' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.card {
  border: 1px solid var(--line-2);
  border-radius: 10px;
  background: var(--surface);
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px;
  font-size: 13px;
}

.sel {
  height: 30px;
  padding: 0 8px;
  border: 1px solid var(--line-2);
  border-radius: var(--r);
  background: var(--surface);
  font-size: 12.5px;
  color: var(--t1);
}

.block {
  margin-top: 18px;
}

.block h3 {
  font-size: 12.5px;
  font-weight: 500;
  color: var(--t2);
  margin-bottom: 8px;
}

.tbl {
  border: 1px solid var(--line-2);
  border-radius: 10px;
  overflow: hidden;
  background: var(--surface);
}

.thead,
.tr {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 60px;
  align-items: center;
}

.thead {
  padding: 8px 12px;
  background: var(--bg-bar);
  border-bottom: 1px solid var(--line-2);
  font-size: 11.5px;
  color: var(--t3);
}

.tr {
  border-bottom: 1px solid var(--line);
}

.tr:last-child {
  border-bottom: none;
}

.tr:hover {
  background: var(--surface-2);
}

.tr.bad {
  background: var(--danger-bg);
}

.tr input {
  width: 100%;
  padding: 10px 12px;
  font-family: var(--mono);
  font-size: 12px;
  color: var(--t1);
  background: transparent;
}

.empty {
  padding: 22px;
  text-align: center;
  font-size: 12px;
  color: var(--t4);
}

.mbtn {
  width: 21px;
  height: 21px;
  margin: 0 auto;
  border-radius: 4px;
  color: var(--t4);
  font-size: 10.5px;
  display: grid;
  place-items: center;
}

.mbtn:hover {
  background: var(--bg-hover);
  color: var(--t2);
}

.mbtn.del:hover {
  color: var(--red);
}

.add-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  width: 100%;
  height: 32px;
  margin-top: 8px;
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

.save-row {
  display: flex;
  gap: 10px;
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
}

.save-row .sp {
  flex: 1;
}
</style>
