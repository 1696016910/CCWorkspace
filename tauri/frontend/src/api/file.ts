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
