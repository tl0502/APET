// internalMove guard（ADR-020 *Updated 2026-05-18*）。
//
// 用途：useSnapWindow 在 listen('tauri://move') handler 入口 isInternalMove(label) early-return，
// 防 solver 推 spoke → spoke setPosition → spoke onMoved → solver(spoke) 死循环。
//
// 释放策略：rAF 释放（确保 onMoved 已经被 OS 触发并被 guard 拦下）；jsdom 无 rAF 时 fallback setTimeout 50ms。
//
// 多窗口 set：用 Set<label>，每窗独立。

const internalGuard = new Set<string>()

/** caller 在调 Tauri setPosition 前调用：标记此窗为"程序内部移动"，
 *  rAF 后自动释放。 */
export function markInternal(label: string): void {
  internalGuard.add(label)
  scheduleRelease(label)
}

/** listen('tauri://move') handler 调此检查，true → skip handler */
export function isInternalMove(label: string): boolean {
  return internalGuard.has(label)
}

/** 测试 helper */
export function clearInternal(): void {
  internalGuard.clear()
}

function scheduleRelease(label: string): void {
  const release = (): void => {
    internalGuard.delete(label)
  }
  // jsdom 实现 rAF（vitest 用 jsdom env），但在 Node 环境 fallback setTimeout
  if (typeof requestAnimationFrame === 'function') {
    requestAnimationFrame(release)
  } else {
    setTimeout(release, 50)
  }
}
