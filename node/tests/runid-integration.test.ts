import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

/**
 * 防回归：send() 内部只能有一个 runId 声明。
 *
 * 之前出现过内层 `const runId = ++runSeq` 遮蔽外层的情况：
 * start/done 用外层 id、text/tool 用内层 id，渲染层把所有增量
 * 当过期事件丢弃，表现为整轮只显示"正在思考"。
 *
 * 手写镜像的单元测试测不到这种作用域问题，只能对源码做结构断言。
 */
describe('chat.ts runId 作用域', () => {
  const src = readFileSync(join(process.cwd(), 'src/main/services/chat.ts'), 'utf-8')

  function bodyOf(fnName: string): string {
    const start = src.indexOf(`export async function ${fnName}(`)
    expect(start).toBeGreaterThan(-1)
    // 找到下一个顶层 export 作为结束
    const rest = src.slice(start + 10)
    const nextExport = rest.indexOf('\nexport ')
    return nextExport === -1 ? rest : rest.slice(0, nextExport)
  }

  it('send() 里只声明一次 runId', () => {
    const body = bodyOf('send')
    const matches = body.match(/const runId = \+\+runSeq/g) ?? []
    expect(matches).toHaveLength(1)
  })

  it('compact() 里只声明一次 runId', () => {
    const body = bodyOf('compact')
    const matches = body.match(/const runId = \+\+runSeq/g) ?? []
    expect(matches).toHaveLength(1)
  })

  it('send() 里所有 emit 都带 runId', () => {
    const body = bodyOf('send')
    // 抓出所有 emit({ ... }) 块（含多行）
    const emits = body.match(/emit\(\{[\s\S]*?\}\)/g) ?? []
    expect(emits.length).toBeGreaterThan(5)
    for (const e of emits) {
      expect(e).toContain('runId')
    }
  })
})
