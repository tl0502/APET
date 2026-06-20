import { Channel } from '@tauri-apps/api/core'

import { invoke } from './ipc'
import type { PersonaSourceDraft } from '@/features/persona-workshop/types'
import type { StreamEvent } from '@/types/chat'
import type {
  PersonaDraftValidationResult,
  PersonaExportResult,
  PersonaImportResult,
  PersonaListItem,
  PersonaSaveResult,
  PersonaSnapshotSummary,
  PersonaSummary,
  SoulRuntimeProfile,
} from '@/types/persona'

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

export function validatePersonaDraft(
  draft: PersonaSourceDraft,
): Promise<PersonaDraftValidationResult> {
  return invoke<PersonaDraftValidationResult>('persona_validate_draft', { draft })
}

export function savePersonaDraft(draft: PersonaSourceDraft): Promise<PersonaSaveResult> {
  return invoke<PersonaSaveResult>('persona_save_draft', { draft })
}

export function saveAndActivatePersonaDraft(
  draft: PersonaSourceDraft,
): Promise<PersonaSaveResult> {
  return invoke<PersonaSaveResult>('persona_save_and_activate_draft', { draft })
}

export function activatePersonaSnapshot(snapshotId: number): Promise<void> {
  return invoke<void>('persona_activate_snapshot', { snapshotId })
}

/** 列出某 persona 的全部快照（倒序，标记 active）。工坊「历史」tab 用；恢复走 activatePersonaSnapshot。 */
export function listPersonaSnapshots(personaId: string): Promise<PersonaSnapshotSummary[]> {
  return invoke<PersonaSnapshotSummary[]>('persona_list_snapshots', { id: personaId })
}

export function getPersonaSnapshotProfile(snapshotId: number): Promise<SoulRuntimeProfile> {
  return invoke<SoulRuntimeProfile>('persona_get_snapshot_profile', { snapshotId })
}

export function importPersonaFromPath(
  path: string,
  activate = false,
): Promise<PersonaImportResult> {
  return invoke<PersonaImportResult>('persona_import', { path, activate })
}

export function exportPersonaSnapshot(
  snapshotId: number,
  path: string,
): Promise<PersonaExportResult> {
  return invoke<PersonaExportResult>('persona_export_snapshot', { snapshotId, path })
}

export function deletePersona(id: string): Promise<void> {
  return invoke<void>('persona_delete', { id })
}

/** 试聊（A2-D）history 单条；与 Rust service.rs::TrialTurn 对齐（camelCase）。 */
export interface TrialTurn {
  role: 'user' | 'assistant'
  content: string
}

/** 试聊同步返回；只含 assistant 临时 id（不落库，故无 conversationId）。 */
export interface TrialSendResult {
  messageId: string
}

/**
 * 人格工坊试聊：用未保存 draft 跑流式，**零持久副作用**（不建 conversation/message/snapshot）。
 * 流式事件走 onStream channel（delta/done/error/replaceMessage）；IPC 立即返 messageId。
 * 取消复用 chat 的 cancelChat(messageId)（后端同一 active_streams map）。
 */
export function trialSend(
  draft: PersonaSourceDraft,
  history: TrialTurn[],
  input: string,
  onStream: Channel<StreamEvent>,
): Promise<TrialSendResult> {
  return invoke<TrialSendResult>('persona_trial_send', { draft, history, input, onStream })
}
