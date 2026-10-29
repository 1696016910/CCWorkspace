import { call } from './invoke'

export interface FileContent {
  name: string
  path: string
  ext: string
  text: string
  size: number
}

/** 读取工作区内的文本文件；工作区外、二进制、超大文件都会被后端拒绝。 */
export function read(path: string): Promise<FileContent> {
  return call('file_read', { path })
}

export interface ImageData {
  name: string
  mediaType: string
  data: string
  preview: string
  size: number
}

/** 读本地图片并转 base64。路径来自 dialog 插件，是可信的绝对路径。 */
export function readImage(path: string): Promise<ImageData> {
  return call('file_image', { path })
}

/**
 * 批量判定路径是否在工作区内。
 *
 * 放在后端：前端手上的工作区路径可能已被移动或删除，按字面比较会误判。
 * 批量是因为历史会话可能有几十条消息，逐条调用会打出几十次 IPC。
 */
export function check(paths: string[]): Promise<boolean[]> {
  return call('file_check', { paths })
}
