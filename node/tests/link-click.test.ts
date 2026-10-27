// @vitest-environment jsdom
import { describe, expect, it, vi } from 'vitest'

/**
 * 消息体点击的核心逻辑（与 MessageItem.vue 的 onBodyClick 等价）。
 *
 * 这里必须测到的是：命中 <a> 时调用 preventDefault。
 * 不调的话 Electron 会让当前窗口导航到目标网址，
 * 应用界面被网页替换且没有后退入口，用户只能杀进程。
 */
async function onBodyClick(
  e: MouseEvent,
  openExternal: (u: string) => Promise<void>
): Promise<void> {
  const target = e.target as HTMLElement
  const link = target.closest('a')
  if (link) {
    e.preventDefault()
    const href = link.getAttribute('href')
    if (href) await openExternal(href)
    return
  }
  if (!target.matches('[data-copy]')) return
}

function mount(html: string): void {
  document.body.innerHTML = `<div class="md">${html}</div>`
}

describe('消息体点击拦截', () => {
  it('链接点击交给系统浏览器，且阻止窗口导航', async () => {
    mount('<a href="https://example.com/x">点我</a>')
    const open = vi.fn(async () => undefined)
    // 用 addEventListener 让 preventDefault 在事件派发期间生效，
    // 与 Vue 的 @click 真实行为一致
    const handler = (ev: MouseEvent): void => void onBodyClick(ev, open)
    document.querySelector('.md')!.addEventListener('click', handler)
    const ev = new MouseEvent('click', { bubbles: true, cancelable: true })
    document.querySelector('a')!.dispatchEvent(ev)

    expect(ev.defaultPrevented).toBe(true)
    expect(open).toHaveBeenCalledWith('https://example.com/x')
  })

  it('嵌套元素上的点击也能命中外层链接', async () => {
    mount('<a href="https://example.com/y"><span>内层文字</span></a>')
    const open = vi.fn(async () => undefined)
    const handler = (ev: MouseEvent): void => void onBodyClick(ev, open)
    document.querySelector('.md')!.addEventListener('click', handler)
    const ev = new MouseEvent('click', { bubbles: true, cancelable: true })
    document.querySelector('span')!.dispatchEvent(ev)

    expect(ev.defaultPrevented).toBe(true)
    expect(open).toHaveBeenCalledWith('https://example.com/y')
  })

  it('复制按钮不触发打开链接', async () => {
    mount('<button data-copy>复制</button>')
    const open = vi.fn(async () => undefined)
    const ev = new MouseEvent('click', { bubbles: true, cancelable: true })
    document.querySelector('button')!.dispatchEvent(ev)
    await onBodyClick(ev, open)

    expect(open).not.toHaveBeenCalled()
  })

  it('普通文字点击不做任何事', async () => {
    mount('<p>普通段落</p>')
    const open = vi.fn(async () => undefined)
    const ev = new MouseEvent('click', { bubbles: true, cancelable: true })
    document.querySelector('p')!.dispatchEvent(ev)
    await onBodyClick(ev, open)

    expect(ev.defaultPrevented).toBe(false)
    expect(open).not.toHaveBeenCalled()
  })
})
