// useContextKey：把 WorkspaceManager.contextKeys 桥到 Vue ref（#35 ADR-021 P1 Phase D）
//
// 用途：
// - 命令面板用 `useContextKey<boolean>('paletteVisible')` 拿到 reactive 可见性
// - 任何 panel SFC 都可订阅 contextKey 自动 reactive（如 `useContextKey('activePanel')` 高亮）
//
// 设计（ADR-021 D3）：
// - ContextKeyService 是纯 TS 发布订阅，本 composable 把 set 事件桥到 Vue ref
// - subscribe 在 setup 期注册，onUnmounted 自动 unsubscribe（不挂 onMounted/onBeforeUnmount 给
//   外部组件，让 composable 透明可组合）
// - 不引入 @vue/reactivity 子包，避免 +12KB gzip + effect scope 手动管理

import { onScopeDispose, shallowRef, type Ref } from 'vue'

import { useWorkspaceManager } from './useWorkspaceManager'

/**
 * 订阅一个 contextKey，返回 reactive ref。
 * @param key contextKey 名（如 'paletteVisible' / 'activePanel' / 'panel.ChatHub.visible'）
 * @returns ref，外部 watch / template 可直接消费；setup scope 销毁时自动 unsubscribe
 *
 * 实现细节：用 shallowRef 而非 ref 避免 UnwrapRef 把 T 折叠成 unknown；contextKey value 都是
 * 基本类型 / 简单对象，浅引用语义就够。
 */
export function useContextKey<T = unknown>(key: string): Ref<T | undefined> {
  const mgr = useWorkspaceManager()
  const value = shallowRef<T | undefined>(mgr.getContextKey(key) as T | undefined)
  const unsub = mgr.subscribeContextKeys(key, () => {
    value.value = mgr.getContextKey(key) as T | undefined
  })
  // onScopeDispose 同时覆盖 setup 内 / composable 嵌套调用 / effect scope；比 onBeforeUnmount 更准
  onScopeDispose(unsub)
  return value
}
