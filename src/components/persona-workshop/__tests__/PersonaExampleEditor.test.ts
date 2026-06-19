import { mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { describe, expect, test } from 'vitest'
import PersonaExampleEditor from '../PersonaExampleEditor.vue'

const stubs = {
  ElButton: {
    props: ['disabled'],
    template: '<button :disabled="disabled" v-bind="$attrs" @click="$emit(`click`)"><slot /></button>',
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
}

describe('PersonaExampleEditor', () => {
  test('adds the first example through the real Element Plus button', async () => {
    const wrapper = mount(PersonaExampleEditor, {
      props: { pairs: [], personaName: '默默', maxExamples: 5 },
    })

    await wrapper.find('[aria-label="添加示例"]').trigger('click')

    expect(wrapper.emitted('update:pairs')?.[0]).toEqual([[{ user: '', assistant: '' }]])
  })

  test('renders empty state and adds the first example', async () => {
    const wrapper = mount(PersonaExampleEditor, {
      props: { pairs: [], personaName: '默默', maxExamples: 5 },
      global: { stubs },
    })

    expect(wrapper.text()).toContain('还没有示例对话')
    await wrapper.find('button').trigger('click')

    expect(wrapper.emitted('update:pairs')?.[0]).toEqual([[{ user: '', assistant: '' }]])
  })

  test('edits user and assistant text', async () => {
    const wrapper = mount(PersonaExampleEditor, {
      props: {
        pairs: [{ user: '你好', assistant: '我在。' }],
        personaName: '默默',
        maxExamples: 5,
      },
      global: { stubs },
    })

    const inputs = wrapper.findAll('textarea')
    await inputs[0].setValue('今天好累')
    await inputs[1].setValue('先慢一点。')

    expect(wrapper.emitted('update:pairs')?.[0]).toEqual([
      [{ user: '今天好累', assistant: '我在。' }],
    ])
    expect(wrapper.emitted('update:pairs')?.[1]).toEqual([
      [{ user: '你好', assistant: '先慢一点。' }],
    ])
  })

  test('deletes a pair and disables add at max count', async () => {
    const pairs = Array.from({ length: 5 }, (_, index) => ({
      user: `用户 ${index + 1}`,
      assistant: `回复 ${index + 1}`,
    }))
    const wrapper = mount(PersonaExampleEditor, {
      props: { pairs, personaName: '默默', maxExamples: 5 },
      global: { stubs },
    })

    expect(wrapper.find('[aria-label="添加示例"]').attributes('disabled')).toBeDefined()
    await wrapper.find('[aria-label="删除示例 1"]').trigger('click')

    expect(wrapper.emitted('update:pairs')?.[0]).toEqual([pairs.slice(1)])
  })
})
