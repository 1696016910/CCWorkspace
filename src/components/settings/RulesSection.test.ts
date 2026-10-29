import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

const rules = [
  { name: 'ai-coding-constitution', description: 'AI 行为总宪法', source: 'user' },
  { name: 'workspace-rules', description: '工作区规则', source: 'user' }
]

vi.mock('@/api/skill', () => ({
  ruleList: vi.fn(async () => rules),
  ruleRead: vi.fn(async () => '# 规则正文\n\n内容在这里。'),
  ruleWrite: vi.fn(async () => {}),
  ruleRemove: vi.fn(async () => {})
}))

import RulesSection from '@/components/settings/RulesSection.vue'

describe('规则分区', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('列出全部规则且没有启用开关', async () => {
    const w = mount(RulesSection)
    await new Promise((r) => setTimeout(r, 10))

    expect(w.findAll('.card')).toHaveLength(2)
    expect(w.text()).toContain('ai-coding-constitution')
    // 规则无法禁用，界面不该出现开关
    expect(w.find('.toggle').exists()).toBe(false)
  })

  it('点查看打开编辑器并加载正文', async () => {
    const w = mount(RulesSection, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 10))

    const viewBtn = w.findAll('.mini').find((b: { text: () => string }) => b.text() === '查看')
    expect(viewBtn).toBeTruthy()
    await viewBtn!.trigger('click')
    await new Promise((r) => setTimeout(r, 50))

    // 编辑器是 teleport 之前的普通弹窗，直接从 wrapper 找
    const ta = document.querySelector('.editor .content') as HTMLTextAreaElement
    expect(ta).toBeTruthy()
    expect(ta.value).toContain('规则正文')

    w.unmount()
  })

  it('已有规则的名称不可编辑', async () => {
    const w = mount(RulesSection, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 10))
    const viewBtn = w.findAll('.mini').find((b: { text: () => string }) => b.text() === '查看')
    await viewBtn!.trigger('click')
    await new Promise((r) => setTimeout(r, 50))

    // nameEditable=false 时不渲染名称输入框
    expect(document.querySelector('.editor .field')).toBeNull()
    w.unmount()
  })

  it('删除前要求二次确认', async () => {
    const w = mount(RulesSection, { attachTo: document.body })
    await new Promise((r) => setTimeout(r, 10))

    const delBtn = w.findAll('.mini').find((b: { text: () => string }) => b.text() === '删除')
    await delBtn!.trigger('click')
    await new Promise((r) => setTimeout(r, 30))

    const api = await import('@/api/skill')
    expect(api.ruleRemove).not.toHaveBeenCalled()
    expect(document.body.textContent).toContain('删除规则')

    w.unmount()
  })
})
