// Workspace 持久化（#35 ADR-021 P1）—— KV 桥接 + JSON 自愈
//
// 存储：config 表 KV
//   - `workspace:layout` → { v: 1, dockview: <SerializedDockview JSON string> }
//   - `workspace:last_active_panel` → 简单 string（panel id）
//
// schema v=1 包装的目的：dockview-vue 升级时若 SerializedDockview schema break，
// 我们能整体 drop 而不报 RuntimeError。参 snap/persistence.ts 同款自愈套路。
//
// 错误兜底：
// - getKv 抛错 → log + null（启动不阻塞，回 default layout）
// - JSON parse 失败 / 非 wrapper / v 不匹配 → log + clear KV + null（脏数据自愈）
// - setKv 抛错 → log（save 是 best-effort，不抛回 caller）

import { getConfig, setConfig } from '@/services/config'
import type { WorkspacePersistence } from './types'

export const WORKSPACE_LAYOUT_KV_KEY = 'workspace:layout'
export const WORKSPACE_LAST_ACTIVE_KV_KEY = 'workspace:last_active_panel'
export const WORKSPACE_LAYOUT_SCHEMA_V = 1 as const

/** KV 注入接口（测试 mock 用） */
export interface WorkspacePersistenceDeps {
  getKv(key: string): Promise<string | null>
  setKv(key: string, value: string): Promise<void>
}

const defaultDeps: WorkspacePersistenceDeps = {
  getKv: getConfig,
  setKv: setConfig,
}

interface LayoutWrapper {
  v: number
  dockview: string
}

function isLayoutWrapper(x: unknown): x is LayoutWrapper {
  if (typeof x !== 'object' || x === null) return false
  const o = x as Record<string, unknown>
  return typeof o.v === 'number' && typeof o.dockview === 'string'
}

/** 生产用持久化实现（基于 config KV）。 */
export class KvWorkspacePersistence implements WorkspacePersistence {
  constructor(private readonly deps: WorkspacePersistenceDeps = defaultDeps) {}

  async loadLayout(): Promise<string | null> {
    let raw: string | null
    try {
      raw = await this.deps.getKv(WORKSPACE_LAYOUT_KV_KEY)
    } catch (e) {
      console.error('[workspace/persistence] loadLayout getKv failed:', e)
      return null
    }
    if (raw === null || raw === '') return null

    let parsed: unknown
    try {
      parsed = JSON.parse(raw)
    } catch (e) {
      console.warn('[workspace/persistence] loadLayout JSON parse failed, clearing:', e)
      await this.clearLayoutSilently()
      return null
    }
    if (!isLayoutWrapper(parsed)) {
      console.warn('[workspace/persistence] loadLayout wrapper invalid, clearing')
      await this.clearLayoutSilently()
      return null
    }
    if (parsed.v !== WORKSPACE_LAYOUT_SCHEMA_V) {
      console.warn(
        `[workspace/persistence] loadLayout schema v=${parsed.v} mismatch (expect ${WORKSPACE_LAYOUT_SCHEMA_V}), clearing`,
      )
      await this.clearLayoutSilently()
      return null
    }
    return parsed.dockview
  }

  async saveLayout(layout: string): Promise<void> {
    const wrapper: LayoutWrapper = { v: WORKSPACE_LAYOUT_SCHEMA_V, dockview: layout }
    try {
      await this.deps.setKv(WORKSPACE_LAYOUT_KV_KEY, JSON.stringify(wrapper))
    } catch (e) {
      console.error('[workspace/persistence] saveLayout setKv failed:', e)
    }
  }

  async loadLastActive(): Promise<string | null> {
    try {
      const raw = await this.deps.getKv(WORKSPACE_LAST_ACTIVE_KV_KEY)
      return raw === null || raw === '' ? null : raw
    } catch (e) {
      console.error('[workspace/persistence] loadLastActive getKv failed:', e)
      return null
    }
  }

  async saveLastActive(id: string): Promise<void> {
    try {
      await this.deps.setKv(WORKSPACE_LAST_ACTIVE_KV_KEY, id)
    } catch (e) {
      console.error('[workspace/persistence] saveLastActive setKv failed:', e)
    }
  }

  private async clearLayoutSilently(): Promise<void> {
    try {
      await this.deps.setKv(WORKSPACE_LAYOUT_KV_KEY, '')
    } catch (e) {
      console.error('[workspace/persistence] clearLayout failed:', e)
    }
  }
}
