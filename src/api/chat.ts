import { call } from './invoke'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { Attachment, ChatEvent, CliMeta, CompactResult } from './types'

/** 发一轮对话。立即返回 runId，正文通过 onEvent 流式到达。 */
export function send(
  sessionId: string,
  text: string,
  attachments: Attachment[] = []
): Promise<number> {
  // 只有图片带 base64 发给模型；普通文件把路径写进文本，由模型自己读
  const images = attachments
    .filter((a) => a.kind === 'image' && a.data && a.mediaType)
    .map((a) => ({ mediaType: a.mediaType as string, data: a.data as string }))
  const filePaths = attachments.filter((a) => a.kind === 'file' && a.path).map((a) => a.path)
  const body = filePaths.length
    ? `${text}\n\n${filePaths.map((p) => `@${p}`).join('\n')}`.trim()
    : text
  // 附件元数据单独传：正文里拼的是给模型看的路径提示，
  // 元数据是给界面回显用的，两者不能混在一起
  const meta = attachments.map((a) => ({
    kind: a.kind,
    name: a.name,
    path: a.path ?? null,
    size: a.size
  }))
  return call('chat_send', { sessionId, text: body, images, attachments: meta })
}

/** 中断指定会话的当前这轮。并发下必须带目标，否则会停掉全部。 */
export function interrupt(sessionId: string): Promise<void> {
  return call('chat_interrupt', { sessionId })
}

/** 订阅对话事件，返回取消订阅函数。 */
export async function onEvent(handler: (e: ChatEvent) => void): Promise<UnlistenFn> {
  return listen<ChatEvent>('chat:event', (ev) => handler(ev.payload))
}

/** 订阅压缩结果。压缩在后台线程跑，结果经事件回传。 */
export async function onCompact(handler: (r: CompactResult) => void): Promise<UnlistenFn> {
  return listen<CompactResult>('chat:compact', (ev) => handler(ev.payload))
}

/** 压缩上下文。立即返回，结果看 onCompact。 */
export function compact(sessionId: string): Promise<void> {
  return call('context_compact', { sessionId })
}

/** CLI 上报的可用能力清单。首轮对话前为 null。 */
export function meta(): Promise<CliMeta | null> {
  return call('cli_meta')
}
