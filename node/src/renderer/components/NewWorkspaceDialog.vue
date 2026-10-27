<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { useWorkspaceStore } from '@/stores/workspace'

const emit = defineEmits<{ close: []; created: [] }>()

const ws = useWorkspaceStore()
const dir = ref('')
const name = ref('')
const saving = ref(false)
const error = ref<string | null>(null)
const nameInput = ref<HTMLInputElement | null>(null)

/** 目录和名称都填了才能创建 */
const canCreate = computed(() => dir.value.trim().length > 0 && name.value.trim().length > 0)

async function pick(): Promise<void> {
  const picked = await window.api.workspace.pick()
  if (!picked) return
  dir.value = picked
  // 选好目录后自动用目录名填名称，用户可改
  if (!name.value.trim()) {
    const base = picked.split(/[\\/]/).filter(Boolean).pop() ?? ''
    name.value = base
  }
  void nextTick(() => nameInput.value?.focus())
}

async function create(): Promise<void> {
  if (!canCreate.value || saving.value) return
  saving.value = true
  error.value = null
  try {
    await ws.add(dir.value.trim(), name.value.trim())
    emit('created')
    emit('close')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') {
    e.preventDefault()
    emit('close')
  } else if (e.key === 'Enter' && canCreate.value) {
    e.preventDefault()
    void create()
  }
}

onMounted(() => window.addEventListener('keydown', onKeydown))
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="mask" @click.self="emit('close')">
    <div class="dialog" role="dialog" aria-modal="true">
      <div class="head">
        <div class="title">新建工作区</div>
        <div class="sub">选择一个项目目录，并给它起个名字</div>
      </div>

      <div class="body">
        <div class="field">
          <label class="label">项目目录</label>
          <div class="path-row">
            <input
              class="input readonly"
              :value="dir"
              placeholder="尚未选择目录"
              readonly
            />
            <button class="btn-pick" type="button" @click="pick">选择</button>
          </div>
          <div class="hint">选好后会自动填入名称，你也可以修改</div>
        </div>

        <div class="field">
          <label class="label">工作区名称</label>
          <input
            ref="nameInput"
            v-model="name"
            class="input"
            placeholder="给这个工作区起个名字"
          />
        </div>

        <p v-if="error" class="err">{{ error }}</p>
      </div>

      <div class="foot">
        <button class="btn" type="button" @click="emit('close')">取消</button>
        <button class="btn primary" type="button" :disabled="!canCreate || saving" @click="create">
          {{ saving ? '创建中…' : '创建' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask {
  position: fixed;
  inset: 0;
  z-index: 200;
  background: rgba(10, 10, 14, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  animation: fade 0.14s ease;
}

@keyframes fade {
  from { opacity: 0; }
  to { opacity: 1; }
}

.dialog {
  width: 460px;
  max-width: calc(100vw - 48px);
  background: var(--surface);
  border-radius: 14px;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.18), 0 2px 8px rgba(0, 0, 0, 0.06);
  overflow: hidden;
  animation: pop 0.16s ease;
}

@keyframes pop {
  from { opacity: 0; transform: scale(0.97); }
  to { opacity: 1; transform: scale(1); }
}

.head {
  padding: 24px 24px 0;
}

.title {
  font-size: 16px;
  font-weight: 600;
  color: var(--t1);
  letter-spacing: -0.1px;
}

.sub {
  font-size: 12.5px;
  color: var(--t4);
  margin-top: 6px;
}

.body {
  padding: 20px 24px 24px;
}

.field {
  margin-bottom: 20px;
}

.field:last-of-type {
  margin-bottom: 0;
}

.label {
  display: block;
  font-size: 12.5px;
  font-weight: 500;
  color: var(--t2);
  margin-bottom: 8px;
}

.input {
  width: 100%;
  height: 40px;
  padding: 0 12px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: var(--surface);
  font-family: var(--font);
  font-size: 13px;
  color: var(--t1);
  transition: border-color 0.15s;
}

.input::placeholder {
  color: var(--t4);
}

.input:focus {
  outline: none;
  border-color: var(--green);
}

.input.readonly {
  background: var(--bg-soft);
  color: var(--t2);
  font-family: var(--mono);
  font-size: 12.5px;
  cursor: default;
  text-overflow: ellipsis;
}

.input.readonly:focus {
  border-color: var(--line-2);
}

.path-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.path-row .input {
  flex: 1;
  min-width: 0;
}

.btn-pick {
  flex: none;
  height: 40px;
  padding: 0 18px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: var(--surface);
  font-family: var(--font);
  font-size: 13px;
  color: var(--t1);
  white-space: nowrap;
  cursor: pointer;
  transition: background 0.15s, border-color 0.15s;
}

.btn-pick:hover {
  background: var(--bg-hover);
  border-color: var(--line-3);
}

.hint {
  font-size: 11.5px;
  color: var(--t4);
  margin-top: 7px;
}

.err {
  margin-top: 12px;
  font-size: 12px;
  color: var(--red);
}

.foot {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  padding: 16px 24px;
  background: var(--bg-bar);
  border-top: 1px solid var(--line);
}

.btn {
  height: 36px;
  padding: 0 20px;
  border-radius: 8px;
  border: 1px solid var(--line-2);
  background: var(--surface);
  font-family: var(--font);
  font-size: 13px;
  color: var(--t1);
  cursor: pointer;
  transition: background 0.14s;
}

.btn:hover {
  background: var(--bg-hover);
}

.btn.primary {
  background: var(--green);
  border-color: var(--green);
  color: #fff;
  font-weight: 500;
}

.btn.primary:hover {
  background: var(--green-h);
}

.btn.primary:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
</style>
