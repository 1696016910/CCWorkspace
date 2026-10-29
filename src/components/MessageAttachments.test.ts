import { describe, expect, it } from 'vitest'
/**
 * 乐观消息的构造逻辑。
 *
 * 发出去到落库回来之间，界面显示的是本地构造的那条消息。它必须带上
 * 附件元数据，否则这段窗口期里附件是看不见的——用户会以为附件没发出去。
 */
function buildOptimistic(text: string, files: { kind: 'image' | 'file'; name: string; path?: string; size: number }[]) {
  return {
    text: text || '（附件）',
    attachments: files.map((a) => ({
      kind: a.kind,
      name: a.name,
      path: a.path,
      size: a.size
    }))
  }
}

/** 路径中间省略：保留盘符与末两段 */
function shortPath(p?: string): string {
  if (!p) return ''
  const sep = p.includes('\\') ? '\\' : '/'
  const parts = p.split(sep).filter(Boolean)
  if (parts.length <= 3) return p
  return `${parts[0]}${sep}...${sep}${parts.slice(-2).join(sep)}`
}

describe('已发送消息的附件回显', () => {
  it('乐观消息必须带附件元数据', () => {
    const files = [
      { kind: 'file' as const, name: 'test.sql', path: 'C:\\a\\test.sql', size: 12288 },
      { kind: 'image' as const, name: 'shot.png', path: 'C:\\a\\shot.png', size: 47616 }
    ]
    const msg = buildOptimistic('看看这个', files)
    expect(msg.attachments).toHaveLength(2)
    expect(msg.attachments[0].name).toBe('test.sql')
    expect(msg.attachments[1].kind).toBe('image')
  })

  it('只发附件、没有文字时也要有可读的正文占位', () => {
    const msg = buildOptimistic('', [
      { kind: 'file' as const, name: 'a.txt', path: 'C:\\a.txt', size: 10 }
    ])
    expect(msg.text).toBe('（附件）')
  })

  it('粘贴的截图没有路径，仍要能显示', () => {
    const msg = buildOptimistic('看图', [
      { kind: 'image' as const, name: '粘贴的图片.png', size: 5000 }
    ])
    expect(msg.attachments[0].path).toBeUndefined()
    expect(msg.attachments[0].name).toBe('粘贴的图片.png')
  })
})

describe('附件路径的中间省略', () => {
  it('深路径保留盘符与末两段', () => {
    expect(shortPath('C:\\Users\\MasterY\\Downloads\\ttt\\test.sql')).toBe(
      'C:\\...\\ttt\\test.sql'
    )
  })

  it('浅路径原样显示', () => {
    expect(shortPath('C:\\a.txt')).toBe('C:\\a.txt')
    expect(shortPath('C:\\a\\b.txt')).toBe('C:\\a\\b.txt')
  })

  it('没有路径时返回空串', () => {
    expect(shortPath(undefined)).toBe('')
  })
})
