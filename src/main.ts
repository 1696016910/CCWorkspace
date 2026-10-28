import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import './styles/global.css'

/**
 * 未捕获的渲染层异常至少要留下痕迹。
 *
 * 没有这两个兜底时，组件事件里漏掉的 await rejection 完全静默，
 * 界面毫无反应，用户只能看到"点了没效果"。
 */
window.addEventListener('error', (e) => {
  console.error('[tauri] error:', e.error ?? e.message)
})

window.addEventListener('unhandledrejection', (e) => {
  console.error('[tauri] unhandledRejection:', e.reason)
})

const app = createApp(App)
app.config.errorHandler = (err, _instance, info) => {
  console.error(`[tauri] vue error (${info}):`, err)
}
app.use(createPinia()).mount('#app')
