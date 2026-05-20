// Workspace 域层公共类型（#35 ADR-021 P1）
//
// 设计要点：
// - PanelDescriptor 13 字段（ADR-021 schema 冻结）
// - PanelContext<T> 工具类型：spike #32 实操坑 3 落地 —— dockview-vue 6.x 给 panel 注入的
//   props 是 { params: { params: userParams, api, containerApi, tabLocation } } 嵌套
//   结构。用 PanelContext<T> 让 panel SFC 写 `defineProps<{ params: PanelContext<MyT> }>()`
//   而不是手敲嵌套两层 params。
// - Command / WhenExpr：命令面板 + when DSL 用
//
// 命名约定：PanelDescriptor.id 必须 PascalCase（同时是 Vue component 名；spike 坑 2 落地约束）
// PanelRegistry.register 内部 regex 校验。

import type { Component } from 'vue'

/**
 * 用户 panel SFC 通过 `defineProps<{ params: PanelContext<MyParams> }>()` 拿到结构化 props。
 * MyParams = 自定义业务 props（如 `{ greeting?: string; mode?: 'compact' | 'full' }`）。
 *
 * spike #32 坑 3：dockview-vue 6.x 内部 VueRenderer 把 panel 包装成 `{ params: { ... } }`。
 * 此处用 generic 抽象，让 panel SFC 不依赖 dockview 内部 wrapper 结构。
 */
export interface PanelContext<TParams = Record<string, unknown>> {
  /** addPanel 时透传的 user params；同一 panel 多实例时区分用 */
  params: TParams
  /** dockview panel API（setActive / setSize / dispose 等）；MVP 不直接消费 */
  api: unknown
  /** dockview container API（addPanel / addFloatingGroup 等）；MVP 不直接消费 */
  containerApi: unknown
  /** 'header' | 'floatingPanel' | ...；panel 当前在 dock 内还是浮出 */
  tabLocation: string
}

/**
 * 默认 panel 位置（dockview 内）。
 * - 'main'：主 dock 区中央（首个 panel 默认位置）
 * - 'main.right'：主 dock 区右侧 split
 * - 'bottom'：底部 panel（M3+ Bottom Panel 落地后启用）
 */
export type PanelLocation = 'main' | 'main.right' | 'bottom'

/**
 * mountStrategy 映射到 dockview renderer（spike #32 关键发现）：
 * - 'lazy' → dockview `renderer: 'onlyWhenVisible'`（默认；切换 panel 时 DOM mount/unmount，省内存）
 * - 'always' → dockview `renderer: 'always'`（DOM 持续存在，保表单 state；推荐 chat.hub / settings.theme）
 * - 'on-demand' → 首次访问后切 always（MVP 用 'lazy' 实现，M3+ 视需求扩展）
 */
export type PanelMountStrategy = 'lazy' | 'always' | 'on-demand'

/**
 * Panel 类别 — Activity Bar 分组用（ADR-021 限制顶部 ≤7 项，超出走命令面板）。
 * - 'chat'：对话相关
 * - 'task'：任务三件套
 * - 'creation'：装扮 / 人格 / 创作
 * - 'config'：设置 / 偏好
 * - 'debug'：调试 / 内省（only when when:'dev.mode' 时显示）
 * - 'play'：游戏化 / 小游戏（M5+）
 */
export type PanelCategory = 'chat' | 'task' | 'creation' | 'config' | 'debug' | 'play'

/**
 * Panel 描述符（ADR-021 13 字段 schema）。
 *
 * @example
 * ```ts
 * const desc: PanelDescriptor = {
 *   id: 'ChatHub',           // PascalCase 必需（spike 坑 2：同时是 Vue component 名）
 *   title: '对话',
 *   component: ChatHubPanel, // Vue Component 同步引用
 *   category: 'chat',
 *   singleton: true,
 *   mountStrategy: 'always',
 *   defaultLocation: 'main',
 *   when: 'persona.active',
 * }
 * ```
 */
export interface PanelDescriptor {
  // 核心 4
  /** 唯一 id，PascalCase（同时充当 Vue component 名）；如 'ChatHub' / 'SettingsTheme' */
  id: string
  /** 显示标题；函数版用于多实例（如 personas.workshop 显示 persona 名） */
  title: string | ((instance?: unknown) => string)
  /** Vue Component（同步引用；dockview-vue 6.x app.component(id, comp) 注册） */
  component: Component
  /** Activity Bar 分组 */
  category: PanelCategory

  // 行为 5
  /** 默认 true：同 id 复用一个实例 */
  singleton?: boolean
  /** 多实例必需：每个实例的 key 派生函数（如 `(p) => p.personaId`） */
  instanceKey?: (params: unknown) => string
  /** 默认 true：用户可手动关闭 panel */
  closable?: boolean
  /** 关闭前 hook：返回 false 取消关闭（如未保存提示） */
  beforeClose?: (params: unknown) => Promise<boolean>
  /** 详见 PanelMountStrategy；默认 'lazy' */
  mountStrategy?: PanelMountStrategy

  // 入口 3
  /** 默认 'main' */
  defaultLocation?: PanelLocation
  /** Activity Bar icon（EP Icon Vue Component） */
  icon?: Component
  /** VSCode-style when DSL；为空时永远显示 */
  when?: string

  // 命令面板 1
  /** 该 panel 暴露给 Ctrl+P 命令面板的命令（MVP 同期） */
  commands?: Command[]
}

/** 命令（Ctrl+P 命令面板 + ActivityBar 触发） */
export interface Command {
  /** 唯一 id，kebab-case；如 'workspace.togglePalette' / 'panel.reveal.ChatHub' */
  id: string
  /** 显示标题（fuzzy 匹配 + 列表显示） */
  title: string
  /** 可选可见性约束；为空永远可执行 */
  when?: string
  /** 执行体；可同步可异步 */
  handler: () => void | Promise<void>
}

/** when DSL AST 节点（whenDsl.ts 解析产物） */
export type WhenExpr =
  | { type: 'key'; name: string }
  | { type: 'not'; child: WhenExpr }
  | { type: 'and'; left: WhenExpr; right: WhenExpr }
  | { type: 'or'; left: WhenExpr; right: WhenExpr }

/** ContextKey 求值上下文（whenDsl.evalWhen 第二参数） */
export type ContextKeyMap = ReadonlyMap<string, unknown>

/**
 * Workspace 持久化接口（manager 内部 KV 桥接，单测可注 mock）。
 * 调用方 = WorkspaceManager；实现方 = KvWorkspacePersistence（生产）或测试 spy。
 */
export interface WorkspacePersistence {
  /** 读 KV `workspace:layout`（dockview 透明 JSON string）；不存在返 null */
  loadLayout(): Promise<string | null>
  saveLayout(layout: string): Promise<void>
  /** 读 KV `workspace:last_active_panel`；不存在返 null */
  loadLastActive(): Promise<string | null>
  saveLastActive(id: string): Promise<void>
}

/**
 * DockviewAdapter 接口（manager 与 dockview 解耦的桥）。
 * 实现方：DockviewAdapter（生产）或测试 spy（manager.test.ts mock）。
 *
 * spike #32 坑 1：实现方必须自带外层 ResizeObserver 喂 dockview api.layout（dockview-vue 6.x
 * 不内置）。spike 坑 2：mountPanel 内调 app.component(id, comp) 注册 Vue component。
 */
export interface WorkspaceAdapter {
  /** Panel mount：用 PanelDescriptor + params 触发 dockview addPanel */
  mountPanel(descriptor: PanelDescriptor, params?: unknown): void
  /** Panel unmount（dockview removePanel）；不存在 = no-op */
  unmountPanel(id: string): void
  /** 把 panel 切为 active（已 mount）；不存在 = no-op */
  revealPanel(id: string): void
  /** panel 是否已 mount */
  isPanelOpen(id: string): boolean
  /** 序列化当前 dockview layout 为 JSON string（不透明） */
  serialize(): string
  /** 从 JSON string 还原 dockview layout；失败抛 */
  deserialize(json: string): void
  /** 析构：disconnect ResizeObserver + dispose dockview */
  dispose(): void
}

/** Panel 注册去重失败 */
export class PanelAlreadyRegisteredError extends Error {
  constructor(id: string) {
    super(`Panel '${id}' is already registered`)
    this.name = 'PanelAlreadyRegisteredError'
  }
}

/** openPanel/revealPanel 时 panel 未注册 */
export class PanelNotRegisteredError extends Error {
  constructor(id: string) {
    super(`Panel '${id}' is not registered`)
    this.name = 'PanelNotRegisteredError'
  }
}

/** when DSL 解析失败 */
export class WhenParseError extends Error {
  constructor(message: string) {
    super(`when DSL parse error: ${message}`)
    this.name = 'WhenParseError'
  }
}

/** Command 注册去重失败 */
export class CommandAlreadyRegisteredError extends Error {
  constructor(id: string) {
    super(`Command '${id}' is already registered`)
    this.name = 'CommandAlreadyRegisteredError'
  }
}

/** Panel id 必须 PascalCase（同时是 Vue component 名；spike 坑 2 + ESLint 约束） */
export class InvalidPanelIdError extends Error {
  constructor(id: string) {
    super(`Panel id '${id}' must be PascalCase (start with uppercase, alphanumeric only)`)
    this.name = 'InvalidPanelIdError'
  }
}
