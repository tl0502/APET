// S4 dragSession 状态机单测。
//
// 覆盖：Idle/Armed/Dragging/Preview 全部转移 / Armed 超时 / commit 各路径
//      / cancel 返 snapshot / detach 记录 / sourceId 不匹配时 onUserMove no-op。

import { beforeEach, describe, expect, it } from 'vitest'
import { detachHistory } from '../candidates'
import { constraintStore } from '../constraintStore'
import {
  ARMED_TIMEOUT_MS,
  DragSession,
  previewAnchorId,
  previewEdge,
  previewFinalRect,
  previewIntensity,
} from '../dragSession'
import { TRIGGER_ZONE } from '../geometry'
import type { Rect, SnapCandidate } from '../types'

const r = (x: number, y: number, w: number, h: number): Rect => ({ x, y, w, h })

const cand = (
  targetId: string,
  overrides: Partial<SnapCandidate> = {},
): SnapCandidate => ({
  // #30 follow-up D：commit 写 constraint.sourceId === movingId。
  // 测试默认占位 '__src__'；commit 验证用例需在 overrides 显式 movingId 与 arm sourceId 一致。
  movingId: '__src__',
  targetId,
  sourceEdge: 'right',
  targetEdge: 'left',
  offset: 0,
  finalRect: r(0, 0, 320, 320),
  score: 0.3,
  // T8 (#31 follow-up B)：distance 字段。TRIGGER_ZONE 已从 60→10，默认 5px 距离仍在 trigger zone 内
  distance: 5,
  ...overrides,
})

beforeEach(() => {
  constraintStore.clear()
  detachHistory.clear()
  previewAnchorId.value = null
  previewEdge.value = null
  previewIntensity.value = 0
  previewFinalRect.value = null
})

describe('DragSession — 初始状态', () => {
  it('new DragSession 起始 state.kind === "idle"', () => {
    const s = new DragSession()
    expect(s.state.kind).toBe('idle')
    expect(s.currentCandidate).toBeNull()
  })
})

describe('DragSession — Idle → arm → Armed', () => {
  it('arm 进入 Armed 并存 forestSnapshot', () => {
    const snap = new Map([['pet', r(0, 0, 320, 320)]])
    const s = new DragSession()
    s.arm('pet', snap, 1000)
    expect(s.state.kind).toBe('armed')
    if (s.state.kind !== 'armed') throw new Error('type narrow')
    expect(s.state.draggedId).toBe('pet')
    expect(s.state.forestSnapshot.get('pet')).toEqual(r(0, 0, 320, 320))
  })

  it('arm 深拷贝 snapshot（外部 mutate 不污染 session）', () => {
    const snap = new Map([['pet', r(0, 0, 320, 320)]])
    const s = new DragSession()
    s.arm('pet', snap, 1000)
    snap.set('pet', r(999, 999, 999, 999)) // 外部 mutate
    if (s.state.kind !== 'armed') throw new Error('type narrow')
    expect(s.state.forestSnapshot.get('pet')).toEqual(r(0, 0, 320, 320))
  })
})

describe('DragSession — Armed → onUserMove → Dragging/Preview', () => {
  it('Armed + onUserMove(null) → Dragging', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('pet', null)
    expect(s.state.kind).toBe('dragging')
  })

  it('Armed + onUserMove(candidate) → Preview', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('pet', cand('chat'))
    expect(s.state.kind).toBe('preview')
    expect(s.currentCandidate?.targetId).toBe('chat')
  })

  it('onUserMove sourceId 不匹配 → no-op', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('chat', cand('pet'))
    expect(s.state.kind).toBe('armed')
  })

  it('Idle 时 onUserMove → no-op', () => {
    const s = new DragSession()
    s.onUserMove('pet', cand('chat'))
    expect(s.state.kind).toBe('idle')
  })
})

describe('DragSession — Dragging ↔ Preview 切换', () => {
  it('Dragging → onUserMove(candidate) → Preview', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('pet', null)
    s.onUserMove('pet', cand('chat'))
    expect(s.state.kind).toBe('preview')
  })

  it('Preview → onUserMove(null) → 回 Dragging（lost candidate）', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('pet', cand('chat'))
    s.onUserMove('pet', null)
    expect(s.state.kind).toBe('dragging')
  })

  it('Preview → onUserMove(更高分 candidate) → Preview 切换 target', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('pet', cand('chat'))
    s.onUserMove('pet', cand('settings'))
    expect(s.currentCandidate?.targetId).toBe('settings')
  })
})

describe('DragSession — Armed 超时', () => {
  it('Armed 超 ARMED_TIMEOUT_MS 无 onMoved → checkArmedTimeout 返 true，state 回 Idle', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    expect(s.checkArmedTimeout(1000 + ARMED_TIMEOUT_MS)).toBe(false) // 边界，仍 Armed
    expect(s.state.kind).toBe('armed')
    expect(s.checkArmedTimeout(1000 + ARMED_TIMEOUT_MS + 1)).toBe(true)
    expect(s.state.kind).toBe('idle')
  })

  it('Dragging 状态下 checkArmedTimeout → 不影响', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('pet', null) // → Dragging
    expect(s.checkArmedTimeout(99999)).toBe(false)
    expect(s.state.kind).toBe('dragging')
  })
})

describe('DragSession — commit', () => {
  it('Preview commit → 写 constraint，state → Idle', () => {
    const s = new DragSession()
    s.arm('chat', new Map(), 1000)
    s.onUserMove('chat', cand('pet', { offset: 50, movingId: 'chat' }))
    const result = s.commit(2000)
    expect(result.committedConstraint?.targetId).toBe('pet')
    expect(result.committedConstraint?.offset).toBe(50)
    expect(result.detached).toBeNull()
    expect(constraintStore.get('chat')?.targetId).toBe('pet')
    expect(s.state.kind).toBe('idle')
  })

  it('Dragging commit（无 candidate）→ 不写 constraint，state → Idle', () => {
    const s = new DragSession()
    s.arm('chat', new Map(), 1000)
    s.onUserMove('chat', null)
    const result = s.commit()
    expect(result.committedConstraint).toBeNull()
    expect(s.state.kind).toBe('idle')
  })

  it('Armed commit（无 onMoved）→ 不写 constraint，state → Idle', () => {
    const s = new DragSession()
    s.arm('chat', new Map(), 1000)
    const result = s.commit()
    expect(result.committedConstraint).toBeNull()
    expect(s.state.kind).toBe('idle')
  })

  it('Idle commit → no-op', () => {
    const s = new DragSession()
    expect(s.commit().committedConstraint).toBeNull()
  })

  it('commit 替换原 target 时记录 detach 到 detachHistory', () => {
    constraintStore.set({
      sourceId: 'chat',
      targetId: 'oldAnchor',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    const s = new DragSession()
    s.arm('chat', new Map(), 1000)
    s.onUserMove('chat', cand('newAnchor', { movingId: 'chat' }))
    const result = s.commit(2000)
    expect(result.detached).toEqual({ sourceId: 'chat', targetId: 'oldAnchor' })
    expect(detachHistory.isRecent('chat', 'oldAnchor', 2000)).toBe(true)
    expect(constraintStore.get('chat')?.targetId).toBe('newAnchor')
  })

  it('commit 同 target（refresh 不变）不记 detach', () => {
    constraintStore.set({
      sourceId: 'chat',
      targetId: 'pet',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    const s = new DragSession()
    s.arm('chat', new Map(), 1000)
    s.onUserMove('chat', cand('pet', { offset: 20, movingId: 'chat' }))
    const result = s.commit(2000)
    expect(result.detached).toBeNull()
    expect(constraintStore.get('chat')?.offset).toBe(20)
  })
})

describe('DragSession — cancel (ESC)', () => {
  it('Preview cancel → 返 snapshot，state → Idle，不写 constraint', () => {
    const snap = new Map([
      ['chat', r(100, 100, 320, 320)],
      ['pet', r(0, 0, 320, 320)],
    ])
    const s = new DragSession()
    s.arm('chat', snap, 1000)
    s.onUserMove('chat', cand('pet'))
    const returned = s.cancel()
    expect(returned?.get('chat')).toEqual(r(100, 100, 320, 320))
    expect(returned?.get('pet')).toEqual(r(0, 0, 320, 320))
    expect(s.state.kind).toBe('idle')
    expect(constraintStore.get('chat')).toBeUndefined()
  })

  it('Idle cancel → 返 null', () => {
    const s = new DragSession()
    expect(s.cancel()).toBeNull()
  })

  it('Armed cancel → 返 snapshot', () => {
    const snap = new Map([['pet', r(0, 0, 320, 320)]])
    const s = new DragSession()
    s.arm('pet', snap, 1000)
    expect(s.cancel()?.get('pet')).toEqual(r(0, 0, 320, 320))
  })
})

describe('DragSession — reset / 注入 store', () => {
  it('reset 立即回 Idle', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.reset()
    expect(s.state.kind).toBe('idle')
  })
})

// T2a (#31)：previewAnchorId reactive ref 同步语义。
// useSnapWindow watch 此 ref 并 emit 跨 webview，各窗根组件靠它切换 .snap-preview class。
// 测试约束：preview 时 = candidate.targetId；其他状态恒 null；任何转移立即生效。
describe('DragSession — previewAnchorId reactive 同步 (T2a #31)', () => {
  it('初始 / arm / Dragging 状态 → null', () => {
    const s = new DragSession()
    expect(previewAnchorId.value).toBeNull()

    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    expect(previewAnchorId.value).toBeNull()

    // Armed → Dragging（candidate=null）
    s.onUserMove('pet', null)
    expect(previewAnchorId.value).toBeNull()
  })

  it('onUserMove 拿到 candidate → 立即设为 candidate.targetId', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    expect(previewAnchorId.value).toBe('chat')
  })

  it('Preview → 失去 candidate（onUserMove null）→ 回 null', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    expect(previewAnchorId.value).toBe('chat')
    s.onUserMove('pet', null) // 拖离 trigger zone
    expect(previewAnchorId.value).toBeNull()
  })

  it('Preview 切到另一个 anchor → 更新 targetId', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    expect(previewAnchorId.value).toBe('chat')
    s.onUserMove('pet', cand('tasks'))
    expect(previewAnchorId.value).toBe('tasks')
  })

  it('commit 后 → null（preview → idle 转移）', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    expect(previewAnchorId.value).toBe('chat')
    s.commit()
    expect(previewAnchorId.value).toBeNull()
  })

  it('cancel 后 → null（任意状态 → idle）', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    expect(previewAnchorId.value).toBe('chat')
    s.cancel()
    expect(previewAnchorId.value).toBeNull()
  })

  it('checkArmedTimeout 触发回 Idle → null', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    // 模拟 1.5s 后 caller 调 checkArmedTimeout
    s.checkArmedTimeout(1000 + ARMED_TIMEOUT_MS + 500)
    expect(previewAnchorId.value).toBeNull()
  })

  it('reset 显式回 Idle → null', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    s.reset()
    expect(previewAnchorId.value).toBeNull()
  })
})

// T7 (#31 follow-up B)：previewEdge + previewIntensity reactive ref
// preview 时 = candidate.targetEdge / 1 - distance/TRIGGER_ZONE；其他状态 null/0。
describe('DragSession — previewEdge + previewIntensity (T7 #31 follow-up B)', () => {
  it('初始状态 previewEdge=null / previewIntensity=0', () => {
    new DragSession()
    expect(previewEdge.value).toBeNull()
    expect(previewIntensity.value).toBe(0)
  })

  it('preview 时 previewEdge = candidate.targetEdge', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat', { targetEdge: 'left', distance: 10 }))
    expect(previewEdge.value).toBe('left')
  })

  it('preview 时 previewIntensity = 1 - distance/TRIGGER_ZONE，clamp [0.25, 1]', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    // distance=0 → intensity=1
    s.onUserMove('pet', cand('chat', { distance: 0 }))
    expect(previewIntensity.value).toBeCloseTo(1, 5)
    // distance=30 (TRIGGER_ZONE/2) → intensity=0.5
    s.onUserMove('pet', cand('chat', { distance: TRIGGER_ZONE / 2 }))
    expect(previewIntensity.value).toBeCloseTo(0.5, 5)
    // distance=TRIGGER_ZONE → intensity 应 clamp 到 0.25 下限（避免完全消失）
    s.onUserMove('pet', cand('chat', { distance: TRIGGER_ZONE }))
    expect(previewIntensity.value).toBe(0.25)
  })

  it('退出 preview → previewEdge=null / previewIntensity=0', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    expect(previewEdge.value).not.toBeNull()
    expect(previewIntensity.value).toBeGreaterThan(0)
    s.onUserMove('pet', null)
    expect(previewEdge.value).toBeNull()
    expect(previewIntensity.value).toBe(0)
  })

  it('cancel / commit → 清零', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    s.cancel()
    expect(previewEdge.value).toBeNull()
    expect(previewIntensity.value).toBe(0)

    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), 1000)
    s.onUserMove('pet', cand('chat'))
    s.commit()
    expect(previewEdge.value).toBeNull()
    expect(previewIntensity.value).toBe(0)
  })
})

// T6 (#31 follow-up B)：group-drag 状态机 — anchor 拖动平移 dependents，不算 candidate / 不写 constraint。
describe('DragSession — group-drag 模式 (T6 #31 follow-up B)', () => {
  it("arm({mode:'group'}) → state.kind='group-drag'", () => {
    const s = new DragSession()
    const snap = new Map([['pet', r(0, 0, 320, 320)], ['chat', r(340, 0, 640, 480)]])
    s.arm('pet', snap, { mode: 'group' })
    expect(s.state.kind).toBe('group-drag')
    if (s.state.kind !== 'group-drag') throw new Error('narrow')
    expect(s.state.draggedId).toBe('pet')
    expect(s.state.forestSnapshot.get('chat')).toEqual(r(340, 0, 640, 480))
  })

  it('group-drag 期间 onUserMove(candidate) → 状态不变（不进 preview）', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), { mode: 'group' })
    s.onUserMove('pet', cand('chat'))
    expect(s.state.kind).toBe('group-drag')
    expect(previewAnchorId.value).toBeNull() // group-drag 不暴露 hover preview
    expect(previewEdge.value).toBeNull()
  })

  it('group-drag commit → committedConstraint=null + detached=null + 回 idle', () => {
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)]]), { mode: 'group' })
    const result = s.commit()
    expect(result.committedConstraint).toBeNull()
    expect(result.detached).toBeNull()
    expect(s.state.kind).toBe('idle')
  })

  it('group-drag cancel 返 forestSnapshot 供回滚', () => {
    const s = new DragSession()
    const snap = new Map([['pet', r(50, 50, 320, 320)], ['chat', r(370, 50, 640, 480)]])
    s.arm('pet', snap, { mode: 'group' })
    const restored = s.cancel()
    expect(restored).not.toBeNull()
    expect(restored?.get('pet')).toEqual(r(50, 50, 320, 320))
    expect(restored?.get('chat')).toEqual(r(370, 50, 640, 480))
    expect(s.state.kind).toBe('idle')
  })

  it("arm({mode:'source'}) 显式 / 不传 mode → 走原 armed 流程（向后兼容）", () => {
    const s = new DragSession()
    // 显式 source
    s.arm('chat', new Map([['chat', r(0, 0, 640, 480)]]), { mode: 'source' })
    expect(s.state.kind).toBe('armed')
    s.reset()
    // 旧签名 arm(id, snap, now: number) 仍工作（mode 默认 source）
    s.arm('chat', new Map([['chat', r(0, 0, 640, 480)]]), 12345)
    expect(s.state.kind).toBe('armed')
    if (s.state.kind === 'armed') expect(s.state.armedAt).toBe(12345)
  })
})

// Phase F (#31 follow-up C)：committing state 转移 + endCommitting + cancel 在 committing
describe('DragSession — committing state (Phase F #31 follow-up C)', () => {
  it('commit(now, sourceRect) preview → committing (写 store + 暴露 fromRect/toRect)', () => {
    const fromRect = r(280, 0, 320, 320) // 松手时 source 离 finalRect 偏 40px
    const finalRect = r(320, 0, 320, 320)
    const s = new DragSession()
    s.arm('chat', new Map([['chat', fromRect]]), 1000)
    s.onUserMove('chat', cand('pet', { finalRect, movingId: 'chat' }))
    const result = s.commit(2000, fromRect)
    expect(result.committedConstraint?.targetId).toBe('pet')
    expect(s.state.kind).toBe('committing')
    if (s.state.kind !== 'committing') throw new Error('narrow')
    expect(s.state.movingId).toBe('chat')
    expect(s.state.fromRect).toEqual(fromRect)
    expect(s.state.toRect).toEqual(finalRect)
    expect(s.state.t0).toBe(2000)
    // constraint 已写入
    expect(constraintStore.get('chat')?.targetId).toBe('pet')
  })

  it('commit(now) 不传 sourceRect → 原行为：直接回 idle（向后兼容）', () => {
    const s = new DragSession()
    s.arm('chat', new Map(), 1000)
    s.onUserMove('chat', cand('pet'))
    const result = s.commit(2000)
    expect(result.committedConstraint).not.toBeNull() // 仍写 store
    expect(s.state.kind).toBe('idle') // 不进 committing
  })

  it('committing 时 reactive ref 全清 (与 idle 同视觉)', () => {
    const fromRect = r(280, 0, 320, 320)
    const finalRect = r(320, 0, 320, 320)
    const s = new DragSession()
    s.arm('chat', new Map([['chat', fromRect]]), 1000)
    s.onUserMove('chat', cand('pet', { finalRect, targetEdge: 'left', distance: 10 }))
    // preview 时 ref 都已就位
    expect(previewAnchorId.value).toBe('pet')
    expect(previewEdge.value).toBe('left')
    expect(previewIntensity.value).toBeGreaterThan(0)
    expect(previewFinalRect.value).toEqual(finalRect)
    // commit 进 committing
    s.commit(2000, fromRect)
    // committing 视觉效果与 idle 同 — 因为窗本身在 tween，不需要再 ghost / glow
    expect(previewAnchorId.value).toBeNull()
    expect(previewEdge.value).toBeNull()
    expect(previewIntensity.value).toBe(0)
    expect(previewFinalRect.value).toBeNull()
  })

  it('endCommitting() committing → idle', () => {
    const fromRect = r(280, 0, 320, 320)
    const s = new DragSession()
    s.arm('chat', new Map([['chat', fromRect]]), 1000)
    s.onUserMove('chat', cand('pet'))
    s.commit(2000, fromRect)
    expect(s.state.kind).toBe('committing')
    s.endCommitting()
    expect(s.state.kind).toBe('idle')
  })

  it('endCommitting() 非 committing 状态 no-op', () => {
    const s = new DragSession()
    // idle → no-op
    s.endCommitting()
    expect(s.state.kind).toBe('idle')
    // armed → no-op
    s.arm('pet', new Map(), 1000)
    s.endCommitting()
    expect(s.state.kind).toBe('armed')
    // dragging → no-op
    s.onUserMove('pet', null)
    s.endCommitting()
    expect(s.state.kind).toBe('dragging')
    // preview → no-op
    s.onUserMove('pet', cand('chat'))
    s.endCommitting()
    expect(s.state.kind).toBe('preview')
  })

  it('committing cancel → 返 forestSnapshot + 回 idle (ESC 在 settle tween 中仍可中断)', () => {
    const fromRect = r(280, 0, 320, 320)
    const finalRect = r(320, 0, 320, 320)
    const snap = new Map([
      ['chat', fromRect],
      ['pet', r(640, 0, 320, 320)],
    ])
    const s = new DragSession()
    s.arm('chat', snap, 1000)
    s.onUserMove('chat', cand('pet', { finalRect }))
    s.commit(2000, fromRect)
    expect(s.state.kind).toBe('committing')
    // ESC：返回 forestSnapshot 让 caller 回滚（包括把 source 拉回 fromRect）
    const restored = s.cancel()
    expect(restored).not.toBeNull()
    expect(restored?.get('chat')).toEqual(fromRect)
    expect(restored?.get('pet')).toEqual(r(640, 0, 320, 320))
    expect(s.state.kind).toBe('idle')
  })

  it('committing reset → idle (与其他状态一致)', () => {
    const fromRect = r(280, 0, 320, 320)
    const s = new DragSession()
    s.arm('chat', new Map(), 1000)
    s.onUserMove('chat', cand('pet'))
    s.commit(2000, fromRect)
    expect(s.state.kind).toBe('committing')
    s.reset()
    expect(s.state.kind).toBe('idle')
  })

  it('committing 时 onUserMove 应忽略 (state.sourceId 检查或 idle 检查兜底)', () => {
    const fromRect = r(280, 0, 320, 320)
    const s = new DragSession()
    s.arm('chat', new Map(), 1000)
    s.onUserMove('chat', cand('pet'))
    s.commit(2000, fromRect)
    // committing 中再来一个 onUserMove —— 不应改 state（用户已松手，新 move 来自 settle tween 的内部
    // setPosition 应由 markInternal 过滤，但即使漏掉也不该破坏 committing）
    const before = s.state.kind
    s.onUserMove('chat', cand('settings'))
    expect(s.state.kind).toBe(before)
  })
})

// #30 follow-up D：primary-attract 模式 — primary 拖动反向吸引 secondary。
// dragSession 内部不区分 'source' vs 'primary-attract'（commit 用 candidate.movingId 推断），
// 但 commit 写入的 constraint.sourceId 必须是 candidate.movingId（即被吸的 secondary），
// 而不是 dragSession.sourceId（即被拖的 primary）。
describe('DragSession — primary-attract (#30 follow-up D)', () => {
  it('arm({mode:"primary-attract"}) 走 armed 流程（与 source 等价的状态机）', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), { mode: 'primary-attract' })
    expect(s.state.kind).toBe('armed')
    if (s.state.kind !== 'armed') throw new Error('narrow')
    expect(s.state.draggedId).toBe('pet')
  })

  it('primary 拖动 candidate.movingId=secondary → preview', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), { mode: 'primary-attract' })
    // primary-attract candidate：movingId 是 secondary（被反向吸），targetId 是 primary
    s.onUserMove('pet', cand('pet', { movingId: 'chat', targetId: 'pet' }))
    expect(s.state.kind).toBe('preview')
    expect(s.currentCandidate?.movingId).toBe('chat')
    expect(s.currentCandidate?.targetId).toBe('pet')
  })

  it('commit 写入 constraint.sourceId === candidate.movingId（不是 dragSession.sourceId）', () => {
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('pet', cand('pet', { movingId: 'chat', offset: 30 }))
    const result = s.commit(2000)
    expect(result.committedConstraint?.sourceId).toBe('chat') // 不是 'pet'
    expect(result.committedConstraint?.targetId).toBe('pet')
    expect(result.committedConstraint?.offset).toBe(30)
    expect(constraintStore.get('chat')?.targetId).toBe('pet')
    // dragSession 自己 sourceId='pet' 但 store 里没 'pet' constraint
    expect(constraintStore.get('pet')).toBeUndefined()
  })

  it('commit 替换原 secondary→someone 的 constraint 时 detach 用 movingId', () => {
    constraintStore.set({
      sourceId: 'chat',
      targetId: 'oldAnchor',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    const s = new DragSession()
    s.arm('pet', new Map(), 1000)
    s.onUserMove('pet', cand('pet', { movingId: 'chat' }))
    const result = s.commit(2000)
    expect(result.detached).toEqual({ sourceId: 'chat', targetId: 'oldAnchor' })
    expect(detachHistory.isRecent('chat', 'oldAnchor', 2000)).toBe(true)
    expect(constraintStore.get('chat')?.targetId).toBe('pet')
  })

  it('committing.sourceId === movingId（settle tween 移动 secondary，不是 primary）', () => {
    const secondaryRect = r(325, 0, 320, 320)
    const finalRect = r(320, 0, 320, 320)
    const s = new DragSession()
    s.arm('pet', new Map([['pet', r(0, 0, 320, 320)], ['chat', secondaryRect]]), 1000)
    s.onUserMove('pet', cand('pet', { movingId: 'chat', finalRect }))
    // caller 传 movingId 的 rect（secondaryRect），不是 primary 的
    const result = s.commit(2000, secondaryRect)
    expect(result.committedConstraint).not.toBeNull()
    expect(s.state.kind).toBe('committing')
    if (s.state.kind !== 'committing') throw new Error('narrow')
    expect(s.state.movingId).toBe('chat')
    expect(s.state.fromRect).toEqual(secondaryRect)
    expect(s.state.toRect).toEqual(finalRect)
  })

  it('ESC 在 primary-attract preview 时 cancel 返完整 forestSnapshot', () => {
    const primaryRect = r(0, 0, 320, 320)
    const secondaryRect = r(325, 0, 320, 320)
    const snap = new Map([
      ['pet', primaryRect],
      ['chat', secondaryRect],
    ])
    const s = new DragSession()
    s.arm('pet', snap, { mode: 'primary-attract' })
    s.onUserMove('pet', cand('pet', { movingId: 'chat' }))
    expect(s.state.kind).toBe('preview')
    const restored = s.cancel()
    expect(restored?.get('pet')).toEqual(primaryRect)
    expect(restored?.get('chat')).toEqual(secondaryRect)
    expect(s.state.kind).toBe('idle')
    // 没写 constraint
    expect(constraintStore.get('chat')).toBeUndefined()
  })
})
