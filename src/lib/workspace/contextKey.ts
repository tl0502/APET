// ContextKey 反应式订阅服务（#35 ADR-021 P1）
//
// 用途：
// - manager 维护一组 key → value 状态（如 `panel.ChatHub.visible`, `persona.active`,
//   `paletteVisible`）
// - SFC 通过 useContextKey(keys) composable 订阅，UI reactive
// - command / panel.when 求值时用 ctx 查询
//
// 设计：手写发布订阅（不引 @vue/reactivity，避免子包边界问题；plan 决策 D3）。
// - set 触发所有"keys 命中"的 callback；same value 不触发（避免无效 reactive 更新）
// - subscribe 接收 string | string[]；返回 unsubscribe 函数

import type { ContextKeyMap } from './types'

type ContextKeyCallback = (changedKey: string) => void

export class ContextKeyService {
  private readonly values = new Map<string, unknown>()
  private readonly subscribers = new Map<string, Set<ContextKeyCallback>>()

  /** 设置 key 值；same value 不触发（用 Object.is 判定） */
  set(key: string, value: unknown): void {
    if (this.values.has(key) && Object.is(this.values.get(key), value)) {
      return
    }
    this.values.set(key, value)
    const subs = this.subscribers.get(key)
    if (subs) {
      for (const cb of subs) {
        cb(key)
      }
    }
  }

  /** 读 key 值；不存在返 undefined */
  get(key: string): unknown {
    return this.values.get(key)
  }

  /** 同 get；只读 Map 视图给 whenDsl.evalWhen 用 */
  asMap(): ContextKeyMap {
    return this.values
  }

  /**
   * 订阅一个或多个 key 的变化。任一 key 变化时 cb 被调用（参数 = 变化的 key 名）。
   * 返回 unsubscribe 函数；调用后清除订阅。
   */
  subscribe(keys: string | string[], cb: ContextKeyCallback): () => void {
    const keyList = Array.isArray(keys) ? keys : [keys]
    for (const k of keyList) {
      let set = this.subscribers.get(k)
      if (!set) {
        set = new Set()
        this.subscribers.set(k, set)
      }
      set.add(cb)
    }
    return () => {
      for (const k of keyList) {
        const set = this.subscribers.get(k)
        if (set) {
          set.delete(cb)
          if (set.size === 0) {
            this.subscribers.delete(k)
          }
        }
      }
    }
  }

  /** 清空所有 key + 订阅（测试 beforeEach 用） */
  clear(): void {
    this.values.clear()
    this.subscribers.clear()
  }
}
