// internalMove guard（ADR-020 *Updated 2026-05-18*）。
//
// 用途：useSnapWindow 在 listen('tauri://move') handler 入口 isInternalMove(label) early-return，
// 防 solver 推 spoke → spoke setPosition → spoke onMoved → solver(spoke) 死循环。
//
// 释放策略（B4 修复 2026-05-19）：
// - 原 rAF 释放：next frame (~16ms) 后解锁。但 Tauri move 事件经 IPC 回灌延迟 30-60ms，
//   rAF 可能先于 move 事件解锁 → guard 失效 → 真触发循环。
// - 现 setTimeout(80ms) 释放：覆盖单次 setPosition 的 IPC roundtrip。markInternal 在 tween
//   每帧（25ms）反复调用 → 反复 cancel + reschedule timer → tween 期间 guard 持续覆盖。
//
// 多窗口 set：用 Map<label, timer>，每窗独立 timer。

const internalTimers = new Map<string, ReturnType<typeof setTimeout>>()

/** 单次 markInternal 后保持 guard 的时长（ms）。
 *  - 80ms 足以覆盖 Tauri setPosition → OS move → move 事件回灌的 IPC roundtrip（典型 30-60ms）
 *  - tween 帧间 25ms，markInternal 反复 reset 不让 guard 间隙解锁
 *  - 不宜过长：onMoved guard 失效后用户的"手动微调"会被认成内部移动而忽略 */
const GUARD_TTL_MS = 80

/** caller 在调 Tauri setPosition 前调用：标记此窗为"程序内部移动"，
 *  GUARD_TTL_MS 后自动释放。每次调用 cancel 已有 timer 再重排，避免 guard 提前过期。 */
export function markInternal(label: string): void {
  const existing = internalTimers.get(label)
  if (existing !== undefined) clearTimeout(existing)
  const timer = setTimeout(() => {
    internalTimers.delete(label)
  }, GUARD_TTL_MS)
  internalTimers.set(label, timer)
}

/** listen('tauri://move') handler 调此检查，true → skip handler */
export function isInternalMove(label: string): boolean {
  return internalTimers.has(label)
}

/** 测试 helper */
export function clearInternal(): void {
  for (const t of internalTimers.values()) clearTimeout(t)
  internalTimers.clear()
}
