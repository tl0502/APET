// Panel 注册中心（#35 ADR-021 P1）
//
// 职责：
// - 管理 PanelDescriptor 索引（id → descriptor）
// - 注册去重 + id 格式校验（PascalCase，spike 坑 2 落地）
// - list() 返回快照供 ActivityBar / CommandPalette 消费
//
// 纯 TS class，无 Vue / dockview 依赖；单测覆盖见 __tests__/panelRegistry.test.ts。

import {
  InvalidPanelIdError,
  PanelAlreadyRegisteredError,
  type PanelDescriptor,
} from './types'

/** PascalCase 校验：必须以大写字母开头，仅字母数字 */
const PASCAL_CASE_RE = /^[A-Z][A-Za-z0-9]*$/

export class PanelRegistry {
  private readonly panels = new Map<string, PanelDescriptor>()

  /**
   * 注册 panel。
   * @throws {InvalidPanelIdError} id 不符合 PascalCase
   * @throws {PanelAlreadyRegisteredError} 同 id 已注册
   */
  register(descriptor: PanelDescriptor): void {
    if (!PASCAL_CASE_RE.test(descriptor.id)) {
      throw new InvalidPanelIdError(descriptor.id)
    }
    if (this.panels.has(descriptor.id)) {
      throw new PanelAlreadyRegisteredError(descriptor.id)
    }
    this.panels.set(descriptor.id, descriptor)
  }

  /** 注销 panel；不存在 = no-op */
  unregister(id: string): void {
    this.panels.delete(id)
  }

  /** 查询单个 descriptor；不存在返 undefined */
  get(id: string): PanelDescriptor | undefined {
    return this.panels.get(id)
  }

  /** 全量 descriptor 数组快照（注册顺序） */
  list(): PanelDescriptor[] {
    return Array.from(this.panels.values())
  }

  /** 当前注册 panel 数 */
  size(): number {
    return this.panels.size
  }

  /** 清空（测试 beforeEach 用） */
  clear(): void {
    this.panels.clear()
  }
}
