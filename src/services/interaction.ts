// InteractionRouter 前端 IPC 绑定（#40，模块 N 主干）。
//
// ADR-025 lock：M2 hitbox = AABB 单 body；前端只发 hitbox='body'，
// hitbox 为参数留口给 M3+ Bone Proxy 4 hitbox 启用时无 IPC 改动。

import { invoke } from './ipc'

/** 5 类物理交互事件（前端状态机分类后调 dispatch）。 */
export type InteractionEventKind =
  | 'click'
  | 'dblclick'
  | 'longpress'
  | 'rclick'
  | 'drag'

/** M2 hitbox 只有 body（ADR-025 AABB 降级）；M3+ 起会扩 head/tail/edge。 */
export type InteractionHitbox = 'body'

/** Rust 端 ReactionEntry — `# 反应配置` 默认表 + .soul.md 覆盖合并结果。 */
export interface ReactionEntry {
  actionId: string
  voiceId: string | null
  moodDelta: string | null
  template: string | null
}

/** Rust emit 的事件名（监听端用）。 */
export const INTERACTION_REACTED_EVENT = 'pet:interaction_reacted'
export const PROTEST_TRIGGERED_EVENT = 'pet:protest_triggered'
export const PROTEST_REVERTED_EVENT = 'pet:protest_reverted'

/** pet:interaction_reacted payload — 与 Rust InteractionReactedPayload 同 schema。 */
export interface InteractionReactedPayload {
  event: InteractionEventKind
  hitbox: InteractionHitbox
  actionId: string
  voiceId: string | null
  moodChange: string | null
  template: string | null
}

/** pet:protest_triggered payload — 抗议触发，附 5s revertAfterMs 让前端 mood icon 自行收尾。 */
export interface ProtestPayload {
  window: string
  actionId: string
  moodChange: string | null
  template: string | null
  revertAfterMs: number
}

/** pet:protest_reverted payload — 5s 后由 Rust 自动 emit；前端可清 mood icon。 */
export interface ProtestRevertPayload {
  window: string
}

/** 派发一次物理交互。失败时返 null（不阻塞拖动），调用方按需降级。 */
export async function dispatchInteraction(
  event: InteractionEventKind,
  hitbox: InteractionHitbox = 'body',
): Promise<ReactionEntry | null> {
  try {
    return await invoke<ReactionEntry>('interaction_dispatch', { event, hitbox })
  } catch (e) {
    console.warn('[interaction] dispatch failed:', e)
    return null
  }
}

/** 记录一次（或 N 次）拖动起点。≥3 次 / 30s 时由 Rust 自动 emit pet:protest_triggered。 */
export async function recordDragCount(window: string, count = 1): Promise<number> {
  try {
    return await invoke<number>('interaction_record_drag_count', { window, count })
  } catch (e) {
    console.warn('[interaction] recordDragCount failed:', e)
    return 0
  }
}

/** 清空指定窗的滑窗 + 抗议状态。dev / 测试用。 */
export async function resetDragState(window: string): Promise<void> {
  try {
    await invoke<void>('interaction_reset_drag_state', { window })
  } catch (e) {
    console.warn('[interaction] resetDragState failed:', e)
  }
}
