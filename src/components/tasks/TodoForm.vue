<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import { ElInput, ElDatePicker, ElSelect, ElOption, ElButton, ElForm, ElFormItem } from 'element-plus'
import type { Todo, TodoCreateInput, TodoUpdateInput, TodoPriority, DueAtChange } from '@/types/todo'

interface Props {
  todo: Todo | null  // null = 创建，非 null = 编辑
  open: boolean
}

const props = defineProps<Props>()
const emit = defineEmits<{
  (e: 'submit', input: TodoCreateInput | TodoUpdateInput): void
  (e: 'cancel'): void
}>()

const titleInput = ref('')
const dueAtInput = ref<Date | null>(null)
const priorityInput = ref<TodoPriority>('normal')

watch(
  () => [props.todo, props.open],
  () => {
    if (props.open) {
      if (props.todo) {
        titleInput.value = props.todo.title
        dueAtInput.value = props.todo.dueAt ? new Date(props.todo.dueAt) : null
        priorityInput.value = props.todo.priority
      } else {
        titleInput.value = ''
        dueAtInput.value = null
        priorityInput.value = 'normal'
      }
    }
  },
  { immediate: true },
)

const titleValid = computed(() => titleInput.value.trim().length > 0)

function buildCreatePayload(): TodoCreateInput {
  return {
    title: titleInput.value.trim(),
    dueAt: dueAtInput.value ? dueAtInput.value.toISOString() : undefined,
    priority: priorityInput.value,
  }
}

function buildUpdatePayload(): TodoUpdateInput {
  if (!props.todo) return {}
  const out: TodoUpdateInput = {}
  if (titleInput.value.trim() !== props.todo.title) {
    out.title = titleInput.value.trim()
  }
  const oldDue = props.todo.dueAt ?? null
  const newDue = dueAtInput.value ? dueAtInput.value.toISOString() : null
  if (oldDue !== newDue) {
    if (newDue === null) {
      out.dueAt = { kind: 'clear' } as DueAtChange
    } else {
      out.dueAt = { kind: 'set', value: newDue } as DueAtChange
    }
  }
  if (priorityInput.value !== props.todo.priority) {
    out.priority = priorityInput.value
  }
  return out
}

function onSubmit() {
  if (!titleValid.value) return
  const payload = props.todo ? buildUpdatePayload() : buildCreatePayload()
  emit('submit', payload)
}

const disabledDate = (d: Date) => d.getTime() < Date.now() - 24 * 60 * 60 * 1000
</script>

<template>
  <ElForm @submit.prevent="onSubmit">
    <ElFormItem label="标题" :required="true">
      <ElInput v-model="titleInput" placeholder="例：复诊 / 买菜 / 写报告" maxlength="120" />
    </ElFormItem>
    <ElFormItem label="截止时间">
      <ElDatePicker
        v-model="dueAtInput"
        type="datetime"
        placeholder="可选；空表示无截止"
        :disabled-date="disabledDate"
        clearable
      />
    </ElFormItem>
    <ElFormItem label="优先级">
      <ElSelect v-model="priorityInput">
        <ElOption label="低" value="low" />
        <ElOption label="普通" value="normal" />
        <ElOption label="重要" value="high" />
      </ElSelect>
    </ElFormItem>
    <div class="todo-form__actions">
      <ElButton @click="emit('cancel')">取消</ElButton>
      <ElButton type="primary" :disabled="!titleValid" @click="onSubmit">
        {{ props.todo ? '保存' : '新建' }}
      </ElButton>
    </div>
  </ElForm>
</template>

<style scoped>
.todo-form__actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 12px;
}
</style>
