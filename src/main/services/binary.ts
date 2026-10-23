import { existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { app } from 'electron'

/**
 * 定位 Agent SDK 自带的 claude 可执行文件。
 *
 * 打包后 SDK 会从 app.asar 内查找，但二进制被 asarUnpack 解到
 * app.asar.unpacked，导致 SDK 拿到一个不可执行的路径。这里显式给出
 * 解包后的真实路径，避免依赖 SDK 的默认探测。
 */
export function resolveClaudeBinary(): string | undefined {
  const platform = process.platform
  const arch = process.arch
  const dir = join(
    '@anthropic-ai',
    `claude-agent-sdk-${platform}-${arch}`
  )
  const exe = platform === 'win32' ? 'claude.exe' : 'claude'

  const candidates: string[] = []

  if (app.isPackaged) {
    // 打包后：resources/app.asar.unpacked/node_modules/...
    const resources = dirname(app.getAppPath())
    candidates.push(join(resources, 'app.asar.unpacked', 'node_modules', dir, exe))
  }

  // 开发态：项目 node_modules
  candidates.push(join(app.getAppPath(), 'node_modules', dir, exe))
  candidates.push(join(process.cwd(), 'node_modules', dir, exe))

  for (const c of candidates) {
    if (existsSync(c)) return c
  }
  return undefined
}
