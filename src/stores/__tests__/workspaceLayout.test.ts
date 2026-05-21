// workspaceLayout store 单测（#33 phase B-redo）—— 9 case
//
// 覆盖：setCategory / setItem / item-per-category 记忆 / setMasterWidth 边界 /
//      loadFromKv 完整 / loadFromKv 损坏自愈 / loadFromKv 未知 id fallback /
//      currentItem getter / saveMasterWidth debounce

import { setActivePinia, createPinia } from 'pinia'
import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest'

import { useWorkspaceLayoutStore } from '../workspaceLayout'

// === Mock @/services/config ===

vi.mock('@/services/config', () => ({
  getConfig: vi.fn(),
  setConfig: vi.fn(),
}))

import { getConfig, setConfig } from '@/services/config'
const mockGetConfig = getConfig as unknown as Mock
const mockSetConfig = setConfig as unknown as Mock

// === setup ===

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
  mockGetConfig.mockResolvedValue(null)
  mockSetConfig.mockResolvedValue(undefined)
})

// === tests ===

describe('workspaceLayout store', () => {
  it('case 1: setCategory 切换 + 持久化', async () => {
    const store = useWorkspaceLayoutStore()
    expect(store.currentCategory).toBe('config') // default
    store.setCategory('task')
    expect(store.currentCategory).toBe('task')
    // KV save 是 async 但 fire-and-forget；直接 next tick 后断言 mock 被调
    await Promise.resolve()
    expect(mockSetConfig).toHaveBeenCalledWith('workspace:current_category', 'task')
  })

  it('case 2: setItem 在当前类别下记忆', async () => {
    const store = useWorkspaceLayoutStore()
    store.setCategory('config')
    store.setItem('SettingsAbout')
    await Promise.resolve()
    expect(store.currentItem).toBe('SettingsAbout')
    expect(mockSetConfig).toHaveBeenCalledWith(
      'workspace:item_per_category',
      expect.stringContaining('SettingsAbout'),
    )
  })

  it('case 3: chat 类别下 setItem noop（itemId 由 ConversationStore 管理）', () => {
    const store = useWorkspaceLayoutStore()
    store.setCategory('chat')
    store.setItem('SettingsTheme') // 应被忽略
    expect(store.currentItem).toBe(null)
  })

  it('case 4: item-per-category 跨切换记忆', () => {
    const store = useWorkspaceLayoutStore()
    store.setCategory('config')
    store.setItem('SettingsAbout')
    store.setCategory('task')
    store.setItem('TasksPomodoro')
    store.setCategory('config') // 回到 config
    expect(store.currentItem).toBe('SettingsAbout') // 仍是上次选中
    store.setCategory('task') // 回到 task
    expect(store.currentItem).toBe('TasksPomodoro')
  })

  it('case 5: setMasterWidth 边界 clamp + debounce 持久化', async () => {
    vi.useFakeTimers()
    const store = useWorkspaceLayoutStore()
    store.setMasterWidth(50) // 低于 min
    expect(store.masterWidth).toBe(store._MASTER_WIDTH_MIN)
    store.setMasterWidth(500) // 高于 max
    expect(store.masterWidth).toBe(store._MASTER_WIDTH_MAX)
    store.setMasterWidth(NaN) // 非法
    expect(store.masterWidth).toBe(store._MASTER_WIDTH_DEFAULT)
    store.setMasterWidth(260) // 正常
    expect(store.masterWidth).toBe(260)

    // 多次调 debounce → 只 save 一次
    expect(mockSetConfig).not.toHaveBeenCalled()
    vi.advanceTimersByTime(store._SAVE_DEBOUNCE_MS)
    await vi.runAllTimersAsync()
    expect(mockSetConfig).toHaveBeenCalledWith('workspace:master_width', '260')
    expect(mockSetConfig).toHaveBeenCalledTimes(1)

    vi.useRealTimers()
  })

  it('case 6: loadFromKv 完整加载', async () => {
    mockGetConfig.mockImplementation(async (key: string) => {
      if (key === 'workspace:current_category') return 'task'
      if (key === 'workspace:item_per_category')
        return JSON.stringify({
          chat: null,
          task: 'TasksPomodoro',
          creation: 'SettingsPersona',
          config: 'SettingsAbout',
        })
      if (key === 'workspace:master_width') return '300'
      return null
    })

    const store = useWorkspaceLayoutStore()
    await store.loadFromKv()

    expect(store.currentCategory).toBe('task')
    expect(store.currentItem).toBe('TasksPomodoro') // task 类别下
    store.setCategory('config')
    expect(store.currentItem).toBe('SettingsAbout')
    expect(store.masterWidth).toBe(300)
  })

  it('case 7: loadFromKv item_per_category JSON 损坏自愈走 default', async () => {
    mockGetConfig.mockImplementation(async (key: string) => {
      if (key === 'workspace:item_per_category') return '{ invalid json'
      return null
    })
    const store = useWorkspaceLayoutStore()
    await store.loadFromKv()
    // 损坏 → 走 default 初始化（config 下默认 SettingsTheme）
    expect(store.currentCategory).toBe('config')
    expect(store.currentItem).toBe('SettingsTheme')
  })

  it('case 8: loadFromKv 未知 panel id（老用户残留）→ fallback 到 defaultItemId', async () => {
    mockGetConfig.mockImplementation(async (key: string) => {
      if (key === 'workspace:item_per_category')
        return JSON.stringify({
          chat: null,
          task: 'WorkspaceObsoletePanel', // 已删除的旧 id
          creation: 'SettingsPersona',
          config: 'SettingsTheme',
        })
      return null
    })
    const store = useWorkspaceLayoutStore()
    await store.loadFromKv()
    store.setCategory('task')
    expect(store.currentItem).toBe('TasksReminder') // task 的 defaultItemId
  })

  it('case 9: setCategoryAndItem 同时切类别+项 + 持久化', async () => {
    const store = useWorkspaceLayoutStore()
    store.setCategoryAndItem('creation', 'SettingsPersona')
    await Promise.resolve()
    expect(store.currentCategory).toBe('creation')
    expect(store.currentItem).toBe('SettingsPersona')
    expect(mockSetConfig).toHaveBeenCalledWith('workspace:current_category', 'creation')
  })
})
