<script setup lang="ts">
// ChatInput：textarea + 发送/取消 toggle（issue #14）。
// - Enter 发送 / Shift+Enter 换行；中英输入法合成期（compositionstart/end）回车不触发发送
// - inputDisabled=true：textarea 禁用（V3 多对话并发下永远 false——用户要求流式中也能编辑）
// - sendDisabled=true：发送按钮置灰（current 对话流式中时父组件传 true）
// - showCancel=true：发送按钮替换为"取消"且可点（仅当前 view = 流式中的对话时）
// - cancelling=true：用户已点取消等后端收尾的中间态，按钮文案"取消中…"且 disable，避免重复点击
// - 空字符串 / 仅空白不发送（trim 后非空才允许）
// - data-tauri-drag-region="false" 防止误识别为拖动区
//
// V3 重构（B13）：把 V2 的 disabled 单 prop 拆成 inputDisabled + sendDisabled——
// 让"输入框是否禁用"和"发送按钮是否可点"完全解耦，支持 ChatGPT 式 UX：
// 当前对话流式中 → 输入框可编辑（用户能写下一条草稿，切换不丢），但发送按钮灰（这条已在生成）。
// 切走到空闲对话 → 输入框可编辑且发送可点（开新流，并发跑）。
import { computed, ref } from 'vue'
import { ElButton, ElInput } from 'element-plus'

interface Props {
  modelValue: string
  /** 输入框禁用（V3：永远 false；保留 prop 是为了未来要禁用时不破坏 API）。 */
  inputDisabled: boolean
  /** 发送按钮禁用（current 对话流式中时为 true，但 showCancel=true 时此值忽略）。 */
  sendDisabled: boolean
  /** 显示"取消"替代"发送"。父组件守护：仅当 stream phase 且当前 view = 流式中对话时为 true。 */
  showCancel: boolean
  /** 取消中（用户已点取消，等后端 done/error 抵达期间）。仅 showCancel=true 时有意义；
   *  按钮文案改"取消中…"并 disable，避免重复点击。可选，省略 = false。 */
  cancelling?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  cancelling: false,
})
const emit = defineEmits<{
  'update:modelValue': [string]
  send: []
  cancel: []
}>()

const composing = ref(false)

const canSend = computed(
  () => !props.sendDisabled && !props.inputDisabled && props.modelValue.trim().length > 0,
)

const buttonLabel = computed(() => {
  if (props.showCancel) return props.cancelling ? '取消中…' : '取消'
  return '发送'
})

const buttonDisabled = computed(() => {
  if (props.showCancel) return props.cancelling
  return !canSend.value
})

function onInput(value: string | number | null | undefined) {
  emit('update:modelValue', value == null ? '' : String(value))
}

function onKeydown(e: Event | KeyboardEvent) {
  if (!(e instanceof KeyboardEvent)) return
  if (e.key !== 'Enter') return
  // 输入法合成期回车 = 选词，不发送
  if (composing.value || e.isComposing) return
  if (e.shiftKey) return // 显式换行
  e.preventDefault()
  if (canSend.value) emit('send')
}

function onClickAction() {
  if (props.showCancel) {
    if (!props.cancelling) emit('cancel')
  } else if (canSend.value) {
    emit('send')
  }
}
</script>

<template>
  <div class="chat-input" data-tauri-drag-region="false">
    <ElInput
      :model-value="modelValue"
      type="textarea"
      :rows="3"
      :disabled="inputDisabled"
      placeholder="说点什么（Enter 发送，Shift+Enter 换行）"
      resize="none"
      @update:model-value="onInput"
      @keydown="onKeydown"
      @compositionstart="composing = true"
      @compositionend="composing = false"
    />
    <ElButton
      :type="showCancel ? 'danger' : 'primary'"
      :disabled="buttonDisabled"
      class="chat-input__action"
      @click="onClickAction"
    >
      {{ buttonLabel }}
    </ElButton>
  </div>
</template>

<style scoped>
.chat-input {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-2);
}

.chat-input__action {
  align-self: flex-end;
}
</style>
