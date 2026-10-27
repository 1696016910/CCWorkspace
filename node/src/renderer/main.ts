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
  console.error('[renderer] error:', e.error ?? e.message)
})

window.addEventListener('unhandledrejection', (e) => {
  console.error('[renderer] unhandledRejection:', e.reason)
})

const app = createApp(App)

// 组件渲染/生命周期抛错：记录后继续跑，避免整页白屏
app.config.errorHandler = (err, _instance, info) => {
  console.error(`[renderer] vue error (${info}):`, err)
}

app.use(createPinia()).mount('#app')
