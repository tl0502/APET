// PanelRegistry 单测（4 case）

import { defineComponent } from 'vue'
import { beforeEach, describe, expect, it } from 'vitest'

import { PanelRegistry } from '../panelRegistry'
import {
  InvalidPanelIdError,
  PanelAlreadyRegisteredError,
  type PanelDescriptor,
} from '../types'

const DummyComp = defineComponent({ render: () => null })

function makeDescriptor(id: string): PanelDescriptor {
  return {
    id,
    title: id,
    component: DummyComp,
    category: 'config',
  }
}

let registry: PanelRegistry
beforeEach(() => {
  registry = new PanelRegistry()
})

describe('PanelRegistry', () => {
  it('register 新 panel → list 长度 1 + get 命中', () => {
    const d = makeDescriptor('ChatHub')
    registry.register(d)
    expect(registry.size()).toBe(1)
    expect(registry.get('ChatHub')).toBe(d)
    expect(registry.list()).toEqual([d])
  })

  it('register 同 id 两次 → 抛 PanelAlreadyRegisteredError', () => {
    registry.register(makeDescriptor('ChatHub'))
    expect(() => registry.register(makeDescriptor('ChatHub'))).toThrow(
      PanelAlreadyRegisteredError,
    )
  })

  it('register 非 PascalCase id → 抛 InvalidPanelIdError', () => {
    expect(() => registry.register(makeDescriptor('chat-hub'))).toThrow(InvalidPanelIdError)
    expect(() => registry.register(makeDescriptor('chatHub'))).toThrow(InvalidPanelIdError)
    expect(() => registry.register(makeDescriptor('Chat_Hub'))).toThrow(InvalidPanelIdError)
    expect(() => registry.register(makeDescriptor('123Chat'))).toThrow(InvalidPanelIdError)
  })

  it('unregister 已注册 + 不存在 id → 正确清/无副作用 + get 返 undefined', () => {
    registry.register(makeDescriptor('ChatHub'))
    registry.register(makeDescriptor('Settings'))
    registry.unregister('ChatHub')
    expect(registry.size()).toBe(1)
    expect(registry.get('ChatHub')).toBeUndefined()
    expect(registry.get('Settings')).toBeDefined()
    // 不存在 id = no-op
    registry.unregister('Nonexistent')
    expect(registry.size()).toBe(1)
  })
})
