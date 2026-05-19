// Snap constraints 持久化（ADR-020 *Updated 2026-05-18*）。
//
// 存储：config 表 KV，key = 'snap:constraints'，value = JSON 数组（Array<PersistedConstraint>）。
//
// 启动期流程（useSnapWindow 触发）：
//   1. windowRegistry 注册所有 visible 窗口
//   2. loadPersistedConstraints() → 从 KV 读 → parse → filter anchor missing → 写入 constraintStore
//   3. solve(allRegisteredIds) → 把所有 attached 窗摆到正确位置
//
// 写入路径：
// - persistConstraints()：仅写 KV（测试 / 兼容路径）
// - persistAndBroadcastConstraints(senderId)：B3 修复——Rust 端原子 写 KV + emit 广播
//   生产路径应优先用此 helper，避免 setKv + emit 分别 IPC 时的顺序 race。
//
// 错误兜底：
// - getKv 失败：log + 返 {loaded:0, dropped:0}（启动不阻塞）
// - JSON parse 失败：log + clear KV + 返 {loaded:0, dropped:0}（脏数据自愈）
// - schema v 不匹配：drop 该条（破坏性 schema 变更时 ++ SCHEMA_V，老数据自动清）
// - anchor missing in registry：drop 该条 + warn（degrade 为 free 窗）

import { getConfig, setConfig, snapPersistAndBroadcast } from '@/services/config'
import { constraintStore } from './constraintStore'
import type { PersistedConstraint, SnapConstraint } from './types'
import { windowRegistry } from './windowRegistry'

export const SNAP_KV_KEY = 'snap:constraints'
export const SCHEMA_V = 1 as const

export interface PersistenceDeps {
  getKv(key: string): Promise<string | null>
  setKv(key: string, value: string): Promise<void>
}

const defaultDeps: PersistenceDeps = {
  getKv: getConfig,
  setKv: setConfig,
}

export interface LoadResult {
  loaded: number
  dropped: number
}

/** 从 KV 读 + filter + 写入 constraintStore。
 *  调用方应在 windowRegistry 注册全部 visible 窗后调用一次（启动期）。 */
export async function loadPersistedConstraints(
  deps: PersistenceDeps = defaultDeps,
): Promise<LoadResult> {
  let raw: string | null
  try {
    raw = await deps.getKv(SNAP_KV_KEY)
  } catch (e) {
    console.error('[snap/persistence] getKv failed:', e)
    return { loaded: 0, dropped: 0 }
  }
  if (raw === null || raw === '') return { loaded: 0, dropped: 0 }

  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch (e) {
    console.warn('[snap/persistence] JSON parse failed, clearing:', e)
    try {
      await deps.setKv(SNAP_KV_KEY, '[]')
    } catch (e2) {
      console.error('[snap/persistence] clear KV failed:', e2)
    }
    return { loaded: 0, dropped: 0 }
  }
  if (!Array.isArray(parsed)) {
    console.warn('[snap/persistence] KV not array, clearing')
    try {
      await deps.setKv(SNAP_KV_KEY, '[]')
    } catch {
      /* ignore */
    }
    return { loaded: 0, dropped: 0 }
  }

  let loaded = 0
  let dropped = 0
  for (const item of parsed) {
    if (!isValidPersistedConstraint(item)) {
      dropped++
      continue
    }
    if (item.v !== SCHEMA_V) {
      dropped++
      continue
    }
    // anchor missing in registry → downgrade 为 free
    if (!windowRegistry.get(item.targetId)) {
      dropped++
      continue
    }
    // 去掉 v 字段写入 store
    const con: SnapConstraint = {
      sourceId: item.sourceId,
      targetId: item.targetId,
      sourceEdge: item.sourceEdge,
      targetEdge: item.targetEdge,
      offset: item.offset,
      enabled: item.enabled,
      createdAt: item.createdAt,
    }
    const r = constraintStore.set(con)
    if (r.ok) {
      loaded++
    } else {
      dropped++
    }
  }
  return { loaded, dropped }
}

/** 把 constraintStore 当前全部 constraints 全量写入 KV。
 *  caller：dragSession.commit / delete constraint 后调一次。
 *
 *  B3 修复后生产路径建议改用 persistAndBroadcastConstraints(senderId) — 后者在 Rust 端
 *  把"写 KV + emit 广播"串行化，避免 emit 比写抵达其他 webview 更早。 */
export async function persistConstraints(deps: PersistenceDeps = defaultDeps): Promise<void> {
  const list = constraintStore.list()
  const persisted: PersistedConstraint[] = list.map((c) => ({ ...c, v: SCHEMA_V }))
  const json = JSON.stringify(persisted)
  try {
    await deps.setKv(SNAP_KV_KEY, json)
  } catch (e) {
    console.error('[snap/persistence] persistConstraints failed:', e)
  }
}

/** B3 修复：原子写 KV + 跨 webview emit 'snap:constraint-changed'。
 *  Rust 端串行（先写 KV，写完才 emit），保证其他 webview 收到事件 reload KV 时读到的是新值。
 *
 *  caller：useSnapWindow 内所有 commit / detach / cleanup 路径，替代两步走的
 *  `await persistConstraints(); await emit(CONSTRAINT_CHANGED_EVT, null)`。
 *  - senderId：调用方 webview 的 label，listener 端用此过滤自回声。
 *  - 失败仅 console.error，不抛（commit 流程不应因 IPC 失败回滚 store）。
 *
 *  注：本函数不接受 deps 注入；测试若需 mock 应直接调 persistConstraints。 */
export async function persistAndBroadcastConstraints(senderId: string): Promise<void> {
  const list = constraintStore.list()
  const persisted: PersistedConstraint[] = list.map((c) => ({ ...c, v: SCHEMA_V }))
  const json = JSON.stringify(persisted)
  try {
    await snapPersistAndBroadcast(json, senderId)
  } catch (e) {
    console.error('[snap/persistence] persistAndBroadcastConstraints failed:', e)
  }
}

/** 类型守卫：parse 出来的对象是否符合 PersistedConstraint 结构（防 JSON 注入异常字段） */
function isValidPersistedConstraint(x: unknown): x is PersistedConstraint {
  if (typeof x !== 'object' || x === null) return false
  const o = x as Record<string, unknown>
  return (
    typeof o.sourceId === 'string' &&
    typeof o.targetId === 'string' &&
    typeof o.sourceEdge === 'string' &&
    ['left', 'right', 'top', 'bottom'].includes(o.sourceEdge as string) &&
    typeof o.targetEdge === 'string' &&
    ['left', 'right', 'top', 'bottom'].includes(o.targetEdge as string) &&
    typeof o.offset === 'number' &&
    typeof o.enabled === 'boolean' &&
    typeof o.createdAt === 'number' &&
    typeof o.v === 'number'
  )
}
