/**
 * Tauri IPC 的统一入口。
 *
 * 所有模块都从这里调 invoke，好处是：
 * - 命令名的下划线风格只在这一个文件出现
 * - 出错时统一转成 Error（Tauri 抛的是字符串，直接冒泡会让 .catch 拿到非 Error）
 */
import { invoke as tauriInvoke } from '@tauri-apps/api/core'

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args)
  } catch (e) {
    // Tauri 的 Err(String) 到这边是字符串，转成 Error 才能保留堆栈与 instanceof 判断
    throw e instanceof Error ? e : new Error(String(e))
  }
}
