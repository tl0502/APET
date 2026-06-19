import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, test, vi } from 'vitest'
import PersonaWorkshopPanel from '../PersonaWorkshopPanel.vue'

vi.mock('@/services/persona', () => ({
  getActivePersona: vi.fn(async () => ({
    id: 'momo',
    snapshot_id: 'momo-1.0.0',
    name: '默默',
    version: '1.0.0',
    source: 'builtin',
    raw_markdown: [
      '# 身份',
      '你叫默默。',
      '',
      '# 性格',
      '- 慵懒',
      '',
      '# 能力',
      '- 陪伴',
      '',
      '# 行为规则',
      '## Do',
      '- 用第二人称',
      "## Don't",
      '- 不空洞鼓励',
    ].join('\n'),
  })),
  listPersonas: vi.fn(async () => [
    { id: 'momo', name: '默默', version: '1.0.0', source: 'builtin', is_active: true },
    { id: 'joker', name: '阿吉', version: '1.0.0', source: 'builtin', is_active: false },
  ]),
}))

describe('PersonaWorkshopPanel', () => {
  test('loads persona cards without opening the inspector initially', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    expect(wrapper.find('[aria-label="角色卡舞台"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('角色卡')
    expect(wrapper.text()).toContain('默默')
    expect(wrapper.text()).toContain('阿吉')
    expect(wrapper.find('[aria-label="人格编辑抽屉"]').exists()).toBe(false)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(false)
  })

  test('opens the inspector when a persona card is clicked', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    expect(activeCard).toBeTruthy()
    await activeCard?.trigger('click')

    expect(wrapper.find('[aria-label="人格编辑抽屉"]').exists()).toBe(true)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(true)
    expect(wrapper.text()).toContain('塑形')
    expect(wrapper.text()).toContain('结构')
    expect(wrapper.text()).toContain('源码')
    expect(wrapper.text()).toContain('编译诊断')
  })

  test('lets the inspector close without clearing the card stage', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    expect(activeCard).toBeTruthy()
    await activeCard?.trigger('click')

    const closeButton = wrapper.findAll('button').find((button) => button.text() === '关闭')
    expect(closeButton).toBeTruthy()
    await closeButton?.trigger('click')

    expect(wrapper.find('[aria-label="人格编辑抽屉"]').exists()).toBe(false)
    expect(wrapper.find('[aria-label="角色卡舞台"]').exists()).toBe(true)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(false)
  })
})
