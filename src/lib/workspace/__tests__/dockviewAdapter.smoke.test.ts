// DockviewAdapter smoke test（4 case）—— 不用真实 dockview mount，注 spy api
//
// 100% 覆盖 dockview 内部 mount 行为代价太大（plan 决策 D2）；这里只验 adapter 把
// WorkspaceAdapter 接口正确翻译为 dockview-vue DockviewApi 调用。

import { defineComponent } from 'vue'
import type { DockviewApi } from 'dockview-vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { DockviewAdapter } from '../dockviewAdapter'
import type { PanelDescriptor } from '../types'

const DummyComp = defineComponent({ render: () => null })

interface AddPanelOpts {
  id: string
  component: string
  title?: string
  params?: unknown
  renderer?: 'always' | 'onlyWhenVisible'
  position?: { referencePanel: string; direction: 'right' }
}

// 极简 DockviewApi spy（仅覆盖 adapter 用到的 6 方法 + 1 property）
function makeApiSpy() {
  const panels: Array<{ id: string; api: { setActive: ReturnType<typeof vi.fn> } }> = []
  return {
    addPanel: vi.fn<(opts: AddPanelOpts) => void>().mockImplementation((opts: AddPanelOpts) => {
      panels.push({ id: opts.id, api: { setActive: vi.fn() } })
    }),
    removePanel: vi.fn((p: { id: string }) => {
      const idx = panels.findIndex((x) => x.id === p.id)
      if (idx >= 0) panels.splice(idx, 1)
    }),
    getPanel: vi.fn((id: string) => panels.find((p) => p.id === id)),
    toJSON: vi.fn(() => ({ grid: { mock: true } })),
    fromJSON: vi.fn(),
    clear: vi.fn(() => {
      panels.length = 0
    }),
    get panels() {
      return panels
    },
  }
}

type ApiSpy = ReturnType<typeof makeApiSpy>

let api: ApiSpy
let adapter: DockviewAdapter

function makeDesc(id: string, overrides: Partial<PanelDescriptor> = {}): PanelDescriptor {
  return {
    id,
    title: id,
    component: DummyComp,
    category: 'config',
    ...overrides,
  }
}

beforeEach(() => {
  api = makeApiSpy()
  // 用 unknown cast 绕开 DockviewApi 完整类型（plan D2：smoke only）
  adapter = new DockviewAdapter(api as unknown as DockviewApi)
})

describe('DockviewAdapter', () => {
  it('mountPanel 调 api.addPanel(component=descriptor.id, title, params, renderer)', () => {
    const desc = makeDesc('ChatHub', { mountStrategy: 'always' })
    adapter.mountPanel(desc, { greeting: 'hi' })
    expect(api.addPanel).toHaveBeenCalledTimes(1)
    const opts = api.addPanel.mock.calls[0]![0]
    expect(opts.id).toBe('ChatHub')
    expect(opts.component).toBe('ChatHub') // 6.x: component = Vue 全局名 = descriptor.id
    expect(opts.title).toBe('ChatHub')
    expect(opts.params).toEqual({ greeting: 'hi' })
    expect(opts.renderer).toBe('always') // mountStrategy 'always' → renderer 'always'
    // 幂等：已存在不重 addPanel
    adapter.mountPanel(desc)
    expect(api.addPanel).toHaveBeenCalledTimes(1)
  })

  it('mountStrategy 默认 → renderer "onlyWhenVisible"；title 函数版 + main.right position', () => {
    // 先放一个 panel 当 main.right 的 reference
    adapter.mountPanel(makeDesc('First'))
    api.addPanel.mockClear()
    // title 函数版 + main.right defaultLocation
    const desc = makeDesc('Second', {
      title: (p) => `dynamic-${(p as { tag?: string } | undefined)?.tag ?? 'x'}`,
      defaultLocation: 'main.right',
    })
    adapter.mountPanel(desc, { tag: 'y' })
    const opts = api.addPanel.mock.calls[0]![0]
    expect(opts.title).toBe('dynamic-y')
    expect(opts.renderer).toBe('onlyWhenVisible') // 默认
    expect(opts.position).toEqual({ referencePanel: 'First', direction: 'right' })
  })

  it('unmountPanel / revealPanel / isPanelOpen 正确转发 + 不存在 = no-op', () => {
    adapter.mountPanel(makeDesc('A'))
    expect(adapter.isPanelOpen('A')).toBe(true)
    expect(adapter.isPanelOpen('B')).toBe(false)

    adapter.revealPanel('A')
    const panelA = api.panels[0]!
    expect(panelA.api.setActive).toHaveBeenCalled()

    // 不存在的 reveal/unmount = no-op
    adapter.revealPanel('Nope')
    adapter.unmountPanel('Nope')
    expect(api.removePanel).not.toHaveBeenCalled()

    adapter.unmountPanel('A')
    expect(api.removePanel).toHaveBeenCalledWith(panelA)
    expect(adapter.isPanelOpen('A')).toBe(false)
  })

  it('serialize / deserialize / dispose 转发', () => {
    const s = adapter.serialize()
    expect(api.toJSON).toHaveBeenCalled()
    expect(s).toBe('{"grid":{"mock":true}}')

    adapter.deserialize('{"x":1}')
    expect(api.fromJSON).toHaveBeenCalledWith({ x: 1 })

    adapter.mountPanel(makeDesc('A'))
    expect(adapter.isPanelOpen('A')).toBe(true)
    adapter.dispose()
    expect(api.clear).toHaveBeenCalled()
    // dispose 后 panel 清空
    expect(adapter.isPanelOpen('A')).toBe(false)
  })
})
