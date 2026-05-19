// roles.ts 单元测试（ADR-020 #30 follow-up D）。
//
// 覆盖：isPrimary / primaryLabels / PRIMARY_LABEL 三 API + 角色不可变性。

import { describe, expect, it } from 'vitest'

import { isPrimary, PRIMARY_LABEL, primaryLabels } from '@/lib/snap/roles'

describe('roles — isPrimary', () => {
  it('"pet" → true（当前唯一 primary）', () => {
    expect(isPrimary('pet')).toBe(true)
  })

  it('"chat" → false（secondary）', () => {
    expect(isPrimary('chat')).toBe(false)
  })

  it('未注册的窗 label → false', () => {
    expect(isPrimary('settings')).toBe(false)
    expect(isPrimary('tasks')).toBe(false)
    expect(isPrimary('')).toBe(false)
  })

  it('大小写敏感（"Pet" / "PET" → false）', () => {
    expect(isPrimary('Pet')).toBe(false)
    expect(isPrimary('PET')).toBe(false)
  })
})

describe('roles — PRIMARY_LABEL 常量', () => {
  it('值为 "pet"', () => {
    expect(PRIMARY_LABEL).toBe('pet')
  })

  it('与 isPrimary 一致', () => {
    expect(isPrimary(PRIMARY_LABEL)).toBe(true)
  })
})

describe('roles — primaryLabels', () => {
  it('返回所有 primary（仅 ["pet"]）', () => {
    expect(primaryLabels()).toEqual(['pet'])
  })

  it('每个返回值满足 isPrimary', () => {
    for (const label of primaryLabels()) {
      expect(isPrimary(label)).toBe(true)
    }
  })

  it('返回 readonly snapshot，调用方修改不影响后续调用', () => {
    const first = primaryLabels()
    // primaryLabels 返 Array.from(...) — 每次调用是新数组
    const second = primaryLabels()
    expect(first).not.toBe(second) // 不同引用
    expect(first).toEqual(second) // 但内容相同
  })
})
