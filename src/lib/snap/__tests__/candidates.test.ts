// S2 candidates 单测（ADR-020 *Updated*）。
//
// 覆盖：基础 trigger zone 进入/排除 / overlap 阈值 / corner dead / 多 candidate
// 排序 / memoryBias (existing / recent detach / neither) / score 数值。

import { beforeEach, describe, expect, it } from 'vitest'
import { DetachHistory, findCandidates, findReverseAttract } from '../candidates'
import { constraintStore } from '../constraintStore'
import { DETACH_BIAS, EXISTING_BIAS, W_MEMORY, W_VELOCITY } from '../geometry'
import type { Rect, WindowRegistration } from '../types'

const r = (x: number, y: number, w: number, h: number): Rect => ({ x, y, w, h })
const reg = (id: string, rect: Rect, visible = true): WindowRegistration => ({ id, rect, visible })

beforeEach(() => {
  constraintStore.clear()
})

describe('findCandidates — 基础 trigger 进入 / 排除', () => {
  it('source.right 距 target.left = 5px (< TRIGGER_ZONE 10) + 完全重叠 → 1 candidate', () => {
    const source = r(0, 0, 320, 320) // right = 320
    const target = reg('target', r(325, 0, 320, 320)) // left = 325; dist = 5
    const cands = findCandidates('source', source, [target])
    expect(cands).toHaveLength(1)
    expect(cands[0]?.targetId).toBe('target')
    expect(cands[0]?.sourceEdge).toBe('right')
    expect(cands[0]?.targetEdge).toBe('left')
    expect(cands[0]?.offset).toBe(0) // y 完全对齐
    // T8 (#31 follow-up B)：distance 字段透传供 UI 算渐进 intensity
    expect(cands[0]?.distance).toBe(5)
  })

  it('distance > TRIGGER_ZONE → 0 candidate', () => {
    // #30 follow-up D revision：TRIGGER_ZONE 10→25 后，dist=30 超阈值
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(350, 0, 320, 320)) // dist = 30 > 25
    expect(findCandidates('s', source, [target])).toHaveLength(0)
  })

  it('!visible → 跳过', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(325, 0, 320, 320), false)
    expect(findCandidates('s', source, [target])).toHaveLength(0)
  })

  it('self（target.id === sourceId）→ 跳过', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('s', r(325, 0, 320, 320)) // 同 id
    expect(findCandidates('s', source, [target])).toHaveLength(0)
  })
})

describe('findCandidates — overlap 阈值', () => {
  it('overlap < threshold (320 * 0.25 = 80 < MIN_OVERLAP 72?)', () => {
    // edge length 320 → threshold = max(72, 80) = 80
    // source.right ↔ target.left；source y=[0,320]，target y=[260,580]，overlap = 60 < 80 → reject
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(325, 260, 320, 320))
    expect(findCandidates('s', source, [target])).toHaveLength(0)
  })

  it('overlap 恰好等于阈值 → accept', () => {
    // length 320 → threshold = 80；让 overlap = 80
    // source y=[0,320]，target y=[240,560] → overlap = [240,320] = 80
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(325, 240, 320, 320))
    expect(findCandidates('s', source, [target])).toHaveLength(1)
  })

  it('短边走 MIN_OVERLAP 兜底：length 100 → threshold 72', () => {
    // source 100x100, target 100x100；overlap 必须 ≥ 72
    const source = r(0, 0, 100, 100)
    const target = reg('t', r(105, 30, 100, 100)) // overlap y=[30,100]=70 < 72 → reject
    expect(findCandidates('s', source, [target])).toHaveLength(0)

    const target2 = reg('t', r(105, 28, 100, 100)) // overlap y=[28,100]=72 == threshold → accept
    expect(findCandidates('s', source, [target2])).toHaveLength(1)
  })
})

describe('findCandidates — corner dead zone', () => {
  it('source 中心紧贴 target corner → 全部 4 个 edge pair 都被 inCornerDead 阻塞', () => {
    // source center 在 (400, 400) target's top-left corner
    const source = r(395, 395, 10, 10)
    const target = reg('t', r(400, 400, 320, 320))
    expect(findCandidates('s', source, [target])).toHaveLength(0)
  })
})

describe('findCandidates — 多 candidate 排序（score 升序）', () => {
  it('两个 target，距离不同 → 距离近的 score 低排前', () => {
    const source = r(0, 0, 320, 320)
    const near = reg('near', r(325, 0, 320, 320)) // dist 5
    const far = reg('far', r(0, 325, 320, 320)) // bottom ↔ top dist 5
    const cands = findCandidates('s', source, [far, near])
    // 两个 dist 都是 5 + 完全重叠 → score 相同；只要稳定 sort 都接受
    expect(cands).toHaveLength(2)
    expect(cands[0]!.score).toBeLessThanOrEqual(cands[1]!.score)
  })

  it('两 target 距离不同 → 近的排前', () => {
    const source = r(0, 0, 320, 320)
    const closer = reg('A', r(323, 0, 320, 320)) // dist 3
    const farther = reg('B', r(327, 0, 320, 320)) // dist 7（同方向）
    // 两者 dist 都 ≤ ATTACH=10 + 同 sourceEdge，closer 应排前
    const cands = findCandidates('s', source, [farther, closer])
    expect(cands).toHaveLength(2)
    expect(cands[0]?.targetId).toBe('A')
    expect(cands[1]?.targetId).toBe('B')
    expect(cands[0]!.score).toBeLessThan(cands[1]!.score)
  })
})

describe('findCandidates — memoryBias', () => {
  it('既无 existing 也无 recent detach → memoryBias = 0, memoryTerm = 1', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(320, 0, 320, 320)) // dist 0, overlap full
    const cands = findCandidates('s', source, [target])
    expect(cands).toHaveLength(1)
    // score = 0×W_DISTANCE + 0×W_OVERLAP + 1×W_MEMORY + 1×W_VELOCITY = W_MEMORY + W_VELOCITY
    // (无 velocity 参数时 vTerm=1 中性)
    expect(cands[0]?.score).toBeCloseTo(W_MEMORY + W_VELOCITY, 9)
  })

  it('existing attachment（constraintStore 有 s→t）→ memoryBias = +0.5', () => {
    constraintStore.set({
      sourceId: 's',
      targetId: 't',
      sourceEdge: 'right',
      targetEdge: 'left',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(320, 0, 320, 320))
    const cands = findCandidates('s', source, [target])
    // score = 0 + 0 + (1 - 0.5) * W_MEMORY + 1 * W_VELOCITY = 0.5*W_MEMORY + W_VELOCITY
    expect(cands[0]?.score).toBeCloseTo((1 - EXISTING_BIAS) * W_MEMORY + W_VELOCITY, 9)
  })

  it('recent detach（30s 内）→ memoryBias = -0.5（更高 score 即更难选中）', () => {
    const history = new DetachHistory()
    history.recordDetach('s', 't', 1000)
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(320, 0, 320, 320))
    const cands = findCandidates('s', source, [target], {
      now: 1000 + 5000, // 5s 后
      detachHistoryInstance: history,
    })
    expect(cands[0]?.score).toBeCloseTo((1 - DETACH_BIAS) * W_MEMORY + W_VELOCITY, 9)
  })

  it('detach 超过 30s 不再算 recent', () => {
    const history = new DetachHistory()
    history.recordDetach('s', 't', 0)
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(320, 0, 320, 320))
    const cands = findCandidates('s', source, [target], {
      now: 31_000,
      detachHistoryInstance: history,
    })
    // 回到 neither 状态 → memoryBias = 0
    expect(cands[0]?.score).toBeCloseTo(W_MEMORY + W_VELOCITY, 9)
  })
})

describe('findCandidates — score 量级合理', () => {
  it('完美吸附（dist=0, overlap full, existing）的 score < 最差合格 candidate', () => {
    constraintStore.set({
      sourceId: 's',
      targetId: 'best',
      sourceEdge: 'right',
      targetEdge: 'left',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    const source = r(0, 0, 320, 320)
    const best = reg('best', r(320, 0, 320, 320)) // perfect snap
    // worst 想做"刚好过线"的候选：overlap=80==threshold, dist=10（trigger zone 边界）
    const worstAccepted = reg('worst', r(330, 240, 320, 320))
    const cands = findCandidates('s', source, [best, worstAccepted])
    expect(cands).toHaveLength(2)
    expect(cands[0]?.targetId).toBe('best')
    expect(cands[0]!.score).toBeLessThan(cands[1]!.score)
  })
})

// #31 follow-up C：hysteresis — docked source 用 DETACH_ZONE (45) 放宽；
// 200ms time lockout 内即使超过 DETACH_ZONE 也不脱钩。
// #30 follow-up D revision：ATTACH_ZONE=25, DETACH_ZONE=45。
describe('findCandidates — hysteresis (Phase D #31 follow-up C)', () => {
  it('未 docked → ATTACH_ZONE 25 阈值（30px 距离应拒）', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(350, 0, 320, 320)) // dist = 30 > 25
    const cands = findCandidates('s', source, [target])
    expect(cands).toHaveLength(0)
  })

  it('显式 dockedTargetId → DETACH_ZONE 45 放宽（30px 仍 accept）', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(350, 0, 320, 320)) // dist = 30 (> ATTACH 25, ≤ DETACH 45)
    const cands = findCandidates('s', source, [target], { dockedTargetId: 't' })
    expect(cands).toHaveLength(1)
    expect(cands[0]?.distance).toBe(30)
  })

  it('docked source 拖远 > DETACH_ZONE → 仍脱钩（50px 拒）', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(370, 0, 320, 320)) // dist = 50 > 45
    const cands = findCandidates('s', source, [target], { dockedTargetId: 't' })
    expect(cands).toHaveLength(0)
  })

  it('time lockout 内（dockedAt now < 200ms）→ 任何距离都保持 docked', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(1000, 0, 320, 320)) // dist = 680，超 DETACH 数十倍
    const cands = findCandidates('s', source, [target], {
      dockedTargetId: 't',
      dockedAt: 5000,
      now: 5100, // 100ms ago，在 200ms lockout 内
    })
    expect(cands.length).toBeGreaterThanOrEqual(1)
    expect(cands.some((c) => c.targetId === 't')).toBe(true)
  })

  it('time lockout 过期（now - dockedAt > 200ms）→ 回到 DETACH_ZONE 行为', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(370, 0, 320, 320)) // dist = 50 > 45
    const cands = findCandidates('s', source, [target], {
      dockedTargetId: 't',
      dockedAt: 5000,
      now: 5300, // 300ms ago，已过 lockout
    })
    expect(cands).toHaveLength(0)
  })

  it('docked 到 a → 仅对 a 放宽，对其他 target 仍 ATTACH', () => {
    const source = r(0, 0, 320, 320)
    const a = reg('a', r(350, 0, 320, 320)) // dist = 30（docked target，DETACH 45 内）
    const b = reg('b', r(0, 350, 320, 320)) // dist = 30（其他 target，ATTACH 25 外）
    const cands = findCandidates('s', source, [a, b], { dockedTargetId: 'a' })
    const ids = cands.map((c) => c.targetId)
    expect(ids).toContain('a') // a 走 DETACH 45，30 < 45 → accept
    expect(ids).not.toContain('b') // b 走 ATTACH 25，30 > 25 → reject
  })

  it('dockedTargetId 缺省时从 constraintStore.get 反推（向后兼容）', () => {
    constraintStore.set({
      sourceId: 's',
      targetId: 't',
      sourceEdge: 'right',
      targetEdge: 'left',
      offset: 0,
      enabled: true,
      createdAt: 0, // 远古，不触发 time lockout
    })
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(350, 0, 320, 320)) // dist = 30
    const cands = findCandidates('s', source, [target])
    expect(cands).toHaveLength(1) // 自动走 DETACH_ZONE 45
  })
})

// #31 follow-up C Phase C：velocity 同向偏置
describe('findCandidates — velocity bias (Phase C #31 follow-up C)', () => {
  it('无 velocity 参数 → vTerm=1 中性（与之前等价）', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(320, 0, 320, 320))
    const cands = findCandidates('s', source, [target])
    expect(cands[0]?.score).toBeCloseTo(W_MEMORY + W_VELOCITY, 9)
  })

  it('静止 velocity ({0,0}) → vTerm=1 中性（与无 velocity 等价）', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(320, 0, 320, 320))
    const cands = findCandidates('s', source, [target], {
      velocity: { x: 0, y: 0 },
    })
    expect(cands[0]?.score).toBeCloseTo(W_MEMORY + W_VELOCITY, 9)
  })

  it('同向 velocity（source 朝右拖向 target.left）→ score 减 W_VELOCITY', () => {
    // source 在 target 左侧，sourceEdge='right' / targetEdge='left'，source 朝 +x 移动
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(320, 0, 320, 320))
    const cands = findCandidates('s', source, [target], {
      velocity: { x: 50, y: 0 }, // 完全同向，vBias=1, vTerm=0
    })
    // score = 0 + 0 + W_MEMORY + 0 * W_VELOCITY = W_MEMORY
    expect(cands[0]?.score).toBeCloseTo(W_MEMORY, 9)
  })

  it('反向 velocity（source 朝左远离）→ vTerm=1 中性（不惩罚）', () => {
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(320, 0, 320, 320))
    const cands = findCandidates('s', source, [target], {
      velocity: { x: -50, y: 0 }, // 反向，vBias=0（截断）, vTerm=1
    })
    expect(cands[0]?.score).toBeCloseTo(W_MEMORY + W_VELOCITY, 9)
  })

  it('多 edge pair：source 完全嵌入 target，同向 velocity 偏向自己方向', () => {
    // source 在 target 内部，4 个 edge pair 距离均为 0 全 overlap full，无 velocity 时 score 相同
    // 加 velocity → 同向 edge 减分被选中
    const source = r(0, 0, 320, 320)
    // target 右侧紧贴，dist=0
    const targetRight = reg('right', r(320, 0, 320, 320)) // sourceEdge='right'
    // target 下侧紧贴，dist=0
    const targetBottom = reg('bottom', r(0, 320, 320, 320)) // sourceEdge='bottom'

    // 朝 +x 拖（velocity right）→ source.right ↔ targetRight.left 应排前
    const candsRight = findCandidates('s', source, [targetRight, targetBottom], {
      velocity: { x: 50, y: 0 },
    })
    expect(candsRight[0]?.targetId).toBe('right')

    // 朝 +y 拖 → targetBottom 应排前
    const candsBottom = findCandidates('s', source, [targetRight, targetBottom], {
      velocity: { x: 0, y: 50 },
    })
    expect(candsBottom[0]?.targetId).toBe('bottom')
  })

  it('velocity 不影响 trigger zone 过滤（仅影响 score）', () => {
    // dist = 80 > ATTACH_ZONE 60；即使同向 velocity 也应被过滤
    const source = r(0, 0, 320, 320)
    const target = reg('t', r(400, 0, 320, 320)) // dist 80
    const cands = findCandidates('s', source, [target], {
      velocity: { x: 100, y: 0 }, // 强同向也无用
    })
    expect(cands).toHaveLength(0)
  })

  it('velocity 让两个等距 candidate 中同向的胜出', () => {
    // 两个等距 candidate（dist=5），无 velocity 时 sort 不稳定；
    // 加同向 velocity → 命中的那个 score 减 W_VELOCITY
    const source = r(0, 0, 320, 320)
    const right = reg('right', r(325, 0, 320, 320)) // source.right→target.left dist 5
    const bottom = reg('bottom', r(0, 325, 320, 320)) // source.bottom→target.top dist 5
    const cands = findCandidates('s', source, [right, bottom], {
      velocity: { x: 50, y: 0 }, // 朝右
    })
    expect(cands[0]?.targetId).toBe('right')
    expect(cands[0]!.score).toBeLessThan(cands[1]!.score)
  })
})

// #30 follow-up F：边段占用 + 错位磁吸。
//
// 场景：A 是大 anchor (300×500)；B 已吸到 A.right 上半段 [0, 250)。
// C 拖近 A.right：
//   - C 想吸到 A.right [0, 250)（与 B 重叠）→ 占用拒绝；尝试滑入下半 [250, 500)
//   - 若 C 投影长度 > 250（剩余空段）→ 拒绝整个 edge pair
describe('findCandidates — 边段占用 + 错位磁吸 (#30 follow-up F)', () => {
  it('A.right 完全空 → C 按原 offset 吸附', () => {
    const A = reg('A', r(0, 0, 300, 500)) // A.right 长 500
    const C = r(305, 100, 100, 100) // C.left 距 A.right 5；投影 [100, 200]
    const cands = findCandidates('C', C, [A])
    expect(cands).toHaveLength(1)
    expect(cands[0]?.offset).toBe(100) // 不滑动，原 offset
    expect(cands[0]?.targetId).toBe('A')
    expect(cands[0]?.targetEdge).toBe('right')
  })

  it('A.right 上半被 B 占 [0, 250]，C 投影 [100, 200] 完全在 occupied → 自动滑到下半 free [250, 500]', () => {
    const A = reg('A', r(0, 0, 300, 500))
    const B = reg('B', r(300, 0, 100, 250)) // B.left=300 紧贴 A.right；投影 [0, 250]
    constraintStore.set({
      sourceId: 'B',
      targetId: 'A',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    // C 想吸 A.right 上半，投影 [100, 200] 完全在 B 占用内
    const C = r(305, 100, 100, 100)
    const cands = findCandidates('C', C, [A, B])
    expect(cands).toHaveLength(1)
    // findFreePlacement 策略 3：projCenter 150 在 occupied，最近 free 段 [250, 500]
    // projCenter(150) ≤ 段中心(375) → 贴段 start = 250；offset = 250
    expect(cands[0]?.offset).toBe(250)
    expect(cands[0]?.targetId).toBe('A')
  })

  it('占用太满，剩余 free 段 < source 投影长度 → 拒绝整个 edge pair', () => {
    const A = reg('A', r(0, 0, 300, 200)) // A.right 长 200
    const B = reg('B', r(300, 0, 100, 180)) // B 占 [0, 180]，剩 free [180, 200] 仅 20px
    constraintStore.set({
      sourceId: 'B',
      targetId: 'A',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    // C 投影长 100，剩余空段 20 装不下 → 拒
    const C = r(305, 50, 100, 100)
    const cands = findCandidates('C', C, [A, B])
    expect(cands).toHaveLength(0)
  })

  it('source 自己已吸到该边 → 评估时排除自己（refresh 路径，否则一定冲突）', () => {
    const A = reg('A', r(0, 0, 300, 500))
    const B = reg('B', r(300, 100, 100, 100)) // B 已吸到 A.right offset=100
    constraintStore.set({
      sourceId: 'B',
      targetId: 'A',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 100,
      enabled: true,
      createdAt: 0,
    })
    // B 自己重新评估（dockedTargetId='A'）→ 应排除自己的旧占用，否则 100% conflict
    const cands = findCandidates('B', B.rect, [A, B], { dockedTargetId: 'A' })
    expect(cands.length).toBeGreaterThan(0)
    expect(cands[0]?.offset).toBe(100) // 不滑动
  })

  it('部分越界：C 投影部分在 occupied → 推到 free 段贴边', () => {
    const A = reg('A', r(0, 0, 300, 500))
    const B = reg('B', r(300, 0, 100, 200)) // B 占 [0, 200]
    constraintStore.set({
      sourceId: 'B',
      targetId: 'A',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    // C 投影 [150, 250]，左侧落在 occupied，右侧落在 free
    // 中心 200 在 free [200, 500] 的 start 上 → 策略 2 推到段 start = 200
    const C = r(305, 150, 100, 100)
    const cands = findCandidates('C', C, [A, B])
    expect(cands).toHaveLength(1)
    expect(cands[0]?.offset).toBe(200)
  })

  it('两 candidate 都合格但占用情况不同 → 评分独立（占用不影响 score 只影响 offset/reject）', () => {
    // A 空 / D 也空，两窗等距等大 → C 应被两窗都接受
    const A = reg('A', r(0, 0, 300, 500))
    const D = reg('D', r(0, 800, 300, 500))
    const C = r(305, 200, 100, 100)
    const cands = findCandidates('C', C, [A, D])
    // C 在 A 旁；C.right ↔ D.left 距远（不命中 D），只剩 C↔A 一个 candidate
    expect(cands).toHaveLength(1)
    expect(cands[0]?.targetId).toBe('A')
  })
})

describe('DetachHistory', () => {
  it('recordDetach + isRecent 路径', () => {
    const h = new DetachHistory()
    h.recordDetach('a', 'b', 1000)
    expect(h.isRecent('a', 'b', 1500)).toBe(true)
    expect(h.isRecent('a', 'b', 31_001)).toBe(false)
    expect(h.isRecent('a', 'c', 1500)).toBe(false) // 不同 target
  })

  it('clear 清空全部', () => {
    const h = new DetachHistory()
    h.recordDetach('a', 'b')
    h.recordDetach('c', 'd')
    expect(h.size()).toBe(2)
    h.clear()
    expect(h.size()).toBe(0)
  })
})

// #30 follow-up D：findReverseAttract — primary 拖动反向吸引附近 secondary。
// 输入：primary 自身 id + rect + registry（含所有 secondary）；
// 输出：合并 sorted candidates，每个 candidate.movingId === 某 secondary，targetId === primary。
describe('findReverseAttract (#30 follow-up D)', () => {
  it('单 secondary 在 trigger zone → 1 candidate，movingId=secondary, targetId=primary', () => {
    const primaryRect = r(0, 0, 320, 320)
    const secondary = reg('chat', r(325, 0, 320, 320)) // dist 5 < ATTACH 10
    const cands = findReverseAttract('pet', primaryRect, [secondary])
    expect(cands).toHaveLength(1)
    expect(cands[0]?.movingId).toBe('chat')
    expect(cands[0]?.targetId).toBe('pet')
    expect(cands[0]?.distance).toBe(5)
  })

  it('多 secondary 距离不同 → 距离近的 score 低排前', () => {
    const primaryRect = r(0, 0, 320, 320)
    const closer = reg('a', r(323, 0, 320, 320)) // dist 3
    const farther = reg('b', r(327, 0, 320, 320)) // dist 7
    const cands = findReverseAttract('pet', primaryRect, [closer, farther])
    expect(cands).toHaveLength(2)
    expect(cands[0]?.movingId).toBe('a')
    expect(cands[0]!.score).toBeLessThan(cands[1]!.score)
  })

  it('primary 自身（id === primaryId）→ 跳过', () => {
    const primaryRect = r(0, 0, 320, 320)
    // 即使把 primary 也塞进 registry，也不应被当 moving
    const self = reg('pet', r(0, 0, 320, 320))
    const cands = findReverseAttract('pet', primaryRect, [self])
    expect(cands).toHaveLength(0)
  })

  it('!visible secondary → 跳过', () => {
    const primaryRect = r(0, 0, 320, 320)
    const hidden = reg('chat', r(325, 0, 320, 320), false)
    const cands = findReverseAttract('pet', primaryRect, [hidden])
    expect(cands).toHaveLength(0)
  })

  it('secondary 太远 > ATTACH_ZONE → 不进 candidate', () => {
    const primaryRect = r(0, 0, 320, 320)
    const far = reg('chat', r(400, 0, 320, 320)) // dist 80 > ATTACH 10
    const cands = findReverseAttract('pet', primaryRect, [far])
    expect(cands).toHaveLength(0)
  })

  it('registry 仅含 primary（无 secondary）→ 空 list', () => {
    const primaryRect = r(0, 0, 320, 320)
    const cands = findReverseAttract('pet', primaryRect, [
      reg('pet', primaryRect),
    ])
    expect(cands).toHaveLength(0)
  })

  it('已 attached 的 secondary 也参与（EXISTING memoryBias 让它 score 更低）', () => {
    constraintStore.set({
      sourceId: 'chat',
      targetId: 'pet',
      sourceEdge: 'right',
      targetEdge: 'left',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    const primaryRect = r(320, 0, 320, 320) // primary 右移
    // attached secondary 离 primary 较远（dist 7）
    const attached = reg('chat', r(0, 0, 320, 320)) // source.right=320, target.left=320 dist=0
    const cands = findReverseAttract('pet', primaryRect, [attached])
    expect(cands.length).toBeGreaterThanOrEqual(1)
    // movingId 仍是 secondary（不是 primary）
    expect(cands[0]?.movingId).toBe('chat')
  })

  // P3 修复 (review 2)：findReverseAttract 必须把全 registry 传给 findCandidates，
  // 让 computeEdgeOccupancy 能 lookup 其他 secondary 的 source rect。原 bug：传
  // mini-registry [primaryReg] → 其他 secondary 不在 registry → occupancy 永远空 →
  // 新反向吸引的 secondary 可能与已占用边段完全重叠。
  it('P3 regression: 已占用边段不能被反向吸引到重叠位置', () => {
    // chat 已吸到 pet.right top 部分（投影 y=0..200，占 pet.right 边段 [0, 200]）
    constraintStore.set({
      sourceId: 'chat',
      targetId: 'pet',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    const pet = r(0, 0, 320, 400)
    const chat = reg('chat', r(320, 0, 320, 200)) // 吸在 pet.right [0, 200]
    // pomodoro 想吸 pet.right，但放在与 chat 完全重叠的 y 位置（投影也是 0..200）
    const pomo = reg('pomodoro', r(325, 0, 320, 200)) // 视觉与 chat 重叠
    const cands = findReverseAttract('pet', pet, [chat, pomo])
    // 若 P3 未修：occupancy 空 → cand 直接给 offset=0 → pomodoro 也吸到 [0, 200] 重叠 chat
    // 修复后：findFreePlacement 检测 [0, 200] 被占，把 pomodoro 滑到 free 段 [200, 400]
    // 找到 cand：起点 y=200（offset=200 - chat 起点 0 = 200）
    const pomoCand = cands.find((c) => c.movingId === 'pomodoro')
    if (pomoCand) {
      // pomodoro 最终 y 必须 ≥ 200（不与 chat 占用段重叠）
      expect(pomoCand.finalRect.y).toBeGreaterThanOrEqual(200)
    }
    // 也可能没 cand（free 段不够长 → findFreePlacement 返 null）— 但绝不能给重叠位置
  })

  it('P3 regression: free 段不够时 pomodoro 不进 candidate（不重叠）', () => {
    // chat 占满整条 pet.right 边
    constraintStore.set({
      sourceId: 'chat',
      targetId: 'pet',
      sourceEdge: 'left',
      targetEdge: 'right',
      offset: 0,
      enabled: true,
      createdAt: 0,
    })
    const pet = r(0, 0, 320, 320)
    const chat = reg('chat', r(320, 0, 320, 320)) // 占满 pet.right
    const pomo = reg('pomodoro', r(325, 0, 320, 320)) // 想吸 pet.right
    const cands = findReverseAttract('pet', pet, [chat, pomo])
    const pomoCand = cands.find((c) => c.movingId === 'pomodoro')
    // 整条 pet.right 已被占满 → 没有 free 段容下 pomodoro → 不该有 cand
    expect(pomoCand).toBeUndefined()
  })
})
