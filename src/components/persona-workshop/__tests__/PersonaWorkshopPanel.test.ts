/* eslint-disable vue/one-component-per-file */
import { flushPromises, mount } from '@vue/test-utils'
import { open, save } from '@tauri-apps/plugin-dialog'
import { ElMessageBox } from 'element-plus'
import { defineComponent, h } from 'vue'
import { describe, expect, test, vi } from 'vitest'
import PersonaWorkshopPanel from '../PersonaWorkshopPanel.vue'
import {
  deletePersona,
  exportPersonaSnapshot,
  getActivePersona,
  importPersonaFromPath,
  listPersonas,
  loadPersona,
  savePersonaDraft,
} from '@/services/persona'

const personaWithoutExamples = {
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
}

const importedPersona = {
  id: 'imported',
  snapshot_id: '202',
  name: '导入人格',
  version: '1.0.0',
  source: 'imported',
  raw_markdown: [
    '# 身份',
    '你叫导入人格。',
    '',
    '# 性格',
    '- 稳定',
    '',
    '# 能力',
    '- 帮用户检查导入导出',
    '',
    '# 行为规则',
    '## Do',
    '- 明确回应',
    "## Don't",
    '- 不读屏幕',
  ].join('\n'),
}

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}))

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
      '',
      '# 例对话',
      '- 用户：你好',
      '  默默：我在。',
    ].join('\n'),
  })),
  listPersonas: vi.fn(async () => [
    { id: 'momo', name: '默默', version: '1.0.0', source: 'builtin', is_active: true },
    { id: 'joker', name: '阿吉', version: '1.0.0', source: 'builtin', is_active: false },
  ]),
  deletePersona: vi.fn(async () => undefined),
  loadPersona: vi.fn(async () => ({
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
      '',
      '# 例对话',
      '- 用户：你好',
      '  默默：我在。',
    ].join('\n'),
  })),
  validatePersonaDraft: vi.fn(async () => ({
    diagnostics: [],
    blocking: false,
    token_estimate: 80,
  })),
  savePersonaDraft: vi.fn(async (draft) => ({
    persona_id: draft.personaId,
    snapshot_id: '101',
    version: draft.personaId === 'momo' ? '1.0.1' : '1.0.0',
    activated: false,
    diagnostics: [],
  })),
  saveAndActivatePersonaDraft: vi.fn(),
  importPersonaFromPath: vi.fn(async () => ({
    persona_id: 'imported',
    snapshot_id: '202',
    version: '1.0.0',
    activated: false,
    diagnostics: [],
  })),
  exportPersonaSnapshot: vi.fn(async () => ({
    persona_id: 'momo',
    snapshot_id: '101',
    version: '1.0.0',
    filename: 'momo-1.0.0.soul.md',
    path: 'C:\\tmp\\momo-1.0.0.soul.md',
  })),
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
    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(false)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(false)
  })

  test('opens a glass editor card when a persona card is double-clicked', async () => {
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
    await activeCard?.trigger('dblclick')

    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(true)
    expect(wrapper.find('.persona-inspector__backdrop').exists()).toBe(true)
    expect(wrapper.find('.persona-inspector__shell').exists()).toBe(true)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(false)
    expect(wrapper.text()).toContain('塑形')
    expect(wrapper.text()).toContain('结构')
    expect(wrapper.text()).toContain('示例')
    expect(wrapper.text()).toContain('源码')
    expect(wrapper.text()).toContain('编译诊断')
  })

  test('single-clicks select a persona card without opening the inspector', async () => {
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

    const otherCard = wrapper.findAll('button').find((button) => button.text().includes('阿吉'))
    expect(otherCard).toBeTruthy()
    await otherCard?.trigger('click')
    await flushPromises()

    // 单击只选中：卡片高亮（aria-pressed=true），但编辑抽屉不打开。
    expect(otherCard?.attributes('aria-pressed')).toBe('true')
    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(false)

    // 双击同一张卡才进编辑。
    await otherCard?.trigger('dblclick')
    await flushPromises()
    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(true)
  })

  test('exposes soul markdown import and export actions', async () => {
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

    expect(wrapper.findAll('button').some((button) => button.text() === '导入 .soul.md')).toBe(
      true,
    )

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('click')
    await flushPromises()

    expect(wrapper.findAll('button').some((button) => button.text() === '导出 .soul.md')).toBe(
      true,
    )
  })

  test('keeps persona file utilities out of the inspector shell', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('dblclick')
    await flushPromises()

    const utilityActions = wrapper.find('.persona-workshop__context-actions')
    const inspectorUtilityActions = wrapper.find('.persona-inspector__utility-actions')
    const footerActions = wrapper.find('.persona-inspector__actions')

    expect(utilityActions.exists()).toBe(true)
    expect(utilityActions.text()).toContain('复制为新人格')
    expect(utilityActions.text()).toContain('导出 .soul.md')
    expect(utilityActions.text()).toContain('删除人格')
    expect(inspectorUtilityActions.exists()).toBe(false)
    expect(footerActions.text()).not.toContain('复制为新人格')
    expect(footerActions.text()).not.toContain('导出 .soul.md')
    expect(footerActions.text()).not.toContain('删除人格')
  })

  test('imports a soul markdown file and opens the imported persona draft', async () => {
    vi.mocked(open).mockResolvedValueOnce('C:\\tmp\\imported.soul.md')
    vi.mocked(loadPersona).mockResolvedValueOnce(importedPersona)
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

    const importButton = wrapper
      .findAll('button')
      .find((button) => button.text() === '导入 .soul.md')
    expect(importButton).toBeTruthy()
    await importButton?.trigger('click')
    await flushPromises()

    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({
        multiple: false,
        filters: [{ name: 'Soul Markdown', extensions: ['md'] }],
      }),
    )
    expect(importPersonaFromPath).toHaveBeenCalledWith('C:\\tmp\\imported.soul.md', false)
    expect(wrapper.text()).toContain('已导入 导入人格 v1.0.0')
    expect(wrapper.text()).toContain('导入人格')
  })

  test('exports the selected saved snapshot through the save dialog', async () => {
    vi.mocked(getActivePersona).mockResolvedValueOnce({
      id: 'momo',
      snapshot_id: '101',
      name: '默默',
      version: '1.0.0',
      source: 'builtin',
      raw_markdown: personaWithoutExamples.raw_markdown,
    })
    vi.mocked(save).mockResolvedValueOnce('C:\\tmp\\momo-1.0.0.soul.md')
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('click')
    await flushPromises()

    const exportButton = wrapper
      .findAll('button')
      .find((button) => button.text() === '导出 .soul.md')
    expect(exportButton).toBeTruthy()
    await exportButton?.trigger('click')
    await flushPromises()

    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({
        defaultPath: 'momo-1.0.0.soul.md',
        filters: [{ name: 'Soul Markdown', extensions: ['md'] }],
      }),
    )
    expect(exportPersonaSnapshot).toHaveBeenCalledWith(101, 'C:\\tmp\\momo-1.0.0.soul.md')
    expect(wrapper.text()).toContain('已导出 momo-1.0.0.soul.md')
  })

  test('deletes an imported persona after confirmation and returns to the active persona', async () => {
    vi.mocked(listPersonas)
      .mockResolvedValueOnce([
        { id: 'momo', name: '默默', version: '1.0.0', source: 'builtin', is_active: true },
        {
          id: 'imported',
          name: '导入人格',
          version: '1.0.0',
          source: 'imported',
          is_active: false,
        },
      ])
      .mockResolvedValueOnce([
        { id: 'momo', name: '默默', version: '1.0.0', source: 'builtin', is_active: true },
      ])
    vi.mocked(loadPersona).mockResolvedValueOnce(importedPersona)
    const confirmSpy = vi
      .spyOn(ElMessageBox, 'confirm')
      .mockResolvedValueOnce('confirm' as never)
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button :disabled="$attrs.disabled" @click="$emit(`click`)"><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const importedCard = wrapper
      .findAll('button')
      .find((button) => button.text().includes('导入人格'))
    expect(importedCard).toBeTruthy()
    await importedCard?.trigger('click')
    await flushPromises()

    const deleteButton = wrapper.findAll('button').find((button) => button.text() === '删除人格')
    expect(deleteButton).toBeTruthy()
    await deleteButton?.trigger('click')
    await flushPromises()

    expect(confirmSpy).toHaveBeenCalledWith(
      '删除「导入人格」？关联快照会一并清除，此操作不可撤销。',
      '确认删除人格',
      expect.objectContaining({
        confirmButtonText: '删除',
        type: 'warning',
      }),
    )
    expect(deletePersona).toHaveBeenCalledWith('imported')
    expect(wrapper.text()).toContain('已删除 导入人格')
    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(false)

    confirmSpy.mockRestore()
  })

  test('shows example tab content and keeps source preview in sync', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: {
            template: '<button :aria-label="$attrs[`aria-label`]" @click="$emit(`click`)"><slot /></button>',
          },
          ElInput: defineComponent({
            props: { modelValue: { type: String, default: '' } },
            emits: ['update:modelValue'],
            setup(props, { emit }) {
              return () =>
                h('textarea', {
                  value: props.modelValue,
                  onInput: (event: Event) =>
                    emit('update:modelValue', (event.target as HTMLTextAreaElement).value),
                })
            },
          }),
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('dblclick')

    const exampleTab = wrapper.findAll('button').find((button) => button.text() === '示例')
    expect(exampleTab).toBeTruthy()
    await exampleTab?.trigger('click')

    expect(wrapper.find('[aria-label="示例对话编辑器"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('示例 1')

    const addExampleButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '添加示例')
    expect(addExampleButton).toBeTruthy()
    await addExampleButton?.trigger('click')
    expect(wrapper.text()).toContain('示例 2')

    const inputs = wrapper.findAll('textarea')
    await inputs[0].setValue('今天有点累')
    await inputs[1].setValue('先慢一点。')

    const sourceTab = wrapper.findAll('button').find((button) => button.text() === '源码')
    await sourceTab?.trigger('click')
    await flushPromises()

    const sourceText = (wrapper.findAll('textarea').at(-1)?.element as HTMLTextAreaElement).value
    expect(sourceText).toContain('# 例对话')
    expect(sourceText).toContain('用户：今天有点累')
    expect(sourceText).toContain('默默：先慢一点。')
  })

  test('edits capabilities in the structured tab and keeps source preview in sync', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: {
            template: '<button :aria-label="$attrs[`aria-label`]" @click="$emit(`click`)"><slot /></button>',
          },
          ElInput: defineComponent({
            props: { modelValue: { type: String, default: '' } },
            emits: ['update:modelValue'],
            setup(props, { emit }) {
              return () =>
                h('textarea', {
                  value: props.modelValue,
                  onInput: (event: Event) =>
                    emit('update:modelValue', (event.target as HTMLTextAreaElement).value),
                })
            },
          }),
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('dblclick')

    const structuredTab = wrapper.findAll('button').find((button) => button.text() === '结构')
    await structuredTab?.trigger('click')
    await flushPromises()

    const structuredTextareas = wrapper.findAll('textarea')
    expect(structuredTextareas).toHaveLength(5)
    expect((structuredTextareas[2].element as HTMLTextAreaElement).value).toContain('- 陪伴')

    await structuredTextareas[2].setValue('- 帮用户拆任务\n- 提醒用户休息')

    const sourceTab = wrapper.findAll('button').find((button) => button.text() === '源码')
    await sourceTab?.trigger('click')
    await flushPromises()

    const sourceText = (wrapper.findAll('textarea').at(-1)?.element as HTMLTextAreaElement).value
    expect(sourceText).toContain('# 能力')
    expect(sourceText).toContain('- 帮用户拆任务\n- 提醒用户休息')
  })

  test('creates a new unsaved persona draft and saves it as a user persona', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: {
            template: '<button :disabled="$attrs.disabled"><slot /></button>',
          },
          ElInput: defineComponent({
            props: { modelValue: { type: String, default: '' } },
            emits: ['update:modelValue'],
            setup(props, { emit }) {
              return () =>
                h('textarea', {
                  value: props.modelValue,
                  onInput: (event: Event) =>
                    emit('update:modelValue', (event.target as HTMLTextAreaElement).value),
                })
            },
          }),
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const newButton = wrapper.findAll('button').find((button) => button.text() === '新建人格')
    expect(newButton).toBeTruthy()
    await newButton?.trigger('click')
    await flushPromises()

    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('新建未保存')

    const sourceTab = wrapper.findAll('button').find((button) => button.text() === '源码')
    await sourceTab?.trigger('click')
    await flushPromises()
    const sourceText = (wrapper.findAll('textarea').at(-1)?.element as HTMLTextAreaElement).value
    expect(sourceText).toContain('你叫新人格')

    const saveButton = wrapper.findAll('button').find((button) => button.text() === '保存快照')
    await saveButton?.trigger('click')
    await flushPromises()

    expect(savePersonaDraft).toHaveBeenLastCalledWith(
      expect.objectContaining({
        personaId: 'user-persona',
        source: 'user',
        version: '1.0.0',
      }),
    )
    expect(wrapper.text()).toContain('已保存')
  })

  test('duplicates the current persona as an unsaved user copy', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: {
            template: '<button :disabled="$attrs.disabled"><slot /></button>',
          },
          ElInput: { template: '<textarea :value="$attrs.modelValue" />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('click')
    await flushPromises()

    const duplicateButton = wrapper
      .findAll('button')
      .find((button) => button.text() === '复制为新人格')
    expect(duplicateButton).toBeTruthy()
    await duplicateButton?.trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('复制未保存')
    expect(wrapper.text()).toContain('默默 副本')

    const saveButton = wrapper.findAll('button').find((button) => button.text() === '保存快照')
    await saveButton?.trigger('click')
    await flushPromises()

    expect(savePersonaDraft).toHaveBeenLastCalledWith(
      expect.objectContaining({
        personaId: 'momo-copy',
        source: 'user',
        version: '1.0.0',
      }),
    )
  })

  test('marks a loaded persona as dirty after editing', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: {
            template: '<button :disabled="$attrs.disabled"><slot /></button>',
          },
          ElInput: defineComponent({
            props: { modelValue: { type: String, default: '' } },
            emits: ['update:modelValue'],
            setup(props, { emit }) {
              return () =>
                h('textarea', {
                  value: props.modelValue,
                  onInput: (event: Event) =>
                    emit('update:modelValue', (event.target as HTMLTextAreaElement).value),
                })
            },
          }),
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('dblclick')
    await flushPromises()

    expect(wrapper.text()).toContain('已保存')

    const simpleInputs = wrapper.findAll('textarea')
    await simpleInputs[1].setValue('新的定位')
    await flushPromises()

    expect(wrapper.text()).toContain('有未保存修改')
  })

  test('adds the first example from an empty example state', async () => {
    vi.mocked(getActivePersona).mockResolvedValueOnce(personaWithoutExamples)
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: {
            template:
              '<button :aria-label="$attrs[`aria-label`]" @click="$emit(`click`)"><slot /></button>',
          },
          ElInput: defineComponent({
            props: { modelValue: { type: String, default: '' } },
            emits: ['update:modelValue'],
            setup(props, { emit }) {
              return () =>
                h('textarea', {
                  value: props.modelValue,
                  onInput: (event: Event) =>
                    emit('update:modelValue', (event.target as HTMLTextAreaElement).value),
                })
            },
          }),
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElIcon: { template: '<span><slot /></span>' },
        },
      },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('dblclick')

    const exampleTab = wrapper.findAll('button').find((button) => button.text() === '示例')
    await exampleTab?.trigger('click')

    expect(wrapper.text()).toContain('还没有示例对话')

    const addExampleButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '添加示例')
    await addExampleButton?.trigger('click')

    expect(wrapper.text()).toContain('示例 1')
  })

  test('adds the first example from the inspector with real Element Plus controls', async () => {
    vi.mocked(getActivePersona).mockResolvedValueOnce(personaWithoutExamples)
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('dblclick')

    const exampleTab = wrapper.findAll('button').find((button) => button.text() === '示例')
    await exampleTab?.trigger('click')

    expect(wrapper.text()).toContain('还没有示例对话')

    const addExampleButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '添加示例')
    await addExampleButton?.trigger('click')

    expect(wrapper.text()).toContain('示例 1')
  })

  test('writes a newly added example into source preview with real Element Plus controls', async () => {
    vi.mocked(getActivePersona).mockResolvedValueOnce(personaWithoutExamples)
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
    })

    await flushPromises()

    const activeCard = wrapper.findAll('button').find((button) => button.text().includes('默默'))
    await activeCard?.trigger('dblclick')

    const exampleTab = wrapper.findAll('button').find((button) => button.text() === '示例')
    await exampleTab?.trigger('click')

    const addExampleButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '添加示例')
    await addExampleButton?.trigger('click')
    await flushPromises()

    const textareas = wrapper.findAll('textarea')
    await textareas[0].setValue('今天好累')
    await textareas[1].setValue('先坐一下，我陪你。')
    await flushPromises()

    const sourceTab = wrapper.findAll('button').find((button) => button.text() === '源码')
    await sourceTab?.trigger('click')
    await flushPromises()

    const sourceText = (wrapper.findAll('textarea').at(-1)?.element as HTMLTextAreaElement).value
    expect(sourceText).toContain('用户：今天好累')
    expect(sourceText).toContain('默默：先坐一下，我陪你。')
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
    await activeCard?.trigger('dblclick')

    const closeButton = wrapper
      .findAll('button')
      .find((button) => button.attributes('aria-label') === '关闭')
    expect(closeButton).toBeTruthy()
    await closeButton?.trigger('click')

    expect(wrapper.find('[aria-label="人格编辑卡片"]').exists()).toBe(false)
    expect(wrapper.find('[aria-label="角色卡舞台"]').exists()).toBe(true)
    expect(wrapper.find('.persona-workshop__layout--inspector-open').exists()).toBe(false)
  })
})
