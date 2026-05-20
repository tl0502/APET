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

  constructor(opts: WorkspaceManagerOpts = {}) {
    this.registry = opts.registry ?? new PanelRegistry()
    this.contextKeys = opts.contextKeys ?? new ContextKeyService()
    this.persistence = opts.persistence ?? null
  }

  // === Adapter 绑定 ===

  /** 绑定 dockview 桥接层；不绑则所有 dockview 相关操作 no-op（manager 仍可用于纯单测） */
  bindAdapter(adapter: WorkspaceAdapter): void {
    this.adapter = adapter
  }

  // === Panel 注册 ===

  registerPanel(descriptor: PanelDescriptor): void {
    this.registry.register(descriptor)
  }

  unregisterPanel(id: string): void {
    this.registry.unregister(id)
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
   */
  revealPanel(id: string): void {
    const descriptor = this.registry.get(id)
    if (!descriptor) throw new PanelNotRegisteredError(id)
    if (!this.openPanels.has(id)) {
      this.openPanel(id)
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
   * @returns 是否真的关闭（beforeClose 拒绝时返 false）
   */
  async closePanel(id: string, force = false): Promise<boolean> {
    const descriptor = this.registry.get(id)
    if (!descriptor) return false
    if (!this.openPanels.has(id)) return false

    if (!force && descriptor.beforeClose) {
      const ok = await descriptor.beforeClose(undefined)
      if (!ok) return false
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
  }

  unregisterCommand(id: string): void {
    this.commands.delete(id)
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
}
