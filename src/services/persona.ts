import { invoke } from './ipc'
import type { PersonaListItem, PersonaSummary } from '@/types/persona'

/** 读取 personas + persona_snapshots，返回完整 raw markdown 与元信息。 */
export function loadPersona(id: string): Promise<PersonaSummary> {
  return invoke<PersonaSummary>('persona_load', { id })
}

/** 列出所有人格 summary（不含 raw_markdown）。onboarding Step 2 / 设置面板列表用。 */
export function listPersonas(): Promise<PersonaListItem[]> {
  return invoke<PersonaListItem[]>('persona_list')
}

/** 把指定 persona 设为 active（其余 is_active=0），跨重启保留。 */
export function activatePersona(id: string): Promise<void> {
  return invoke<void>('persona_activate', { id })
}

/** 读当前激活人格 summary（含 raw_markdown）。#14 ChatPanel header 标题、设置面板提示用。 */
export function getActivePersona(): Promise<PersonaSummary> {
  return invoke<PersonaSummary>('persona_get_active')
}
