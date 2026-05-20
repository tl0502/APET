// Workspace persistence 单测（6 case）—— deps 注入 mock KV

import { beforeEach, describe, expect, it, vi } from 'vitest'

import {
  KvWorkspacePersistence,
  WORKSPACE_LAST_ACTIVE_KV_KEY,
  WORKSPACE_LAYOUT_KV_KEY,
  WORKSPACE_LAYOUT_SCHEMA_V,
} from '../persistence'

function makeDeps() {
  return {
    getKv: vi.fn<(key: string) => Promise<string | null>>(),
    setKv: vi.fn<(key: string, value: string) => Promise<void>>(),
  }
}

let consoleErrSpy: ReturnType<typeof vi.spyOn>
let consoleWarnSpy: ReturnType<typeof vi.spyOn>
beforeEach(() => {
  consoleErrSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
  consoleWarnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {})
})

describe('KvWorkspacePersistence', () => {
  it('saveLayout + loadLayout 往返（含 schema v wrapper）', async () => {
    const deps = makeDeps()
    deps.setKv.mockResolvedValue(undefined)
    const p = new KvWorkspacePersistence(deps)

    await p.saveLayout('{"grid":"x"}')
    expect(deps.setKv).toHaveBeenCalledTimes(1)
    const [key, value] = deps.setKv.mock.calls[0]!
    expect(key).toBe(WORKSPACE_LAYOUT_KV_KEY)
    const wrapper = JSON.parse(value) as { v: number; dockview: string }
    expect(wrapper.v).toBe(WORKSPACE_LAYOUT_SCHEMA_V)
    expect(wrapper.dockview).toBe('{"grid":"x"}')

    deps.getKv.mockResolvedValue(value)
    const loaded = await p.loadLayout()
    expect(loaded).toBe('{"grid":"x"}')
  })

  it('loadLayout KV 不存在 → null', async () => {
    const deps = makeDeps()
    deps.getKv.mockResolvedValue(null)
    const p = new KvWorkspacePersistence(deps)
    expect(await p.loadLayout()).toBeNull()
    // 空串也算不存在
    deps.getKv.mockResolvedValue('')
    expect(await p.loadLayout()).toBeNull()
  })

  it('loadLayout JSON 损坏 → log + clear KV + null', async () => {
    const deps = makeDeps()
    deps.getKv.mockResolvedValue('not-json{{{')
    deps.setKv.mockResolvedValue(undefined)
    const p = new KvWorkspacePersistence(deps)
    const r = await p.loadLayout()
    expect(r).toBeNull()
    expect(consoleWarnSpy).toHaveBeenCalled()
    expect(deps.setKv).toHaveBeenCalledWith(WORKSPACE_LAYOUT_KV_KEY, '')
  })

  it('loadLayout schema v 不匹配 → log + clear + null', async () => {
    const deps = makeDeps()
    deps.getKv.mockResolvedValue(JSON.stringify({ v: 99, dockview: '{}' }))
    deps.setKv.mockResolvedValue(undefined)
    const p = new KvWorkspacePersistence(deps)
    expect(await p.loadLayout()).toBeNull()
    expect(consoleWarnSpy).toHaveBeenCalled()
    expect(deps.setKv).toHaveBeenCalledWith(WORKSPACE_LAYOUT_KV_KEY, '')
  })

  it('loadLayout wrapper 结构不合法 → clear + null', async () => {
    const deps = makeDeps()
    // 缺字段
    deps.getKv.mockResolvedValue(JSON.stringify({ v: 1 }))
    deps.setKv.mockResolvedValue(undefined)
    const p = new KvWorkspacePersistence(deps)
    expect(await p.loadLayout()).toBeNull()
    expect(deps.setKv).toHaveBeenCalledWith(WORKSPACE_LAYOUT_KV_KEY, '')
    // 类型错（dockview 不是 string）
    deps.setKv.mockClear()
    deps.getKv.mockResolvedValue(JSON.stringify({ v: 1, dockview: 123 }))
    expect(await p.loadLayout()).toBeNull()
    expect(deps.setKv).toHaveBeenCalledWith(WORKSPACE_LAYOUT_KV_KEY, '')
  })

  it('loadLayout getKv 抛错 → log + null（不阻塞）；saveLayout setKv 抛错 → fire-and-log', async () => {
    const deps = makeDeps()
    deps.getKv.mockRejectedValue(new Error('IPC down'))
    const p = new KvWorkspacePersistence(deps)
    expect(await p.loadLayout()).toBeNull()
    expect(consoleErrSpy).toHaveBeenCalled()

    deps.setKv.mockRejectedValue(new Error('IPC fail'))
    await expect(p.saveLayout('{}')).resolves.toBeUndefined()

    // last_active 同样的容错
    deps.getKv.mockRejectedValue(new Error('IPC down'))
    expect(await p.loadLastActive()).toBeNull()
    await expect(p.saveLastActive('ChatHub')).resolves.toBeUndefined()

    // saveLastActive 成功路径 + loadLastActive 读回
    deps.setKv.mockResolvedValue(undefined)
    await p.saveLastActive('ChatHub')
    expect(deps.setKv).toHaveBeenCalledWith(WORKSPACE_LAST_ACTIVE_KV_KEY, 'ChatHub')
    deps.getKv.mockResolvedValue('ChatHub')
    expect(await p.loadLastActive()).toBe('ChatHub')
    // null + 空串都视为不存在
    deps.getKv.mockResolvedValue(null)
    expect(await p.loadLastActive()).toBeNull()
    deps.getKv.mockResolvedValue('')
    expect(await p.loadLastActive()).toBeNull()
  })
})
