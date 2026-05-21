// WorkspaceManager 单测（18 case）—— 覆盖 issue#35 验收 6 路径
//
// 注入 spy adapter + spy persistence + 真 PanelRegistry + 真 ContextKeyService。

import { defineComponent } from 'vue'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { WorkspaceManager } from '../manager'
import {
  CommandAlreadyRegisteredError,
  PanelAlreadyRegisteredError,
  PanelNotRegisteredError,
  type Command,
  type PanelDescriptor,
  type WorkspaceAdapter,
  type WorkspacePersistence,
} from '../types'

const DummyComp = defineComponent({ render: () => null })

function makeDescriptor(id: string, overrides: Partial<PanelDescriptor> = {}): PanelDescriptor {
  return {
    id,
    title: id,
    component: DummyComp,
    category: 'config',
    ...overrides,
  }
}

function makeSpyAdapter(): WorkspaceAdapter & {
  spies: {
    mountPanel: ReturnType<typeof vi.fn>
    unmountPanel: ReturnType<typeof vi.fn>
    revealPanel: ReturnType<typeof vi.fn>
    serialize: ReturnType<typeof vi.fn>
    deserialize: ReturnType<typeof vi.fn>
    dispose: ReturnType<typeof vi.fn>
    subscribeEvents: ReturnType<typeof vi.fn>
  }
  state: { serializedJson: string }
  /** review P0 修复：暴露 events 让测试可以模拟 dockview 事件回灌 manager */
  events: { current: import('../types').WorkspaceAdapterEvents | null }
} {
  const state = { serializedJson: '{"grid":"empty"}' }
  const events: { current: import('../types').WorkspaceAdapterEvents | null } = { current: null }
  const spies = {
    mountPanel: vi.fn(),
    unmountPanel: vi.fn(),
    revealPanel: vi.fn(),
    serialize: vi.fn(() => state.serializedJson),
    deserialize: vi.fn(),
    dispose: vi.fn(),
    subscribeEvents: vi.fn((e: import('../types').WorkspaceAdapterEvents) => {
      events.current = e
    }),
  }
  return {
    spies,
    state,
    events,
    mountPanel: spies.mountPanel as WorkspaceAdapter['mountPanel'],
    unmountPanel: spies.unmountPanel as WorkspaceAdapter['unmountPanel'],
    revealPanel: spies.revealPanel as WorkspaceAdapter['revealPanel'],
    isPanelOpen: () => false,
    serialize: spies.serialize as WorkspaceAdapter['serialize'],
    deserialize: spies.deserialize as WorkspaceAdapter['deserialize'],
    subscribeEvents: spies.subscribeEvents as WorkspaceAdapter['subscribeEvents'],
    dispose: spies.dispose as WorkspaceAdapter['dispose'],
  }
}

function makeSpyPersistence(): WorkspacePersistence & {
  spies: {
    loadLayout: ReturnType<typeof vi.fn>
    saveLayout: ReturnType<typeof vi.fn>
    loadLastActive: ReturnType<typeof vi.fn>
    saveLastActive: ReturnType<typeof vi.fn>
  }
} {
  const spies = {
    loadLayout: vi.fn().mockResolvedValue(null),
    saveLayout: vi.fn().mockResolvedValue(undefined),
    loadLastActive: vi.fn().mockResolvedValue(null),
    saveLastActive: vi.fn().mockResolvedValue(undefined),
  }
  return {
    spies,
    loadLayout: spies.loadLayout as WorkspacePersistence['loadLayout'],
    saveLayout: spies.saveLayout as WorkspacePersistence['saveLayout'],
    loadLastActive: spies.loadLastActive as WorkspacePersistence['loadLastActive'],
    saveLastActive: spies.saveLastActive as WorkspacePersistence['saveLastActive'],
  }
}

let mgr: WorkspaceManager
let adapter: ReturnType<typeof makeSpyAdapter>
beforeEach(() => {
  mgr = new WorkspaceManager()
  adapter = makeSpyAdapter()
  mgr.bindAdapter(adapter)
})

describe('WorkspaceManager — 注册去重（验收路径 1）', () => {
  it('1. registerPanel 同 id 第二次抛 PanelAlreadyRegisteredError', () => {
    mgr.registerPanel(makeDescriptor('ChatHub'))
    expect(() => mgr.registerPanel(makeDescriptor('ChatHub'))).toThrow(
      PanelAlreadyRegisteredError,
    )
  })

  it('2. registerCommand 同 id 第二次抛 CommandAlreadyRegisteredError', () => {
    const c: Command = { id: 'workspace.test', title: 'Test', handler: vi.fn() }
    mgr.registerCommand(c)
    expect(() => mgr.registerCommand({ ...c })).toThrow(CommandAlreadyRegisteredError)
    // unregister 后可重注册
    mgr.unregisterCommand('workspace.test')
    expect(() => mgr.registerCommand({ ...c })).not.toThrow()
  })
})

describe('WorkspaceManager — openPanel（验收路径 2 幂等）', () => {
  it('3. openPanel 未打开 → adapter.mountPanel 调用一次 + 触发 onPanelActivated', () => {
    const desc = makeDescriptor('ChatHub')
    mgr.registerPanel(desc)
    const activatedCb = vi.fn()
    mgr.onPanelActivated(activatedCb)

    mgr.openPanel('ChatHub', { greeting: 'hi' })

    expect(adapter.spies.mountPanel).toHaveBeenCalledTimes(1)
    expect(adapter.spies.mountPanel).toHaveBeenCalledWith(desc, { greeting: 'hi' })
    expect(activatedCb).toHaveBeenCalledTimes(1)
    expect(activatedCb).toHaveBeenCalledWith('ChatHub')
    expect(mgr.isPanelOpen('ChatHub')).toBe(true)
    expect(mgr.getActivePanel()).toBe('ChatHub')
    expect(mgr.getContextKey('panel.ChatHub.visible')).toBe(true)
    expect(mgr.getContextKey('activePanel')).toBe('ChatHub')
  })

  it('4. openPanel 已打开 → 不重 mount + activated 不重复', () => {
    mgr.registerPanel(makeDescriptor('ChatHub'))
    const activatedCb = vi.fn()
    mgr.onPanelActivated(activatedCb)

    mgr.openPanel('ChatHub')
    mgr.openPanel('ChatHub') // 幂等
    expect(adapter.spies.mountPanel).toHaveBeenCalledTimes(1)
    // active 未变 → 第二次不重发 activated
    expect(activatedCb).toHaveBeenCalledTimes(1)
  })

  it('5. openPanel 未注册 id → 抛 PanelNotRegisteredError', () => {
    expect(() => mgr.openPanel('Nope')).toThrow(PanelNotRegisteredError)
    expect(adapter.spies.mountPanel).not.toHaveBeenCalled()
  })
})

describe('WorkspaceManager — revealPanel（验收路径 3 两条路径）', () => {
  it('6. revealPanel 未注册 → 抛', () => {
    expect(() => mgr.revealPanel('Nope')).toThrow(PanelNotRegisteredError)
  })

  it('7. 路径 A：未打开 → 走 openPanel（mountPanel + activated）', () => {
    mgr.registerPanel(makeDescriptor('ChatHub'))
    const activatedCb = vi.fn()
    mgr.onPanelActivated(activatedCb)
    mgr.revealPanel('ChatHub')
    expect(adapter.spies.mountPanel).toHaveBeenCalledTimes(1)
    expect(adapter.spies.revealPanel).not.toHaveBeenCalled()
    expect(activatedCb).toHaveBeenCalledWith('ChatHub')
  })

  it('8. 路径 B：已打开非 active → adapter.revealPanel + activated 触发', () => {
    mgr.registerPanel(makeDescriptor('ChatHub'))
    mgr.registerPanel(makeDescriptor('Settings'))
    mgr.openPanel('ChatHub')
    mgr.openPanel('Settings') // Settings 现在 active

    const activatedCb = vi.fn()
    mgr.onPanelActivated(activatedCb)
    mgr.revealPanel('ChatHub')
    expect(adapter.spies.revealPanel).toHaveBeenCalledWith('ChatHub')
    expect(mgr.getActivePanel()).toBe('ChatHub')
    expect(activatedCb).toHaveBeenCalledTimes(1)

    // 已 active → no-op
    activatedCb.mockClear()
    adapter.spies.revealPanel.mockClear()
    mgr.revealPanel('ChatHub')
    expect(adapter.spies.revealPanel).not.toHaveBeenCalled()
    expect(activatedCb).not.toHaveBeenCalled()
  })
})

describe('WorkspaceManager — closePanel', () => {
  it('9. closePanel 已打开 → unmountPanel + visible=false + deactivated 触发', async () => {
    mgr.registerPanel(makeDescriptor('ChatHub'))
    mgr.openPanel('ChatHub')
    const deactivatedCb = vi.fn()
    mgr.onPanelDeactivated(deactivatedCb)
    const result = await mgr.closePanel('ChatHub')
    expect(result).toBe(true)
    expect(adapter.spies.unmountPanel).toHaveBeenCalledWith('ChatHub')
    expect(mgr.isPanelOpen('ChatHub')).toBe(false)
    expect(mgr.getContextKey('panel.ChatHub.visible')).toBe(false)
    expect(mgr.getActivePanel()).toBeNull()
    expect(deactivatedCb).toHaveBeenCalledWith('ChatHub')
  })

  it('10. closePanel beforeClose 返 false → 拒绝关闭', async () => {
    const beforeClose = vi.fn().mockResolvedValue(false)
    mgr.registerPanel(makeDescriptor('Editor', { beforeClose }))
    mgr.openPanel('Editor')
    const result = await mgr.closePanel('Editor')
    expect(result).toBe(false)
    expect(beforeClose).toHaveBeenCalled()
    expect(adapter.spies.unmountPanel).not.toHaveBeenCalled()
    expect(mgr.isPanelOpen('Editor')).toBe(true)
    // force=true 跳过 beforeClose
    beforeClose.mockClear()
    const r2 = await mgr.closePanel('Editor', true)
    expect(r2).toBe(true)
    expect(beforeClose).not.toHaveBeenCalled()
    expect(mgr.isPanelOpen('Editor')).toBe(false)
  })

  it('11. closePanel 未打开 / 未注册 → false 返（不抛）', async () => {
    expect(await mgr.closePanel('Nonexistent')).toBe(false)
    mgr.registerPanel(makeDescriptor('ChatHub'))
    expect(await mgr.closePanel('ChatHub')).toBe(false) // 未打开
  })

  it('11b. closePanel beforeClose 返 true → 正常走 unmount', async () => {
    const beforeClose = vi.fn().mockResolvedValue(true)
    mgr.registerPanel(makeDescriptor('Editor', { beforeClose }))
    mgr.openPanel('Editor')
    const r = await mgr.closePanel('Editor')
    expect(r).toBe(true)
    expect(beforeClose).toHaveBeenCalled()
    expect(adapter.spies.unmountPanel).toHaveBeenCalledWith('Editor')
    expect(mgr.isPanelOpen('Editor')).toBe(false)
  })
})

describe('WorkspaceManager — executeCommand + when DSL（验收路径 4）', () => {
  it('12. 未注册命令 → 抛', async () => {
    await expect(mgr.executeCommand('nope')).rejects.toThrow(/not found/)
  })

  it('13. when=false → 抛 disabled；when=true → handler 执行 + 返值', async () => {
    const handler = vi.fn().mockResolvedValue('ok')
    mgr.registerCommand({ id: 'cmd.a', title: 'A', when: 'allowed', handler })
    // 默认 ctx 无 allowed → false
    await expect(mgr.executeCommand('cmd.a')).rejects.toThrow(/disabled/)
    expect(handler).not.toHaveBeenCalled()

    mgr.setContextKey('allowed', true)
    const r = await mgr.executeCommand('cmd.a')
    expect(r).toBe('ok')
    expect(handler).toHaveBeenCalledTimes(1)

    // 无 when → 永远可执行
    const h2 = vi.fn()
    mgr.registerCommand({ id: 'cmd.b', title: 'B', handler: h2 })
    await mgr.executeCommand('cmd.b')
    expect(h2).toHaveBeenCalled()
  })

  it('14. listCommands(filterByWhen=true) → 过滤掉 when=false 的', () => {
    mgr.registerCommand({
      id: 'cmd.a',
      title: 'A',
      when: 'allowed',
      handler: vi.fn(),
    })
    mgr.registerCommand({ id: 'cmd.b', title: 'B', handler: vi.fn() })
    expect(mgr.listCommands().length).toBe(2)
    expect(mgr.listCommands(true).length).toBe(1)
    expect(mgr.listCommands(true)[0]!.id).toBe('cmd.b')
    mgr.setContextKey('allowed', true)
    expect(mgr.listCommands(true).length).toBe(2)
  })

  it('14b. isWhenSatisfied 表达式损坏 → fail-closed 返 false + log', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    expect(mgr.isWhenSatisfied('&& bad ||')).toBe(false)
    expect(errSpy).toHaveBeenCalled()
    errSpy.mockRestore()
  })

  it('14d. unregisterPanel + listOpenPanels 转发', () => {
    mgr.registerPanel(makeDescriptor('A'))
    mgr.registerPanel(makeDescriptor('B'))
    mgr.openPanel('A')
    mgr.openPanel('B')
    expect(mgr.listOpenPanels().sort()).toEqual(['A', 'B'])
    mgr.unregisterPanel('A')
    // unregister 后 registry 拿不到（openPanel 再试 → 抛）
    expect(() => mgr.openPanel('A')).toThrow(PanelNotRegisteredError)
  })

  it('14c. onPanelActivated / onPanelDeactivated 返回的 unsubscribe 函数生效', () => {
    mgr.registerPanel(makeDescriptor('ChatHub'))
    const activatedCb = vi.fn()
    const deactivatedCb = vi.fn()
    const unsubA = mgr.onPanelActivated(activatedCb)
    const unsubD = mgr.onPanelDeactivated(deactivatedCb)
    unsubA()
    unsubD()
    // unsubscribe 后 open/close 不再触发 cb
    mgr.openPanel('ChatHub')
    expect(activatedCb).not.toHaveBeenCalled()
    return mgr.closePanel('ChatHub').then(() => {
      expect(deactivatedCb).not.toHaveBeenCalled()
    })
  })
})

describe('WorkspaceManager — serialize/deserialize（验收路径 5）', () => {
  it('15. 绑 adapter：serialize 透传 adapter.serialize；deserialize 透传 + 异常吞掉', async () => {
    adapter.state.serializedJson = '{"a":1}'
    expect(mgr.serialize()).toBe('{"a":1}')
    expect(adapter.spies.serialize).toHaveBeenCalledTimes(1)

    await mgr.deserialize('{"b":2}')
    expect(adapter.spies.deserialize).toHaveBeenCalledWith('{"b":2}')

    // adapter.deserialize 抛 → log + 不抛回 caller
    adapter.spies.deserialize.mockImplementation(() => {
      throw new Error('bad layout')
    })
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    await expect(mgr.deserialize('garbage')).resolves.toBeUndefined()
    expect(errSpy).toHaveBeenCalled()
    errSpy.mockRestore()
  })

  it('16. 未绑 adapter：serialize 返 default empty + deserialize no-op', async () => {
    const bare = new WorkspaceManager()
    expect(bare.serialize()).toBe('{"grid":null}')
    await expect(bare.deserialize('{"x":1}')).resolves.toBeUndefined() // 不抛
  })
})

describe('WorkspaceManager — contextKey & persistence（验收路径 6）', () => {
  it('17. openPanel 自动 setContextKey panel.X.visible + activePanel + subscribe 触发', () => {
    mgr.registerPanel(makeDescriptor('ChatHub'))
    const sub = vi.fn()
    const unsub = mgr.subscribeContextKeys(['panel.ChatHub.visible', 'activePanel'], sub)
    mgr.openPanel('ChatHub')
    // 至少触发 2 次（panel.X.visible=true + activePanel=ChatHub）
    expect(sub).toHaveBeenCalled()
    expect(mgr.getContextKey('panel.ChatHub.visible')).toBe(true)
    expect(mgr.getContextKey('activePanel')).toBe('ChatHub')
    // unsubscribe 后无关变更不触发
    sub.mockClear()
    unsub()
    mgr.setContextKey('panel.ChatHub.visible', false)
    expect(sub).not.toHaveBeenCalled()
  })

  it('18. persistence 注入 + 三个 KV 便捷方法走持久化；未注入时 no-op', async () => {
    const persistence = makeSpyPersistence()
    persistence.spies.loadLayout.mockResolvedValue('{"loaded":true}')
    persistence.spies.loadLastActive.mockResolvedValue('SavedPanel')
    const mgr2 = new WorkspaceManager({ persistence })
    const adapter2 = makeSpyAdapter()
    mgr2.bindAdapter(adapter2)

    // loadLayoutFromKv → persistence.loadLayout → adapter.deserialize
    await mgr2.loadLayoutFromKv()
    expect(persistence.spies.loadLayout).toHaveBeenCalled()
    expect(adapter2.spies.deserialize).toHaveBeenCalledWith('{"loaded":true}')

    // saveLayoutToKv → persistence.saveLayout(serialize())
    adapter2.state.serializedJson = '{"saved":1}'
    await mgr2.saveLayoutToKv()
    expect(persistence.spies.saveLayout).toHaveBeenCalledWith('{"saved":1}')

    // loadLastActiveFromKv 透传
    expect(await mgr2.loadLastActiveFromKv()).toBe('SavedPanel')

    // saveLastActiveToKv：无 active 时 no-op；有 active 才写
    await mgr2.saveLastActiveToKv()
    expect(persistence.spies.saveLastActive).not.toHaveBeenCalled()
    mgr2.registerPanel(makeDescriptor('ChatHub'))
    mgr2.openPanel('ChatHub')
    await mgr2.saveLastActiveToKv()
    expect(persistence.spies.saveLastActive).toHaveBeenCalledWith('ChatHub')

    // 未注入 persistence：四个便捷方法都 no-op + 不抛
    const bare = new WorkspaceManager()
    await expect(bare.loadLayoutFromKv()).resolves.toBeUndefined()
    await expect(bare.saveLayoutToKv()).resolves.toBeUndefined()
    expect(await bare.loadLastActiveFromKv()).toBeNull()
    await expect(bare.saveLastActiveToKv()).resolves.toBeUndefined()
  })

  it('18b. loadLayoutFromKv：persistence 返 null → 不调 adapter.deserialize', async () => {
    const persistence = makeSpyPersistence()
    persistence.spies.loadLayout.mockResolvedValue(null)
    const mgr2 = new WorkspaceManager({ persistence })
    const adapter2 = makeSpyAdapter()
    mgr2.bindAdapter(adapter2)
    await mgr2.loadLayoutFromKv()
    expect(persistence.spies.loadLayout).toHaveBeenCalled()
    expect(adapter2.spies.deserialize).not.toHaveBeenCalled()
  })
})

describe('WorkspaceManager — adapter 事件回灌（review P0 F-2.1/2.2/2.3/1.1 修复）', () => {
  it('19. bindAdapter 触发 subscribeEvents；adapter mount 事件回灌 openPanels + contextKey + activated', () => {
    expect(adapter.spies.subscribeEvents).toHaveBeenCalledTimes(1)
    const events = adapter.events.current!
    expect(events).toBeTruthy()

    // 模拟 deserialize 后 dockview 端 mount 一个未通过 manager.openPanel 的 panel
    mgr.registerPanel(makeDescriptor('Restored'))
    const activatedCb = vi.fn()
    mgr.onPanelActivated(activatedCb)

    events.onPanelMounted('Restored')
    expect(mgr.isPanelOpen('Restored')).toBe(true)
    expect(mgr.getContextKey('panel.Restored.visible')).toBe(true)
    // mount 事件不主动切 active（active 是 onActivePanelChanged 单独触发）
    expect(mgr.getActivePanel()).toBeNull()
    expect(activatedCb).not.toHaveBeenCalled()

    // 幂等：再次回灌相同 id 不重复
    events.onPanelMounted('Restored')
    expect(mgr.listOpenPanels()).toEqual(['Restored'])
  })

  it('20. adapter 用户点 tab 切 active 事件回灌 activePanelId + activated 触发', () => {
    const events = adapter.events.current!
    mgr.registerPanel(makeDescriptor('A'))
    mgr.registerPanel(makeDescriptor('B'))
    events.onPanelMounted('A')
    events.onPanelMounted('B')

    const activatedCb = vi.fn()
    mgr.onPanelActivated(activatedCb)

    events.onActivePanelChanged('A')
    expect(mgr.getActivePanel()).toBe('A')
    expect(mgr.getContextKey('activePanel')).toBe('A')
    expect(activatedCb).toHaveBeenCalledWith('A')

    events.onActivePanelChanged('B')
    expect(mgr.getActivePanel()).toBe('B')
    expect(activatedCb).toHaveBeenCalledWith('B')

    // 幂等：active 已是 B，再回灌 B 不重复触发
    activatedCb.mockClear()
    events.onActivePanelChanged('B')
    expect(activatedCb).not.toHaveBeenCalled()

    // null = 无 active（最后一个 panel 被关）
    events.onActivePanelChanged(null)
    expect(mgr.getActivePanel()).toBeNull()
    expect(mgr.getContextKey('activePanel')).toBeNull()
  })

  it('21. adapter 用户点 tab ✕ 关 panel 事件回灌 openPanels 删除 + visible=false + deactivated 触发', () => {
    const events = adapter.events.current!
    mgr.registerPanel(makeDescriptor('A'))
    events.onPanelMounted('A')
    events.onActivePanelChanged('A')

    const deactivatedCb = vi.fn()
    mgr.onPanelDeactivated(deactivatedCb)

    events.onPanelRemoved('A')
    expect(mgr.isPanelOpen('A')).toBe(false)
    expect(mgr.getContextKey('panel.A.visible')).toBe(false)
    // active 是 'A' → 自动清空 active
    expect(mgr.getActivePanel()).toBeNull()
    expect(mgr.getContextKey('activePanel')).toBeNull()
    expect(deactivatedCb).toHaveBeenCalledWith('A')

    // 幂等：再次 remove 同 id 不重复触发
    deactivatedCb.mockClear()
    events.onPanelRemoved('A')
    expect(deactivatedCb).not.toHaveBeenCalled()
  })
})

describe('WorkspaceManager — review P1 修复（params 透传 / 变更事件 / 并发护栏）', () => {
  it('22. revealPanel(id, params) 未打开走 openPanel → adapter.mountPanel 收到 params (F-1.2)', () => {
    const desc = makeDescriptor('ChatHub')
    mgr.registerPanel(desc)
    mgr.revealPanel('ChatHub', { greeting: 'reveal-with-params' })
    expect(adapter.spies.mountPanel).toHaveBeenCalledWith(desc, {
      greeting: 'reveal-with-params',
    })
    // 已打开路径下 revealPanel 不再透传 params 给 adapter.mountPanel（只触发 revealPanel）
    mgr.registerPanel(makeDescriptor('Other'))
    mgr.openPanel('Other') // 切走 active，让下次 revealPanel('ChatHub') 走 already-open 路径
    adapter.spies.mountPanel.mockClear() // 清掉 openPanel('Other') 的调用
    mgr.revealPanel('ChatHub', { ignored: true })
    expect(adapter.spies.mountPanel).not.toHaveBeenCalled()
    expect(adapter.spies.revealPanel).toHaveBeenCalledWith('ChatHub')
  })

  it('23. onPanelsChanged / onCommandsChanged 注册时触发；unsubscribe 后不触发 (F-5.6/4.5/3.4)', () => {
    const panelsCb = vi.fn()
    const commandsCb = vi.fn()
    const unsubP = mgr.onPanelsChanged(panelsCb)
    const unsubC = mgr.onCommandsChanged(commandsCb)

    mgr.registerPanel(makeDescriptor('A'))
    expect(panelsCb).toHaveBeenCalledTimes(1)
    mgr.unregisterPanel('A')
    expect(panelsCb).toHaveBeenCalledTimes(2)

    mgr.registerCommand({ id: 'cmd.x', title: 'X', handler: vi.fn() })
    expect(commandsCb).toHaveBeenCalledTimes(1)
    mgr.unregisterCommand('cmd.x')
    expect(commandsCb).toHaveBeenCalledTimes(2)
    // 不存在的命令 unregister 不应再 emit
    mgr.unregisterCommand('cmd.missing')
    expect(commandsCb).toHaveBeenCalledTimes(2)

    unsubP()
    unsubC()
    mgr.registerPanel(makeDescriptor('B'))
    mgr.registerCommand({ id: 'cmd.y', title: 'Y', handler: vi.fn() })
    expect(panelsCb).toHaveBeenCalledTimes(2) // 未变
    expect(commandsCb).toHaveBeenCalledTimes(2) // 未变
  })

  it('24. closePanel 并发护栏：beforeClose await 期间二次 close → 直接返 false，hook 不重复触发 (F-8.1)', async () => {
    let resolveBeforeClose: (v: boolean) => void = () => {}
    const beforeClose = vi.fn(
      () =>
        new Promise<boolean>((resolve) => {
          resolveBeforeClose = resolve
        }),
    )
    mgr.registerPanel(makeDescriptor('Editor', { beforeClose }))
    mgr.openPanel('Editor')

    // 第 1 次 close：进 beforeClose 阻塞中
    const p1 = mgr.closePanel('Editor')
    // beforeClose 已被调用 1 次但未 resolve
    expect(beforeClose).toHaveBeenCalledTimes(1)
    // 第 2 次 close：被并发护栏挡，立即返 false，不再调 beforeClose
    const r2 = await mgr.closePanel('Editor')
    expect(r2).toBe(false)
    expect(beforeClose).toHaveBeenCalledTimes(1)

    // 现在 resolve 第 1 次的 beforeClose=true → 第 1 次走完 unmount
    resolveBeforeClose(true)
    const r1 = await p1
    expect(r1).toBe(true)
    expect(adapter.spies.unmountPanel).toHaveBeenCalledWith('Editor')
    expect(mgr.isPanelOpen('Editor')).toBe(false)
  })
})
