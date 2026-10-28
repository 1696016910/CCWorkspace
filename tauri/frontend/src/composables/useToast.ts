import { ref } from 'vue'

export interface ToastItem {
  id: number
  text: string
  kind: 'info' | 'success' | 'error'
}

const items = ref<ToastItem[]>([])
let seq = 0

function push(text: string, kind: ToastItem['kind'], ms = 2600): void {
  const id = ++seq
  items.value = [...items.value, { id, text, kind }]
  setTimeout(() => {
    items.value = items.value.filter((t) => t.id !== id)
  }, ms)
}

/** 全局轻提示：成功 / 失败 / 普通信息 */
export function useToast() {
  return {
    items,
    info: (t: string) => push(t, 'info'),
    success: (t: string) => push(t, 'success'),
    error: (t: string) => push(t, 'error', 4000)
  }
}
