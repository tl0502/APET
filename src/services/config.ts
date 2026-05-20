// `config` 表 KV IPC wrapper（#30 磁吸 + 未来 settings 表 fallback 复用）。
//
// 与 `memory` 表区分（services/memory.ts）：
// - memory 表：用户偏好（"你叫什么名字" 等 LLM 推断）
// - config 表：运行时配置（窗口位置 / active_conversation_id / snap:constraints 等）
//
// 详 src-tauri/src/services/config.rs 头注释。

import { invoke } from './ipc'

/** 读 config 表 KV；不存在返 null。 */
export function getConfig(key: string): Promise<string | null> {
  return invoke<string | null>('config_get', { key })
}

/** UPSERT config 表 KV。 */
export function setConfig(key: string, value: string): Promise<void> {
  return invoke<void>('config_set', { key, value })
}

/** 删 config 表 KV；不存在 = no-op。 */
export function deleteConfig(key: string): Promise<void> {
  return invoke<void>('config_delete', { key })
}

/** B3 修复：原子 persist snap:constraints + 跨 webview 广播 snap:constraint-changed。
 *  Rust 端先 await config_set 完成（KV 写盘），再 emit 广播——保证抵达任一 webview 时 KV 已是新值。
 *  senderId 透传给前端 listener 自过滤（A4 修复）。
 *
 *  传入的 value 是序列化好的 JSON 字符串（与 setConfig('snap:constraints', value) 等价）。 */
export function snapPersistAndBroadcast(value: string, senderId: string): Promise<void> {
  return invoke<void>('snap_persist_and_broadcast', { value, senderId })
}

/** #30 follow-up I：把全量 constraints + per-window visualInset 推到 Rust 端 SnapState。
 *  Rust 端 Moved 事件后接管 BFS solver + 批量 set_position，替代前端 group-drag 路径 N 次 IPC，
 *  消除链式拖动抖动（Windows webview2 setPosition IPC ≥5ms，N=3 链跌到 22Hz）。
 *
 *  caller：useSnapWindow 内 commit / detach / persistence load 路径上调一次。
 *  constraint 结构需与 Rust SnapConstraint 对齐（camelCase；仅 5 字段）。 */
export interface RustSnapConstraint {
  sourceId: string
  targetId: string
  sourceEdge: 'left' | 'right' | 'top' | 'bottom'
  targetEdge: 'left' | 'right' | 'top' | 'bottom'
  offset: number
}

export interface RustVisualInset {
  top: number
  right: number
  bottom: number
  left: number
}

export function snapSyncConstraints(
  constraints: RustSnapConstraint[],
  insets: Record<string, RustVisualInset>,
): Promise<void> {
  return invoke<void>('snap_sync_constraints', { constraints, insets })
}
