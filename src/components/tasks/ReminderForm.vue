<script setup lang="ts">
// ReminderForm：新建 / 编辑提醒的内嵌表单。
// 形态简化：3 种触发模式 radio + 各自的输入控件 + priority radio + title input。
// 提交时拼装 CreateInput / UpdateInput 契约（trigger_type / trigger_spec）。
//
// triggerKind UI 抽象（vs 后端 triggerType + triggerSpec）:
//   - 'every'  → triggerType='daily',  triggerSpec='*/N * *'      （每 N 分钟）
//   - 'at'     → triggerType='daily',  triggerSpec='HH:MM'        （每天 HH:MM，UTC）
//   - 'once'   → triggerType='once',   triggerSpec=ISO8601        （单次）
import { computed, reactive, watch } from 'vue'
import {
  ElButton,
  ElDatePicker,
  ElForm,
  ElFormItem,
  ElInput,
  ElInputNumber,
  ElRadio,
  ElRadioGroup,
  ElTimePicker,
} from 'element-plus'
import type {
  Reminder,
  ReminderCreateInput,
  ReminderPriority,
} from '@/types/reminder'

const props = defineProps<{
  reminder?: Reminder | null
  busy?: boolean
}>()

const emit = defineEmits<{
  submit: [payload: ReminderCreateInput]
  cancel: []
}>()

type TriggerKind = 'every' | 'at' | 'once'

interface FormState {
  title: string
  priority: ReminderPriority
  triggerKind: TriggerKind
  everyMinutes: number
  /** 仅 hour:minute 维度有意义；DatePicker 'date-time' 模式但只取 H/M */
  atTime: Date
  /** RFC3339 序列化目标；UI 选具体未来时刻 */
  onceTime: Date
}

function defaultState(): FormState {
  // onceTime 默认 5 分钟后，便于测试
  const oncePlus5 = new Date(Date.now() + 5 * 60 * 1000)
  return {
    title: '',
    priority: 'soft',
    triggerKind: 'every',
    everyMinutes: 30,
    atTime: new Date(2000, 0, 1, 9, 0),
    onceTime: oncePlus5,
  }
}

const state = reactive<FormState>(defaultState())

watch(
  () => props.reminder,
  (r) => {
    Object.assign(state, defaultState())
    if (!r) return
    state.title = r.title
    state.priority = r.priority
    if (r.triggerType === 'once') {
      state.triggerKind = 'once'
      const dt = new Date(r.triggerSpec)
      if (!Number.isNaN(dt.getTime())) state.onceTime = dt
    } else if (r.triggerSpec.startsWith('*/')) {
      state.triggerKind = 'every'
      const n = parseInt(r.triggerSpec.slice(2).split(' ')[0] ?? '30', 10)
      state.everyMinutes = Number.isFinite(n) && n > 0 ? n : 30
    } else {
      state.triggerKind = 'at'
      const [h, m] = r.triggerSpec.split(':').map((x) => parseInt(x, 10))
      if (Number.isFinite(h) && Number.isFinite(m)) {
        state.atTime = new Date(2000, 0, 1, h, m)
      }
    }
  },
  { immediate: true },
)

/** 暴露给父组件：模板预设按钮可调此方法预填 */
function applyTemplate(template: {
  title: string
  triggerType: 'daily' | 'once'
  triggerSpec: string
  priority: ReminderPriority
}) {
  state.title = template.title
  state.priority = template.priority
  if (template.triggerType === 'once') {
    state.triggerKind = 'once'
    state.onceTime = new Date(template.triggerSpec)
  } else if (template.triggerSpec.startsWith('*/')) {
    state.triggerKind = 'every'
    state.everyMinutes = parseInt(template.triggerSpec.slice(2).split(' ')[0] ?? '30', 10)
  } else {
    state.triggerKind = 'at'
    const [h, m] = template.triggerSpec.split(':').map((x) => parseInt(x, 10))
    state.atTime = new Date(2000, 0, 1, h ?? 9, m ?? 0)
  }
}

defineExpose({ applyTemplate })

const canSubmit = computed(() => state.title.trim().length > 0)

function pad2(n: number): string {
  return String(n).padStart(2, '0')
}

function onSubmit() {
  if (!canSubmit.value || props.busy) return
  let triggerType: 'once' | 'daily'
  let triggerSpec: string
  switch (state.triggerKind) {
    case 'once':
      triggerType = 'once'
      triggerSpec = state.onceTime.toISOString()
      break
    case 'at':
      triggerType = 'daily'
      triggerSpec = `${pad2(state.atTime.getHours())}:${pad2(state.atTime.getMinutes())}`
      break
    case 'every':
      triggerType = 'daily'
      triggerSpec = `*/${state.everyMinutes} * *`
      break
  }
  emit('submit', {
    title: state.title.trim(),
    triggerType,
    triggerSpec,
    priority: state.priority,
  })
}
</script>

<template>
  <ElForm label-position="top" class="reminder-form" @submit.prevent="onSubmit">
    <ElFormItem label="标题" required>
      <ElInput
        v-model="state.title"
        placeholder="例如：起来喝口水"
        maxlength="80"
        show-word-limit
        clearable
      />
    </ElFormItem>

    <ElFormItem label="提醒方式">
      <ElRadioGroup v-model="state.triggerKind">
        <ElRadio value="every">每隔 N 分钟</ElRadio>
        <ElRadio value="at">每天 HH:MM</ElRadio>
        <ElRadio value="once">单次（在某时刻）</ElRadio>
      </ElRadioGroup>
    </ElFormItem>

    <ElFormItem v-if="state.triggerKind === 'every'" label="间隔（分钟）">
      <ElInputNumber
        v-model="state.everyMinutes"
        :min="1"
        :max="1440"
        :step="5"
        controls-position="right"
      />
      <span class="reminder-form__hint">建议 ≥ 5 分钟，避免打扰过密</span>
    </ElFormItem>

    <ElFormItem v-else-if="state.triggerKind === 'at'" label="每天触发时刻（UTC）">
      <ElTimePicker
        v-model="state.atTime"
        format="HH:mm"
        value-format="HH:mm"
      />
      <span class="reminder-form__hint">M2 按 UTC 解释；本地时区转换在 #29 / M3 接入</span>
    </ElFormItem>

    <ElFormItem v-else label="触发时刻">
      <ElDatePicker
        v-model="state.onceTime"
        type="datetime"
        format="YYYY-MM-DD HH:mm"
        :clearable="false"
      />
    </ElFormItem>

    <ElFormItem label="优先级">
      <ElRadioGroup v-model="state.priority">
        <ElRadio value="soft">柔和（FOCUS 期入队）</ElRadio>
        <ElRadio value="hard">强提醒（可打断 FOCUS）</ElRadio>
      </ElRadioGroup>
    </ElFormItem>

    <div class="reminder-form__actions">
      <ElButton :disabled="busy" @click="emit('cancel')">取消</ElButton>
      <ElButton type="primary" :loading="busy" :disabled="!canSubmit" @click="onSubmit">
        {{ reminder ? '保存' : '创建' }}
      </ElButton>
    </div>
  </ElForm>
</template>

<style scoped>
.reminder-form {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
}

.reminder-form__hint {
  display: inline-block;
  margin-left: var(--aipet-space-3);
  color: var(--aipet-color-text-3);
  font-size: 12px;
}

.reminder-form__actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--aipet-space-2);
  padding-top: var(--aipet-space-2);
}
</style>
