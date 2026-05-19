// S0 sanity test —— 验证 vitest + jsdom + @vue/test-utils 基建跑通。
// 后续删除（S1 起在 src/lib/snap/__tests__/ 写真正的单测）。

import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'

describe('S0 sanity', () => {
  it('vitest runs', () => {
    expect(1 + 1).toBe(2)
  })

  it('jsdom env: document is defined', () => {
    expect(typeof document).toBe('object')
  })

  it('@vue/test-utils: mount renders a component', () => {
    const Hello = defineComponent({ render: () => h('span', 'hi') })
    const wrapper = mount(Hello)
    expect(wrapper.text()).toBe('hi')
  })
})
