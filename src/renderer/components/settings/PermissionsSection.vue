<script setup lang="ts">
import { onMounted, ref } from 'vue'

const MODES = ['default', 'acceptEdits', 'plan', 'dontAsk', 'bypassPermissions']

const defaultMode = ref('default')
const allow = ref<string[]>([])
const deny = ref<string[]>([])
const newAllow = ref('')
const newDeny = ref('')
const saving = ref(false)
const message = ref<string | null>(null)

async function load(): Promise<void> {
  const p = await window.api.config.readPermissions()
  defaultMode.value = p.defaultMode
  allow.value = p.allow
  deny.value = p.deny
}

async function save(): Promise<void> {
  saving.value = true
  try {
    // allow/deny 是 Vue 响应式数组，直接传给 ipcRenderer 会因结构化克隆
    // 拒绝 Proxy 而报 "An object could not be cloned"。必须复制成普通数组。
    await window.api.config.savePermissions({
      defaultMode: defaultMode.value,
      allow: [...allow.value],
      deny: [...deny.value]
    })
    message.value = '已保存'
    setTimeout(() => (message.value = null), 3000)
  } catch (e) {
    message.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

function add(list: 'allow' | 'deny'): void {
  const target = list === 'allow' ? allow : deny
  const input = list === 'allow' ? newAllow : newDeny
  const value = input.value.trim()
  if (!value || target.value.includes(value)) return
  target.value = [...target.value, value]
  input.value = ''
}

function remove(list: 'allow' | 'deny', index: number): void {
  const target = list === 'allow' ? allow : deny
  target.value = target.value.filter((_, i) => i !== index)
}

onMounted(load)
</script>

<template>
  <div>
    <h1 class="st-h1x">权限</h1>
    <p class="st-subx">控制 Claude 可以执行哪些工具和命令。</p>

    <div v-if="defaultMode === 'bypassPermissions'" class="st-notice warn">
      <span>▲</span>
      <span>当前为 <b>bypassPermissions</b>：Claude 可执行任意命令、任意写文件，不再询问。建议改为 acceptEdits。</span>
    </div>

    <div class="st-blockx" style="margin-top: 18px">
      <h3>默认模式</h3>
      <div class="st-cardx">
        <div class="st-rowx">
          <div class="st-l">
            <div class="st-n">新建会话时使用</div>
            <div class="st-d">该值写入 settings.json 的 permissions.defaultMode</div>
          </div>
          <div class="st-r">
            <select v-model="defaultMode" class="select">
              <option v-for="m in MODES" :key="m" :value="m">{{ m }}</option>
            </select>
          </div>
        </div>
      </div>
    </div>

    <div class="st-blockx">
      <h3>允许规则 · {{ allow.length }}</h3>
      <div class="st-cardx">
        <div v-for="(r, i) in allow" :key="`a-${i}`" class="rule-row">
          <span class="mono">{{ r }}</span>
          <span class="sp" />
          <button class="mbtn del" @click="remove('allow', i)">✕</button>
        </div>
        <div v-if="allow.length === 0" class="rule-empty">暂无允许规则</div>
      </div>
      <div class="add-rule">
        <input v-model="newAllow" placeholder="例如 Bash(git *)" @keydown.enter="add('allow')" />
        <button class="st-btn" @click="add('allow')">添加</button>
      </div>
    </div>

    <div class="st-blockx">
      <h3>拒绝规则 · {{ deny.length }}</h3>
      <div class="st-cardx">
        <div v-for="(r, i) in deny" :key="`d-${i}`" class="rule-row">
          <span class="mono">{{ r }}</span>
          <span class="sp" />
          <button class="mbtn del" @click="remove('deny', i)">✕</button>
        </div>
        <div v-if="deny.length === 0" class="rule-empty">
          暂无拒绝规则。建议加入 Bash(rm -rf *) 之类的保护。
        </div>
      </div>
      <div class="add-rule">
        <input v-model="newDeny" placeholder="例如 Bash(rm -rf *)" @keydown.enter="add('deny')" />
        <button class="st-btn" @click="add('deny')">添加</button>
      </div>
    </div>

    <div class="save-row">
      <span class="sp" />
      <button class="st-btn pri" :disabled="saving" @click="save">
        {{ saving ? '保存中…' : '保存' }}
      </button>
    </div>
    <p v-if="message" class="st-err ok">{{ message }}</p>
  </div>
</template>

<style scoped>
.select {
  height: 34px;
  padding: 0 8px;
  border-radius: var(--r);
  border: 1px solid var(--line-3);
  background: var(--surface-2);
  font-size: 12px;
  color: var(--t1);
  font-family: inherit;
}

.rule-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 15px;
  border-bottom: 1px solid var(--line);
  font-size: 12px;
}

.rule-row:last-child {
  border-bottom: none;
}

.rule-row:hover {
  background: var(--surface-2);
}

.rule-row .mono {
  font-family: var(--mono);
  font-size: 11.5px;
  color: var(--t2);
}

.rule-row .sp {
  flex: 1;
}

.rule-empty {
  padding: 16px 15px;
  font-size: 12px;
  color: var(--t4);
}

.add-rule {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}

.add-rule input {
  flex: 1;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--line-3);
  border-radius: var(--r);
  font-family: var(--mono);
  font-size: 11.5px;
  background: var(--surface-2);
}

.add-rule input:focus {
  border-color: #bfc3c7;
}

.save-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 18px;
  padding-top: 14px;
  border-top: 1px solid var(--line);
}

.save-row .sp {
  flex: 1;
}

.mbtn {
  width: 21px;
  height: 21px;
  border-radius: 4px;
  color: #a0a0a7;
  font-size: 10.5px;
  display: grid;
  place-items: center;
}

.mbtn.del:hover {
  background: var(--danger-soft);
  color: var(--red);
}

.st-err.ok {
  color: var(--green-t);
}
</style>
