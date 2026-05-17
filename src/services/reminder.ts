// #22 ReminderService 前端 IPC wrapper。
// 与 src-tauri/src/commands/reminder.rs 同步；6 命令 + 2 event 名常量。

import { invoke } from './ipc'
import type {
  Reminder,
  ReminderCreateInput,
  ReminderUpdateInput,
  SnoozeMinutes,
} from '@/types/reminder'

export function createReminder(input: ReminderCreateInput): Promise<Reminder> {
  return invoke<Reminder>('reminder_create', { input })
}

export function listReminders(): Promise<Reminder[]> {
  return invoke<Reminder[]>('reminder_list')
}

export function updateReminder(id: string, input: ReminderUpdateInput): Promise<Reminder> {
  return invoke<Reminder>('reminder_update', { id, input })
}

export function deleteReminder(id: string): Promise<void> {
  return invoke<void>('reminder_delete', { id })
}

export function snoozeReminder(id: string, minutes: SnoozeMinutes): Promise<Reminder> {
  return invoke<Reminder>('reminder_snooze', { id, minutes })
}

export function completeReminder(id: string): Promise<void> {
  return invoke<void>('reminder_complete', { id })
}
