/**
 * 主题存 localStorage。
 *
 * Node 版把主题存在主进程（Electron 重启后要恢复），
 * Tauri 版没有等价的主进程状态，WebView 的 localStorage 足够且更简单。
 */
const KEY = 'cw-theme'

export type Theme = 'light' | 'dark'

export function get(): Theme {
  return localStorage.getItem(KEY) === 'dark' ? 'dark' : 'light'
}

export function set(t: Theme): void {
  localStorage.setItem(KEY, t)
  apply(t)
}

export function apply(t: Theme): void {
  document.documentElement.dataset.theme = t
}
