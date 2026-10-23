/**
 * 渲染进程侧的路径规范化，仅用于 map 的 key 比较。
 * 与主进程 normalizePath 的语义保持一致：统一分隔符 + 小写。
 */
export function normalizePathKey(p: string): string {
  return p.replace(/\\/g, '/').toLowerCase()
}
