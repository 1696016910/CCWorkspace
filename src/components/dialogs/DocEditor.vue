<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'

/**
 * Markdown 文档编辑弹窗。
 *
 * Skill 与规则都是「一个名字 + 一段 markdown 正文」，共用这一个编辑器。
 * 名称只在新建时可改：改名等于换文件，已经存在的不允许在这里改。
 */
const props = defineProps<{
  title: string
  name: string
  nameEditable: boolean
  /** 异步取正文，打开弹窗时调用 */
  loader: () => Promise<string>
}>()

const emit = defineEmits<{
  close: []
  save: [content: string, name: string]
}>()

const draftName = ref(props.name)
const content = ref('')
const loading = ref(true)
const error = ref<string | null>(null)
const saving = ref(false)

async function load(): Promise<void> {
  loading.value = true
  try {
    content.value = await props.loader()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    loading.value = false
  }
}

async function save(): Promise<void> {
  const n = draftName.value.trim()
  if (!n) {
    error.value = '名称不能为空'
    return
  }
  if (!content.value.trim()) {
    error.value = '内容不能为空'
    return
  }
  saving.value = true
  error.value = null
  try {
    emit('save', content.value, n)
  } finally {
    saving.value = false
  }
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === 'Escape') emit('close')
  // Ctrl+S 保存，符合编辑器的通用直觉
  if ((e.ctrlKey || e.metaKey) && e.key === 's') {
    e.preventDefault()
    void save()
  }
}

onMounted(() => {
  window.addEventListener('keydown', onKeydown)
  void load()
})
onUnmounted(() => window.removeEventListener('keydown', onKeydown))
</script>

<template>
  <div class="scrim" @click.self="emit('close')">
    <div class="editor">
      <div class="ehead">
        <span class="etitle">{{ props.title }}</span>
        <span class="sp" />
        <button class="x" title="关闭" @click="emit('close')">✕</button>
      </div>

      <div class="ebody">
        <label v-if="props.nameEditable" class="field">
          <span>名称</span>
          <input v-model="draftName" placeholder="kebab-case-name" />
        </label>

        <p v-if="loading" class="hint">加载中…</p>
        <textarea
          v-else
          v-model="content"
          class="content"
          spellcheck="false"
          placeholder="Markdown 正文"
        />

        <p v-if="error" class="err">{{ error }}</p>
      </div>

      <div class="efoot">
        <span class="hint">Ctrl + S 保存</span>
        <span class="sp" />
        <button class="btn" @click="emit('close')">取消</button>
        <button class="btn pri" :disabled="saving || loading" @click="save">保存</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.scrim {
  position: fixed;
  inset: 0;
  z-index: 120;
  background: rgba(10, 10, 14, 0.45);
  display: grid;
  place-items: center;
}

.editor {
  width: min(760px, 90vw);
  height: min(640px, 84vh);
  display: flex;
  flex-direction: column;
  background: var(--surface);
  border: 1px solid var(--line-2);
  border-radius: 12px;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.28);
  overflow: hidden;
}

.ehead {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--line);
  flex: none;
}

.etitle {
  font-size: 13px;
  font-weight: 500;
}

.sp {
  flex: 1;
}

.x {
  width: 24px;
  height: 24px;
  border-radius: 5px;
  color: var(--t3);
  font-size: 12px;
}

.x:hover {
  background: var(--bg-hover);
}

.ebody {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 14px 16px;
}

.field {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 12px;
  color: var(--t2);
  flex: none;
}

.field input {
  flex: 1;
  height: 30px;
  padding: 0 10px;
  border: 1px solid var(--line-2);
  border-radius: 6px;
  background: var(--surface);
  font-family: var(--mono);
  font-size: 12px;
  color: var(--t1);
}

.content {
  flex: 1;
  min-height: 0;
  padding: 10px 12px;
  border: 1px solid var(--line-2);
  border-radius: 8px;
  background: var(--surface);
  font-family: var(--mono);
  font-size: 12px;
  line-height: 1.7;
  color: var(--t1);
  resize: none;
}

.hint {
  font-size: 11.5px;
  color: var(--t4);
}

.err {
  font-size: 12px;
  color: var(--red);
  flex: none;
}

.efoot {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-top: 1px solid var(--line);
  flex: none;
}

.btn {
  height: 30px;
  padding: 0 14px;
  border-radius: 6px;
  border: 1px solid var(--line-3);
  background: var(--btn-bg);
  font-size: 12px;
  color: var(--t1);
}

.btn:hover {
  background: var(--bg-hover);
}

.btn.pri {
  background: var(--green);
  border-color: var(--green);
  color: #fff;
}

.btn.pri:hover {
  background: var(--green-h);
}

.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
