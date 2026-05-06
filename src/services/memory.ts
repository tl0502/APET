import { invoke } from './ipc'
import type { MemoryItem } from '@/types/memory'

/** KV 偏好表读单值；不存在返回 null。 */
export function getMemory(key: string): Promise<string | null> {
  return invoke<string | null>('memory_get', { key })
}

/** UPSERT KV 偏好（source 自动标 'user_set'）。 */
export function setMemory(key: string, value: string): Promise<void> {
  return invoke<void>('memory_set', { key, value })
}

/** 列全部 KV，按 key 升序。 */
export function listMemories(): Promise<MemoryItem[]> {
  return invoke<MemoryItem[]>('memory_list')
}

/** 按 key 删除。 */
export function deleteMemory(key: string): Promise<void> {
  return invoke<void>('memory_delete', { key })
}
