import { call } from './invoke'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { ChatEvent } from './types'

/** 发一轮对话。立即返回 runId，正文通过 onEvent 流式到达。 */
export function send(sessionId: string, text: string): Promise<number> {
  return call('chat_send', { sessionId, text })
}

/** 中断当前这轮。 */
export function interrupt(): Promise<void> {
  return call('chat_interrupt')
}

/** 订阅对话事件，返回取消订阅函数。 */
export async function onEvent(handler: (e: ChatEvent) => void): Promise<UnlistenFn> {
  return listen<ChatEvent>('chat:event', (ev) => handler(ev.payload))
}
