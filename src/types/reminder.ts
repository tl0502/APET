// #22 ReminderService 前端类型契约。
// 与后端 services/reminder.rs 同步（serde camelCase 序列化）。

export type ReminderTriggerType = 'once' | 'daily'
export type ReminderPriority = 'soft' | 'hard'

export interface Reminder {
  id: string
  title: string
  triggerType: ReminderTriggerType
  triggerSpec: string
  priority: ReminderPriority
  enabled: boolean
  snoozeCount: number
  nextFireAt: string | null
  createdAt: string
  updatedAt: string
}

export interface ReminderCreateInput {
  title: string
  triggerType: ReminderTriggerType
  triggerSpec: string
  priority?: ReminderPriority
}

export interface ReminderUpdateInput {
  title?: string
  triggerType?: ReminderTriggerType
  triggerSpec?: string
  priority?: ReminderPriority
  enabled?: boolean
}

/** scheduler tick 触发时全局广播；pet 窗气泡 + tasks 窗列表都 listen。 */
export interface ReminderFiredPayload {
  reminderId: string
  priority: ReminderPriority
  title: string
  snoozeCount: number
}

/** 启动期 catch-up 合并：30min 内未触发的多条 reminder 合并一条 toast 给 tasks 窗。 */
export interface ReminderCatchUpItem {
  reminderId: string
  title: string
  priority: ReminderPriority
}

export type SnoozeMinutes = 5 | 15 | 30
export const SNOOZE_OPTIONS: readonly SnoozeMinutes[] = [5, 15, 30] as const

/** 与后端 MAX_SNOOZE_COUNT 同步；UI 在 snoozeCount==3 时隐藏稍后按钮（防 backend MaxSnoozeExceeded）。 */
export const MAX_SNOOZE_COUNT = 3

/** Tauri event 名（架构 §683 契约 + 本 issue 拍板）。 */
export const REMINDER_FIRED_EVENT = 'reminder:fired'
export const REMINDER_CATCH_UP_EVENT = 'reminder:catch_up'

/**
 * 模板预设库（hardcode；前 3 个 id 与 onboarding Step 4 ReminderIntentsView INTENTS 对齐）。
 * #29 实例化 onboarding KV `onboarding:reminder_intents` 时直接按 id 反查本表 → CreateInput。
 *
 * 注：daily HH:MM M2 按 UTC 解释；中国用户感受为 +8h 偏移（如 23:00 → 7AM 本地）。
 * follow-up #29/M3 接入本地时区后此偏移消失。每 N 分钟形式（triggerSpec 为
 * 星号-斜杠-N-空格-星号-空格-星号）不受时区影响。
 */
export interface ReminderTemplate {
  id: string
  emoji: string
  label: string
  hint: string
  triggerType: ReminderTriggerType
  triggerSpec: string
  priority: ReminderPriority
}

export const REMINDER_TEMPLATES: readonly ReminderTemplate[] = [
  {
    id: 'water',
    emoji: '💧',
    label: '喝水',
    hint: '每 30 分钟',
    triggerType: 'daily',
    triggerSpec: '*/30 * *',
    priority: 'soft',
  },
  {
    id: 'sit_long',
    emoji: '🪑',
    label: '久坐起身',
    hint: '每 60 分钟',
    triggerType: 'daily',
    triggerSpec: '*/60 * *',
    priority: 'soft',
  },
  {
    id: 'focus_study',
    emoji: '📚',
    label: '学习专注',
    hint: '每天 09:00（UTC，约本地 17:00）',
    triggerType: 'daily',
    triggerSpec: '09:00',
    priority: 'hard',
  },
  {
    id: 'stretch',
    emoji: '🧘',
    label: '伸展活动',
    hint: '每 90 分钟',
    triggerType: 'daily',
    triggerSpec: '*/90 * *',
    priority: 'soft',
  },
  {
    id: 'early_sleep',
    emoji: '🌙',
    label: '早睡',
    hint: '每天 23:00（UTC，约本地 07:00）',
    triggerType: 'daily',
    triggerSpec: '23:00',
    priority: 'soft',
  },
] as const
