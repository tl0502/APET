// 窗口注册表：id → WindowRegistration。
//
// 设计 note（Tauri multi-window 约束）：
// - Tauri 每个 webview 是独立 JS context，本 module 是 webview-local 单例
// - cross-webview 同步靠 emit('snap:registry-update', ...) 广播（在 useSnapWindow 内实现）
// - 本 module 只管 in-memory state；持久化由 persistence.ts 处理 constraint，
//   windowRegistry 不持久化（每次启动各 webview onMounted 重新注册自己）

import type { Rect, WindowRegistration } from './types'

class WindowRegistry {
  private _byId = new Map<string, WindowRegistration>()

  /** 注册或全量替换一个窗口的状态。 */
  upsert(reg: WindowRegistration): void {
    this._byId.set(reg.id, { ...reg })
  }

  /** 只更新 rect（onMoved / onResized 路径用，比 upsert 省一次对象创建）。 */
  updateRect(id: string, rect: Rect): boolean {
    const cur = this._byId.get(id)
    if (!cur) return false
    cur.rect = rect
    return true
  }

  /** 只更新 visible。 */
  updateVisible(id: string, visible: boolean): boolean {
    const cur = this._byId.get(id)
    if (!cur) return false
    cur.visible = visible
    return true
  }

  get(id: string): WindowRegistration | undefined {
    return this._byId.get(id)
  }

  delete(id: string): boolean {
    return this._byId.delete(id)
  }

  list(): WindowRegistration[] {
    return Array.from(this._byId.values())
  }

  clear(): void {
    this._byId.clear()
  }
}

export const windowRegistry = new WindowRegistry()
export { WindowRegistry }
