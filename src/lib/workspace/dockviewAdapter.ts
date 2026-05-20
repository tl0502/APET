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
} from './types'

export class DockviewAdapter implements WorkspaceAdapter {
  constructor(private readonly api: DockviewApi) {}

  mountPanel(descriptor: PanelDescriptor, params?: unknown): void {
    if (this.api.getPanel(descriptor.id)) {
      // 幂等：已存在 = no-op（manager.openPanel 已确保此分支不会触发，但桥层多一层兜底）
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

  dispose(): void {
    // dockview-vue 6.x: <DockviewVue> 组件 unmount 时自动 dispose dockview 实例
    // 本类持有的 api 引用会随 SFC unmount 失效；外层 ResizeObserver 由调用方
    // (WorkspaceShell.vue) 在 onBeforeUnmount 时 disconnect
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
