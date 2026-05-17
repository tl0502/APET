<script setup lang="ts">
// ReminderPanel：tasks 窗"提醒"tab 的主面板（issue #22）。
//
// 组成:
// - 模板预设区(REMINDER_TEMPLATES 6 个常量按钮)→ 点击后打开 Form 弹窗预填
// - 列表(ReminderList)→ 排序: enabled 优先 + 按 nextFireAt 升序
// - 新建按钮 → 打开 Form 弹窗(空表单)
// - Form 弹窗(ElDialog 包 ReminderForm)→ submit/cancel
// - listen 'reminder:fired' / 'reminder:catch_up' → 触发列表 refresh(状态/snoozeCount 等)

import { computed, onBeforeUnmount, onMounted, ref, useTemplateRef } from 'vue'
import { ElButton, ElDialog, ElEmpty, ElMessage } from 'element-plus'
import { CirclePlus, Refresh } from '@element-plus/icons-vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import ReminderList from '@/components/tasks/ReminderList.vue'
import ReminderForm from '@/components/tasks/ReminderForm.vue'
import {
  completeReminder,
  createReminder,
  deleteReminder,
  listReminders,
  snoozeReminder,
  updateReminder,
} from '@/services/reminder'
import {
  REMINDER_CATCH_UP_EVENT,
  REMINDER_FIRED_EVENT,
  REMINDER_TEMPLATES,
  type Reminder,
  type ReminderCatchUpItem,
  type ReminderCreateInput,
  type ReminderFiredPayload,
  type SnoozeMinutes,
} from '@/types/reminder'

const reminders = ref<Reminder[]>([])
const busy = ref(false)
const formVisible = ref(false)
const editing = ref<Reminder | null>(null)
const formRef = useTemplateRef<InstanceType<typeof ReminderForm>>('formRef')

const sorted = computed(() => {
  return [...reminders.value].sort((a, b) => {
    // enabled 优先；同状态按 nextFireAt 升序（null 推到末尾）
    if (a.enabled !== b.enabled) return a.enabled ? -1 : 1
    if (!a.nextFireAt && !b.nextFireAt) return 0
    if (!a.nextFireAt) return 1
    if (!b.nextFireAt) return -1
    return a.nextFireAt.localeCompare(b.nextFireAt)
  })
})

async function refresh() {
  try {
    reminders.value = await listReminders()
  } catch (e) {
    ElMessage.error(`加载提醒列表失败：${e}`)
  }
}

function openCreate() {
  editing.value = null
  formVisible.value = true
}

function openEdit(r: Reminder) {
  editing.value = r
  formVisible.value = true
}

function applyTemplate(tpl: (typeof REMINDER_TEMPLATES)[number]) {
  editing.value = null
  formVisible.value = true
  // 等下一帧 ReminderForm mount，再调 exposed.applyTemplate
  requestAnimationFrame(() => {
    formRef.value?.applyTemplate({
      title: tpl.label,
      triggerType: tpl.triggerType,
      triggerSpec: tpl.triggerSpec,
      priority: tpl.priority,
    })
  })
}

async function onSubmit(input: ReminderCreateInput) {
  busy.value = true
  try {
    if (editing.value) {
      await updateReminder(editing.value.id, input)
      ElMessage.success('已保存')
    } else {
      await createReminder(input)
      ElMessage.success('已创建')
    }
    formVisible.value = false
    await refresh()
  } catch (e) {
    ElMessage.error(`保存失败：${e}`)
  } finally {
    busy.value = false
  }
}

async function onSnooze(payload: { id: string; minutes: SnoozeMinutes }) {
  busy.value = true
  try {
    await snoozeReminder(payload.id, payload.minutes)
    ElMessage.success(`已稍后 ${payload.minutes} 分钟`)
    await refresh()
  } catch (e) {
    ElMessage.error(`稍后失败：${e}`)
  } finally {
    busy.value = false
  }
}

async function onComplete(id: string) {
  busy.value = true
  try {
    await completeReminder(id)
    ElMessage.success('已完成')
    await refresh()
  } catch (e) {
    ElMessage.error(`完成失败：${e}`)
  } finally {
    busy.value = false
  }
}

async function onDelete(id: string) {
  if (!confirm('确定删除这条提醒？历史记录会一并清除。')) return
  busy.value = true
  try {
    await deleteReminder(id)
    ElMessage.success('已删除')
    await refresh()
  } catch (e) {
    ElMessage.error(`删除失败：${e}`)
  } finally {
    busy.value = false
  }
}

async function onToggleEnabled(r: Reminder) {
  busy.value = true
  try {
    await updateReminder(r.id, { enabled: !r.enabled })
    await refresh()
  } catch (e) {
    ElMessage.error(`切换失败：${e}`)
  } finally {
    busy.value = false
  }
}

let unlistenFired: UnlistenFn | null = null
let unlistenCatchUp: UnlistenFn | null = null

onMounted(async () => {
  await refresh()
  // reminder:fired → 本窗列表刷新（next_fire_at / snooze 已变）
  unlistenFired = await listen<ReminderFiredPayload>(REMINDER_FIRED_EVENT, () => {
    refresh().catch(() => {})
  })
  // catch_up 启动期一次性事件 → 给用户一个合并 toast
  unlistenCatchUp = await listen<ReminderCatchUpItem[]>(REMINDER_CATCH_UP_EVENT, (e) => {
    const items = e.payload ?? []
    if (items.length === 0) return
    const titles = items.map((it) => it.title).join('、')
    ElMessage({
      type: 'info',
      duration: 6000,
      message: `刚才你不在，错过 ${items.length} 条提醒：${titles}`,
    })
  })
})

onBeforeUnmount(() => {
  unlistenFired?.()
  unlistenCatchUp?.()
})
</script>

<template>
  <div class="reminder-panel">
    <header class="reminder-panel__header">
      <div class="reminder-panel__title-row">
        <h2 class="reminder-panel__title">提醒</h2>
        <div class="reminder-panel__actions">
          <ElButton :icon="Refresh" :loading="busy" text @click="refresh">刷新</ElButton>
          <ElButton type="primary" :icon="CirclePlus" @click="openCreate">新建提醒</ElButton>
        </div>
      </div>
      <p class="reminder-panel__hint">
        点下方模板预设一键创建，或自定义新建。提醒到点会在桌宠头顶气泡 + 系统通知双通道展示。
      </p>
    </header>

    <section class="reminder-panel__templates">
      <button
        v-for="tpl in REMINDER_TEMPLATES"
        :key="tpl.id"
        type="button"
        class="reminder-template"
        :disabled="busy"
        @click="applyTemplate(tpl)"
      >
        <span class="reminder-template__emoji">{{ tpl.emoji }}</span>
        <span class="reminder-template__label">{{ tpl.label }}</span>
        <span class="reminder-template__hint">{{ tpl.hint }}</span>
      </button>
    </section>

    <main class="reminder-panel__body">
      <ReminderList
        v-if="sorted.length"
        :reminders="sorted"
        :busy="busy"
        @edit="openEdit"
        @snooze="onSnooze"
        @complete="onComplete"
        @delete="onDelete"
        @toggle-enabled="onToggleEnabled"
      />
      <ElEmpty v-else description="还没有提醒，从上方模板快速创建一条">
        <template #image>
          <span style="font-size: 56px">🔔</span>
        </template>
      </ElEmpty>
    </main>

    <ElDialog
      v-model="formVisible"
      :title="editing ? '编辑提醒' : '新建提醒'"
      width="520"
      :close-on-click-modal="false"
      append-to-body
    >
      <ReminderForm
        ref="formRef"
        :reminder="editing"
        :busy="busy"
        @submit="onSubmit"
        @cancel="formVisible = false"
      />
    </ElDialog>
  </div>
</template>

<style scoped>
.reminder-panel {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-4);
  padding: var(--aipet-space-4) var(--aipet-space-2);
}

.reminder-panel__header {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.reminder-panel__title-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--aipet-space-3);
}

.reminder-panel__title {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
}

.reminder-panel__actions {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-2);
}

.reminder-panel__hint {
  margin: 0;
  font-size: 13px;
  color: var(--aipet-color-text-2);
  line-height: 1.5;
}

.reminder-panel__templates {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: var(--aipet-space-2);
}

.reminder-template {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  padding: var(--aipet-space-3);
  background: var(--aipet-color-surface);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-lg);
  cursor: pointer;
  font: inherit;
  text-align: left;
  transition: border-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    background-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    transform var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.reminder-template:hover:not(:disabled) {
  border-color: var(--aipet-color-primary);
  background: color-mix(in srgb, var(--aipet-color-primary) 6%, var(--aipet-color-surface));
}

.reminder-template:active:not(:disabled) {
  transform: scale(0.98);
}

.reminder-template:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.reminder-template__emoji {
  font-size: 24px;
}

.reminder-template__label {
  font-size: 14px;
  font-weight: 600;
  color: var(--aipet-color-text-1);
}

.reminder-template__hint {
  font-size: 12px;
  color: var(--aipet-color-text-3);
}

.reminder-panel__body {
  flex: 1 1 auto;
  min-height: 200px;
}
</style>
