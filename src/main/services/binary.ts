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
function candidatesFor(suffix: string): string[] {
  const platform = process.platform
  const arch = process.arch
  const dir = join('@anthropic-ai', `claude-agent-sdk-${platform}-${arch}${suffix}`)
  const exe = platform === 'win32' ? 'claude.exe' : 'claude'
  const out: string[] = []

  if (app.isPackaged) {
    // 打包后：resources/app.asar.unpacked/node_modules/...
    const resources = dirname(app.getAppPath())
    out.push(join(resources, 'app.asar.unpacked', 'node_modules', dir, exe))
  }
  // 开发态：项目 node_modules
  out.push(join(app.getAppPath(), 'node_modules', dir, exe))
  out.push(join(process.cwd(), 'node_modules', dir, exe))
  return out
}

export function resolveClaudeBinary(): string | undefined {
  // Linux 上 Alpine 等 musl 发行版要用单独的 -musl 包
  const suffixes = process.platform === 'linux' ? ['', '-musl'] : ['']
  for (const suffix of suffixes) {
    for (const c of candidatesFor(suffix)) {
      if (existsSync(c)) return c
    }
  }
  return undefined
}

/**
 * 找不到二进制时给一句能指导排查的话。
 *
 * 直接把 undefined 传给 SDK 会让它回退到 asar 内的路径并报 spawn ENOENT，
 * 那个错误跟"平台包没装上"毫无关系，用户根本不会往这个方向想。
 */
export function describeBinaryMissing(): string {
  const dir = `claude-agent-sdk-${process.platform}-${process.arch}`
  return `未找到内置的 claude 可执行文件（缺少 ${dir}）。请重新安装应用；开发态请检查 node_modules 下是否装了对应的平台包。`
}
