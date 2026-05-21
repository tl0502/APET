// DockviewAdapter — WorkspaceManager 与 dockview-vue 的唯一桥（#35 ADR-021 P1）
//
// 分层契约（ADR-021 强制）：
// - 本文件是项目里**唯一**被允许 import dockview API 的模块
// - panel SFC / pinia store / IPC handler 等业务代码禁止 import dockview-*
// - 桥两端：WorkspaceManager（纯 TS）↔ dockview-vue 6.x DockviewApi
//
// spike #32 实操坑落地：
// - 坑 1 ResizeObserver：本类不持有 RO（element 不在此处可见）；调用方
//   WorkspaceShell.vue 在 onReady 后建 RO 监听 dock host，触发 api.layout(w,h)
// - 坑 2 component registry：addPanel 时 component 字段 = Vue component 名
//   （MVP 用 descriptor.id 同时充当 Vue component 名；WorkspaceShell.vue main.ts
//   阶段 app.component(d.id, d.component) 全局注册）
// - 坑 3 props 嵌套：dockview-vue 自动包装为 { params: { params, api, containerApi,
//   tabLocation } }；panel SFC 通过 PanelContext<T> 拿；adapter 不参与
// - 坑 4 popout：不实现 popout 相关 API（MVP 不做；Tauri WebView2 结构性不可行）

import type { DockviewApi } from 'dockview-vue'

import type {
  PanelDescriptor,
  PanelLocation,
  PanelMountStrategy,
  WorkspaceAdapter,
  WorkspaceAdapterEvents,
} from './types'

/** dockview Event<T> 订阅返回的 disposable（subscribeEvents 内收集，dispose 时清理） */
interface DockviewDisposable {
  dispose(): void
}

export class DockviewAdapter implements WorkspaceAdapter {
  private readonly eventDisposables: DockviewDisposable[] = []

  constructor(private readonly api: DockviewApi) {}

  mountPanel(descriptor: PanelDescriptor, params?: unknown): void {
    if (this.api.getPanel(descriptor.id)) {
      // 幂等：dockview 端已存在 = no-op。可能来自 manager.openPanel 重复调用 / 也可能来自
      // deserialize 后 manager 主动 openPanel 但 fromJSON 已 mount 同 id（adapter 事件回灌让
      // manager 知道，再调 openPanel 时 manager 内部短路；本兜底防御调用方误判）。
      return
    }
    const title =
      typeof descriptor.title === 'function' ? descriptor.title(params) : descriptor.title
    this.api.addPanel({
      id: descriptor.id,
      component: descriptor.id, // 6.x: component 字段 = Vue 全局 component 名（与 descriptor.id 同源）
      title,
      params: params as Record<string, unknown> | undefined,
      renderer: mapMountStrategyToRenderer(descriptor.mountStrategy),
      ...mapLocationToPosition(descriptor.defaultLocation, this.api),
    })
  }

  unmountPanel(id: string): void {
    const panel = this.api.getPanel(id)
    if (panel) {
      this.api.removePanel(panel)
    }
  }

  revealPanel(id: string): void {
    const panel = this.api.getPanel(id)
    if (panel) {
      panel.api.setActive()
    }
  }

  isPanelOpen(id: string): boolean {
    return this.api.getPanel(id) !== undefined
  }

  serialize(): string {
    return JSON.stringify(this.api.toJSON())
  }

  deserialize(json: string): void {
    // 失败抛 → manager.deserialize catch + log + 走 default layout
    this.api.fromJSON(JSON.parse(json) as Parameters<DockviewApi['fromJSON']>[0])
  }

  /**
   * review P0 修复（F-2.1/2.2/2.3/1.1）：订阅 dockview 真实状态变化回灌 manager。
   *
   * dockview Event<T> 是 fn-like 接口 `(listener) => IDisposable`；subscribe 返
   * disposable 存 eventDisposables 数组，dispose() 时统一清理避免内存泄漏。
   *
   * onDidAddPanel 由 addPanel / fromJSON 内部同步 fire（见 dockviewComponent.js line 2353）；
   * onDidRemovePanel 同步 fire（用户点 tab ✕ / removePanel / clear 都触发）；
   * onDidActivePanelChange 用户点 tab 时同步 fire，参数 panel | undefined。
   */
  subscribeEvents(events: WorkspaceAdapterEvents): void {
    this.eventDisposables.push(
      this.api.onDidAddPanel((panel) => events.onPanelMounted(panel.id)),
      this.api.onDidRemovePanel((panel) => events.onPanelRemoved(panel.id)),
      this.api.onDidActivePanelChange((panel) => events.onActivePanelChanged(panel?.id ?? null)),
    )
  }

  dispose(): void {
    // dockview-vue 6.x: <DockviewVue> 组件 unmount 时自动 dispose dockview 实例
    // 本类持有的 api 引用会随 SFC unmount 失效；外层 ResizeObserver 由调用方
    // (WorkspaceShell.vue) 在 onBeforeUnmount 时 disconnect
    for (const d of this.eventDisposables) {
      try {
        d.dispose()
      } catch (e) {
        console.warn('[DockviewAdapter] event disposable dispose failed (non-fatal):', e)
      }
    }
    this.eventDisposables.length = 0
    this.api.clear()
  }
}

/** mountStrategy → dockview renderer 映射（spike #32 关键发现：dockview 内置 keep-alive） */
function mapMountStrategyToRenderer(
  strategy: PanelMountStrategy | undefined,
): 'always' | 'onlyWhenVisible' {
  if (strategy === 'always') return 'always'
  // 'lazy' + 'on-demand' + undefined → 默认省内存
  return 'onlyWhenVisible'
}

/** PanelLocation → addPanel position 选项 */
function mapLocationToPosition(
  location: PanelLocation | undefined,
  api: DockviewApi,
): { position?: { referencePanel: string; direction: 'right' } } {
  // MVP：'main.right' 时尝试用最后一个 panel 做参考做 right split；其他默认 dockview 自动布局
  if (location === 'main.right') {
    const panels = api.panels
    if (panels.length > 0) {
      const last = panels[panels.length - 1]!
      return { position: { referencePanel: last.id, direction: 'right' } }
    }
  }
  // 'main' / 'bottom' / undefined → 让 dockview 默认放置（MVP 阶段 'bottom' 同 'main'，M3+ Bottom Panel 启用时扩展）
  return {}
}
