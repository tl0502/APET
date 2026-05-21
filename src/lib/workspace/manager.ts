// WorkspaceManager — Workspace 域层核心（#35 ADR-021 P1）
//
// 职责：
// - PanelRegistry / ContextKeyService / WorkspacePersistence / Command 编排
// - openPanel / revealPanel / closePanel API（含幂等 + beforeClose hook + 自动 active 切换）
// - bindAdapter → 把 dockview 操作委派 WorkspaceAdapter（manager 自己 0 行 dockview 代码）
// - executeCommand（含 when 条件求值；when 假抛错让 UI toast）
// - serialize/deserialize（透明 string；adapter 决定 dockview JSON 格式）
//
// 分层契约（ADR-021 强制）：
// - manager 是纯 TS，无 Vue / dockview 依赖
// - 业务代码（panel SFC / pinia store）通过 useWorkspaceManager composable 拿到 manager
//   实例；禁止直接 import dockview API（只有 DockviewAdapter 允许）
//
// 100% 单测覆盖（issue#35 验收硬要求）：deps 注入 PanelRegistry / ContextKeyService /
// WorkspacePersistence / WorkspaceAdapter 四接口，单测 mock 所有。

import { ContextKeyService } from './contextKey'
import { PanelRegistry } from './panelRegistry'
import {
  CommandAlreadyRegisteredError,
  PanelNotRegisteredError,
  type Command,
  type PanelDescriptor,
  type WorkspaceAdapter,
  type WorkspacePersistence,
} from './types'
import { evalWhen, parseWhen } from './whenDsl'

export interface WorkspaceManagerOpts {
  /** 默认 new PanelRegistry()；测试可注 spy */
  registry?: PanelRegistry
  /** 默认 new ContextKeyService()；测试可注 spy */
  contextKeys?: ContextKeyService
  /** 持久化注入；省略 = 不持久化（manager 仍可用，只是 layout/lastActive 不入 KV） */
  persistence?: WorkspacePersistence
}

type PanelLifecycleCallback = (id: string) => void
type ChangedCallback = () => void

export class WorkspaceManager {
  readonly registry: PanelRegistry
  readonly contextKeys: ContextKeyService
  readonly persistence: WorkspacePersistence | null

  private adapter: WorkspaceAdapter | null = null
  private readonly commands = new Map<string, Command>()
  private readonly openPanels = new Set<string>()
  private activePanelId: string | null = null
  private readonly activatedSubs = new Set<PanelLifecycleCallback>()
  private readonly deactivatedSubs = new Set<PanelLifecycleCallback>()
  /** review P1 修复 (F-5.6 / F-4.5)：动态 register/unregister 时通知 ActivityBar 重渲染 */
  private readonly panelsChangedSubs = new Set<ChangedCallback>()
  /** review P1 修复 (F-3.4)：动态 register/unregister command 时通知 CommandPalette 重过滤 */
  private readonly commandsChangedSubs = new Set<ChangedCallback>()
  /** review P1 修复 (F-8.1)：closePanel 并发护栏，防 beforeClose await 期间用户连点二次 close 重复触发 hook */
  private readonly closingPanels = new Set<string>()

  constructor(opts: WorkspaceManagerOpts = {}) {
    this.registry = opts.registry ?? new PanelRegistry()
    this.contextKeys = opts.contextKeys ?? new ContextKeyService()
    this.persistence = opts.persistence ?? null
  }

  // === Adapter 绑定 ===

  /**
   * 绑定 dockview 桥接层；不绑则所有 dockview 相关操作 no-op（manager 仍可用于纯单测）。
   *
   * review P0 修复：绑定时调 adapter.subscribeEvents(...)，让 dockview 真实状态变化
   * （fromJSON 还原 / 用户点 tab 切 active / 用户点 tab ✕ 关 panel）回灌 manager，
   * 解决 manager.openPanels / activePanelId 与 dockview 双轨漂移问题。
   */
  bindAdapter(adapter: WorkspaceAdapter): void {
    this.adapter = adapter
    adapter.subscribeEvents({
      onPanelMounted: (id) => this.syncPanelMounted(id),
      onPanelRemoved: (id) => this.syncPanelRemoved(id),
      onActivePanelChanged: (id) => this.syncActiveChanged(id),
    })
  }

  // === Adapter → manager 回灌（幂等，被 adapter 在 dockview 事件触发时调用） ===

  /** dockview 端 panel mount：把 openPanels / contextKey 拉齐（manager.openPanel 自己也会调一次，幂等 OK） */
  private syncPanelMounted(id: string): void {
    if (this.openPanels.has(id)) return // 已同步
    this.openPanels.add(id)
    this.contextKeys.set(`panel.${id}.visible`, true)
  }

  /**
   * dockview 端 panel removed：把 openPanels 拉齐 + 触发 deactivated。
   * 关键路径：用户点 dockview tab 自带的 ✕ 关闭 panel（**绕过 manager.closePanel**，beforeClose hook 死代码 — 见 review F-2.3）。
   * 此 sync 让 manager 状态对齐，但 beforeClose 仍被绕过 — 真正修复需 dockview 提供 cancelable hook（M2 follow-up）。
   */
  private syncPanelRemoved(id: string): void {
    if (!this.openPanels.has(id)) return // 已同步
    this.openPanels.delete(id)
    this.contextKeys.set(`panel.${id}.visible`, false)
    if (this.activePanelId === id) {
      this.activePanelId = null
      this.contextKeys.set('activePanel', null)
    }
    for (const cb of this.deactivatedSubs) cb(id)
  }

  /** dockview 端 active panel 切换（用户点 tab）；幂等 */
  private syncActiveChanged(id: string | null): void {
    if (this.activePanelId === id) return // 已同步
    if (id === null) {
      this.activePanelId = null
      this.contextKeys.set('activePanel', null)
      return
    }
    this.activePanelId = id
    this.contextKeys.set('activePanel', id)
    for (const cb of this.activatedSubs) cb(id)
  }

  // === Panel 注册 ===

  registerPanel(descriptor: PanelDescriptor): void {
    this.registry.register(descriptor)
    this.notifyPanelsChanged()
  }

  unregisterPanel(id: string): void {
    this.registry.unregister(id)
    this.notifyPanelsChanged()
  }

  // === Panel 生命周期 ===

  /**
   * 打开 panel（幂等：已打开则 reveal）。
   * @throws {PanelNotRegisteredError} panel id 未注册
   */
  openPanel(id: string, params?: unknown): void {
    const descriptor = this.registry.get(id)
    if (!descriptor) throw new PanelNotRegisteredError(id)
    if (this.openPanels.has(id)) {
      // 幂等：已打开 → 仅切 active（不重 mount）
      this.revealActive(id)
      return
    }
    this.adapter?.mountPanel(descriptor, params)
    this.openPanels.add(id)
    this.contextKeys.set(`panel.${id}.visible`, true)
    this.revealActive(id)
  }

  /**
   * 切换 panel 为 active（如未打开则自动 openPanel）。
   * 已 active = no-op。
   *
   * review P1 修复 (F-1.2)：补 params 透传 — 之前 revealPanel 不接 params，未打开走自动 open
   * 路径时 panel 拿不到调用方的 params（command "revealPanel A" 永远以 undefined 启动）。
   */
  revealPanel(id: string, params?: unknown): void {
    const descriptor = this.registry.get(id)
    if (!descriptor) throw new PanelNotRegisteredError(id)
    if (!this.openPanels.has(id)) {
      this.openPanel(id, params)
      return
    }
    if (this.activePanelId === id) return // no-op
    this.adapter?.revealPanel(id)
    this.revealActive(id)
  }

  /**
   * 关闭 panel（含 beforeClose hook）。
   * - force=true 跳过 beforeClose 直接关
   * - 未打开 / 未注册 = no-op（不抛）
   * - **并发护栏** (review P1 F-8.1)：beforeClose await 期间被再次 close 调用直接返 false，
   *   防同一 panel 的 beforeClose 被并发触发（用户高速连按 ✕ / 命令面板二次执行 close 命令场景）。
   * @returns 是否真的关闭（beforeClose 拒绝 / 并发护栏命中时返 false）
   */
  async closePanel(id: string, force = false): Promise<boolean> {
    const descriptor = this.registry.get(id)
    if (!descriptor) return false
    if (!this.openPanels.has(id)) return false
    if (this.closingPanels.has(id)) return false // 并发护栏

    if (!force && descriptor.beforeClose) {
      this.closingPanels.add(id)
      try {
        const ok = await descriptor.beforeClose(undefined)
        if (!ok) return false
      } finally {
        this.closingPanels.delete(id)
      }
    }

    this.adapter?.unmountPanel(id)
    this.openPanels.delete(id)
    this.contextKeys.set(`panel.${id}.visible`, false)
    if (this.activePanelId === id) {
      this.activePanelId = null
      this.contextKeys.set('activePanel', null)
    }
    for (const cb of this.deactivatedSubs) cb(id)
    return true
  }

  isPanelOpen(id: string): boolean {
    return this.openPanels.has(id)
  }

  listOpenPanels(): string[] {
    return Array.from(this.openPanels)
  }

  getActivePanel(): string | null {
    return this.activePanelId
  }

  onPanelActivated(cb: PanelLifecycleCallback): () => void {
    this.activatedSubs.add(cb)
    return () => {
      this.activatedSubs.delete(cb)
    }
  }

  onPanelDeactivated(cb: PanelLifecycleCallback): () => void {
    this.deactivatedSubs.add(cb)
    return () => {
      this.deactivatedSubs.delete(cb)
    }
  }

  /**
   * registry 变化（register / unregister panel）订阅。
   * review P1 修复 (F-5.6 / F-4.5)：ActivityBar 用此事件触发重渲染，否则动态注册的 panel
   * 不会出现在导航栏（registry.list() 不是 reactive，Vue 无法感知）。
   */
  onPanelsChanged(cb: ChangedCallback): () => void {
    this.panelsChangedSubs.add(cb)
    return () => {
      this.panelsChangedSubs.delete(cb)
    }
  }

  /**
   * commands 变化（registerCommand / unregisterCommand）订阅。
   * review P1 修复 (F-3.4)：CommandPalette 用此事件触发列表重过滤，否则动态注册的命令
   * 在 palette 打开期间不会出现。
   */
  onCommandsChanged(cb: ChangedCallback): () => void {
    this.commandsChangedSubs.add(cb)
    return () => {
      this.commandsChangedSubs.delete(cb)
    }
  }

  // === Command ===

  /**
   * 注册命令。
   * @throws {CommandAlreadyRegisteredError} 同 id 已注册
   */
  registerCommand(command: Command): void {
    if (this.commands.has(command.id)) {
      throw new CommandAlreadyRegisteredError(command.id)
    }
    this.commands.set(command.id, command)
    this.notifyCommandsChanged()
  }

  unregisterCommand(id: string): void {
    if (this.commands.delete(id)) {
      this.notifyCommandsChanged()
    }
  }

  /** 列出所有命令；filterByWhen=true 只返当前 when 求真的命令 */
  listCommands(filterByWhen = false): Command[] {
    const all = Array.from(this.commands.values())
    if (!filterByWhen) return all
    return all.filter((c) => this.isWhenSatisfied(c.when))
  }

  /**
   * 执行命令。
   * @throws Error 命令未注册 / when 假
   */
  async executeCommand(id: string): Promise<unknown> {
    const command = this.commands.get(id)
    if (!command) throw new Error(`Command '${id}' not found`)
    if (!this.isWhenSatisfied(command.when)) {
      throw new Error(`Command '${id}' is disabled (when='${command.when}' is false)`)
    }
    return command.handler()
  }

  // === when DSL 求值（panel.when 与 command.when 共用） ===

  /** 求 when 表达式真值；空 when 或无效表达式都返 true（fail-open） */
  isWhenSatisfied(when: string | undefined): boolean {
    if (!when) return true
    try {
      const ast = parseWhen(when)
      return evalWhen(ast, this.contextKeys.asMap())
    } catch (e) {
      console.error('[workspace] invalid when DSL:', when, e)
      return false
    }
  }

  // === ContextKey 转发 ===

  getContextKey(key: string): unknown {
    return this.contextKeys.get(key)
  }

  setContextKey(key: string, value: unknown): void {
    this.contextKeys.set(key, value)
  }

  subscribeContextKeys(keys: string | string[], cb: (changedKey: string) => void): () => void {
    return this.contextKeys.subscribe(keys, cb)
  }

  // === Layout 序列化 ===

  /** 透明序列化（dockview JSON string）；未绑 adapter 返 empty default */
  serialize(): string {
    if (!this.adapter) return '{"grid":null}'
    return this.adapter.serialize()
  }

  /** 反序列化；失败静默 log（让 UI 走 default layout，不阻塞启动） */
  async deserialize(json: string): Promise<void> {
    if (!this.adapter) return
    try {
      this.adapter.deserialize(json)
    } catch (e) {
      console.error('[workspace] deserialize failed:', e)
    }
  }

  // === 持久化便捷方法（仅当 persistence 注入时生效） ===

  async loadLayoutFromKv(): Promise<void> {
    if (!this.persistence) return
    const layout = await this.persistence.loadLayout()
    if (layout) await this.deserialize(layout)
  }

  async saveLayoutToKv(): Promise<void> {
    if (!this.persistence) return
    await this.persistence.saveLayout(this.serialize())
  }

  async saveLastActiveToKv(): Promise<void> {
    if (!this.persistence || !this.activePanelId) return
    await this.persistence.saveLastActive(this.activePanelId)
  }

  async loadLastActiveFromKv(): Promise<string | null> {
    if (!this.persistence) return null
    return this.persistence.loadLastActive()
  }

  // === 内部 ===

  /** 切 active panel + 触发 activated 订阅 */
  private revealActive(id: string): void {
    if (this.activePanelId === id) return
    this.activePanelId = id
    this.contextKeys.set('activePanel', id)
    for (const cb of this.activatedSubs) cb(id)
  }

  private notifyPanelsChanged(): void {
    for (const cb of this.panelsChangedSubs) cb()
  }

  private notifyCommandsChanged(): void {
    for (const cb of this.commandsChangedSubs) cb()
  }
}
