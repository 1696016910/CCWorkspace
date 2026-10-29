import { describe, expect, it } from 'vitest'

/**
 * 文件树刷新的语义。
 *
 * 曾经的 bug：刷新会清空 expanded，用户展开的几层目录全部收起。
 * 对话结束后自动刷新时尤其烦人——看文件看到一半，树自己合上了。
 */

interface TreeNode {
  name: string
  path: string
  isDir: boolean
}

/** 模拟 refresh 的核心逻辑：保留展开状态、只重拉已展开的层 */
async function refreshTree(
  expanded: Set<string>,
  listDir: (rel: string) => Promise<TreeNode[]>
): Promise<{ children: Map<string, TreeNode[]>; expanded: Set<string> }> {
  const toLoad = ['', ...expanded]
  const next = new Map<string, TreeNode[]>()
  for (const rel of toLoad) {
    try {
      next.set(rel, await listDir(rel))
    } catch {
      // 目录被删了就跳过
    }
  }
  return {
    children: next,
    // 展开过但不存在的路径要剔掉
    expanded: new Set([...expanded].filter((r) => next.has(r)))
  }
}

const fakeTree: Record<string, TreeNode[]> = {
  '': [
    { name: 'src', path: '/src', isDir: true },
    { name: 'a.md', path: '/a.md', isDir: false }
  ],
  src: [{ name: 'main.ts', path: '/src/main.ts', isDir: false }],
  docs: [{ name: 'b.md', path: '/docs/b.md', isDir: false }]
}

describe('文件树刷新', () => {
  it('保留已展开的层级', async () => {
    const expanded = new Set(['src'])
    const { children, expanded: after } = await refreshTree(expanded, async (rel) => {
      if (!(rel in fakeTree)) throw new Error('no such dir')
      return fakeTree[rel]
    })

    expect(after.has('src')).toBe(true)
    expect(children.has('src')).toBe(true)
    expect(children.get('src')?.[0].name).toBe('main.ts')
  })

  it('目录消失后把它的展开状态剔掉', async () => {
    // 用户展开过 docs，但 docs 现在被删了
    const expanded = new Set(['src', 'docs'])
    const { expanded: after } = await refreshTree(expanded, async (rel) => {
      if (rel === 'docs') throw new Error('目录不存在')
      return fakeTree[rel] ?? []
    })

    expect(after.has('src')).toBe(true)
    expect(after.has('docs')).toBe(false)
  })

  it('根层永远重新拉取', async () => {
    let rootCalls = 0
    await refreshTree(new Set(), async (rel) => {
      if (rel === '') rootCalls++
      return []
    })
    expect(rootCalls).toBe(1)
  })

  it('只拉已展开的层，不碰没展开的', async () => {
    const calls: string[] = []
    await refreshTree(new Set(['src']), async (rel) => {
      calls.push(rel)
      return []
    })
    expect(calls).toEqual(['', 'src'])
    expect(calls).not.toContain('docs')
  })
})
