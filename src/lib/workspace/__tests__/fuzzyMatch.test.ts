// fuzzyMatch 单测（6 case）—— 命令面板模糊匹配

import { describe, expect, it } from 'vitest'

import { fuzzyFilter, fuzzyMatch } from '../fuzzyMatch'

describe('fuzzyMatch', () => {
  it('完全顺序匹配 → 命中 + indices', () => {
    const r = fuzzyMatch('rc', 'Reveal Chat')
    expect(r).not.toBeNull()
    expect(r!.indices.length).toBe(2)
    expect(r!.score).toBeGreaterThan(0)
  })

  it('未全部命中 → null', () => {
    expect(fuzzyMatch('xz', 'Reveal Chat')).toBeNull()
    expect(fuzzyMatch('abc', 'def')).toBeNull()
    // query 比 target 长 → null
    expect(fuzzyMatch('reveal long', 'short')).toBeNull()
  })

  it('score 排序：前缀匹配 > 中间匹配 > 跳跃匹配', () => {
    const prefix = fuzzyMatch('rev', 'Reveal Chat')!
    const middle = fuzzyMatch('cha', 'Reveal Chat')!
    const skip = fuzzyMatch('rc', 'Reveal Chat')!
    expect(prefix.score).toBeGreaterThan(middle.score)
    // 连续 + 前缀 > 跳跃
    expect(prefix.score).toBeGreaterThan(skip.score)
  })

  it('大小写不敏感', () => {
    const r1 = fuzzyMatch('RC', 'Reveal Chat')
    const r2 = fuzzyMatch('rc', 'Reveal Chat')
    expect(r1).not.toBeNull()
    expect(r2).not.toBeNull()
    expect(r1!.score).toBe(r2!.score)
  })

  it('空 query → 视为命中所有 target，score=0', () => {
    const r = fuzzyMatch('', 'Anything')
    expect(r).toEqual({ score: 0, indices: [] })
  })

  it('fuzzyFilter 按 score 降序排 + 命中过滤', () => {
    const items = [
      { id: 'a', text: 'Reveal Chat' },
      { id: 'b', text: 'Open Settings' },
      { id: 'c', text: 'Reveal Chat Hub' },
      { id: 'd', text: 'XYZ' },
    ]
    const out = fuzzyFilter('rc', items, (it) => it.text)
    // 'XYZ' 没 r → 过滤掉；'Open Settings' 没 r → 过滤掉
    // 'Reveal Chat' 与 'Reveal Chat Hub' 都命中
    expect(out.length).toBe(2)
    // 第一名 score >= 第二名
    expect(out[0]!.matchResult.score).toBeGreaterThanOrEqual(out[1]!.matchResult.score)
    // 每个返回项保留原字段
    expect(out[0]!.id).toBeDefined()
    expect(out[0]!.text).toBeDefined()
  })
})
