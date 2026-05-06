import { invoke } from './ipc'
import type { PersonaSummary } from '@/types/persona'

/** 读取 personas + persona_snapshots，返回完整 raw markdown 与元信息。 */
export function loadPersona(id: string): Promise<PersonaSummary> {
  return invoke<PersonaSummary>('persona_load', { id })
}

/** 把指定 persona 设为 active（其余 is_active=0），跨重启保留。 */
export function activatePersona(id: string): Promise<void> {
  return invoke<void>('persona_activate', { id })
}
