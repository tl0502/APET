<script setup lang="ts">
// ChatInput：textarea + 发送/取消 toggle（issue #14 + Vercel 风美化）。
// - Enter 发送 / Shift+Enter 换行;中英输入法合成期（compositionstart/end）回车不触发发送
// - inputDisabled=true: textarea 禁用（V3 多对话并发下永远 false——用户要求流式中也能编辑）
// - sendDisabled=true: 发送按钮置灰（current 对话流式中时父组件传 true）
// - showCancel=true: 发送按钮替换为"取消"且可点（仅当前 view = 流式中的对话时）
// - cancelling=true: 用户已点取消等后端收尾的中间态,按钮 disable + icon 改 loading 圆点
// - 空字符串 / 仅空白不发送（trim 后非空才允许）
// - data-tauri-drag-region="false" 防止误识别为拖动区
//
// 视觉:Vercel 风纯色 + 1px border(8px 圆角,无毛玻璃无阴影);EP textarea 内边 border/padding
// 抹平视觉为单一气泡;发送按钮 32×32 绝对定位右下角,icon-only(Promotion/Close/Loading);
// :focus-within 时外框转 primary + 2px 半透 focus ring。
//
// V3 重构（B13）继续保留:把 V2 的 disabled 单 prop 拆成 inputDisabled + sendDisabled——
// 让"输入框是否禁用"和"发送按钮是否可点"完全解耦。
import { computed, ref } from 'vue'
import { ElButton, ElIcon, ElInput } from 'element-plus'
import { Close, Loading, Promotion } from '@element-plus/icons-vue'

interface Props {
  modelValue: string
  /** 输入框禁用（V3:永远 false;保留 prop 是为了未来要禁用时不破坏 API）。 */
  inputDisabled: boolean
  /** 发送按钮禁用（current 对话流式中时为 true,但 showCancel=true 时此值忽略）。 */
  sendDisabled: boolean
  /** 显示"取消"替代"发送"。父组件守护:仅当 stream phase 且当前 view = 流式中对话时为 true。 */
  showCancel: boolean
  /** 取消中（用户已点取消,等后端 done/error 抵达期间）。仅 showCancel=true 时有意义;
   *  按钮文案改"取消中…"并 disable,避免重复点击。可选,省略 = false。 */
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
/** send 按钮成功触发后短暂亮起的 pulse 反馈(300ms keyframe);避免连续点击叠加,
 *  每次点击都重置 timer。仅 send 路径触发,cancel 不闪(语义不该庆祝)。 */
const sendPulsing = ref(false)
let sendPulseTimer: ReturnType<typeof setTimeout> | null = null

const canSend = computed(
  () => !props.sendDisabled && !props.inputDisabled && props.modelValue.trim().length > 0,
)

/** 屏幕阅读器/title 用文案;UI 只显图标。 */
const buttonAriaLabel = computed(() => {
  if (props.showCancel) return props.cancelling ? '取消中' : '取消'
  return '发送'
})

const buttonDisabled = computed(() => {
  if (props.showCancel) return props.cancelling
  return !canSend.value
})

const buttonType = computed<'primary' | 'danger'>(() =>
  props.showCancel ? 'danger' : 'primary',
)

function onInput(value: string | number | null | undefined) {
  emit('update:modelValue', value == null ? '' : String(value))
}

function onKeydown(e: Event | KeyboardEvent) {
  if (!(e instanceof KeyboardEvent)) return
  if (e.key !== 'Enter') return
  // 输入法合成期回车 = 选词,不发送
  if (composing.value || e.isComposing) return
  if (e.shiftKey) return // 显式换行
  e.preventDefault()
  if (canSend.value) {
    triggerSendPulse()
    emit('send')
  }
}

function triggerSendPulse() {
  if (sendPulseTimer) clearTimeout(sendPulseTimer)
  sendPulsing.value = true
  sendPulseTimer = setTimeout(() => {
    sendPulsing.value = false
    sendPulseTimer = null
  }, 320)
}

function onClickAction() {
  if (props.showCancel) {
    if (!props.cancelling) emit('cancel')
  } else if (canSend.value) {
    triggerSendPulse()
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
      placeholder="说点什么(Enter 发送,Shift+Enter 换行)"
      resize="none"
      class="chat-input__textarea"
      @update:model-value="onInput"
      @keydown="onKeydown"
      @compositionstart="composing = true"
      @compositionend="composing = false"
    />
    <ElButton
      :type="buttonType"
      :disabled="buttonDisabled"
      :aria-label="buttonAriaLabel"
      :title="buttonAriaLabel"
      circle
      :class="['chat-input__action', { 'is-pulsing': sendPulsing }]"
      @click="onClickAction"
    >
      <ElIcon :class="{ 'is-spinning': cancelling }">
        <Loading v-if="cancelling" />
        <Close v-else-if="showCancel" />
        <Promotion v-else />
      </ElIcon>
    </ElButton>
  </div>
</template>

<style scoped>
.chat-input {
  position: relative;
  /* Phase C floating composer：surface-raised 浮卡 + 14px 大圆角 + 上向阴影
     （--aipet-shadow-composer = hairline border + 弥散光，Telegram/Discord 浮卡风）。
     从 message-scroll 视觉上"浮起"一档，与之前贴底大 textarea 拉开层次。 */
  padding: var(--aipet-space-2) 56px var(--aipet-space-2) var(--aipet-space-3);
  background: var(--aipet-color-surface-raised);
  border: 1px solid var(--aipet-color-border);
  border-radius: 14px;
  box-shadow: var(--aipet-shadow-composer);
  min-height: 96px; /* 防 textarea 行数突变导致整 input 区域上下抖动 */
  box-sizing: border-box;
  transition: border-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    box-shadow var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.chat-input:focus-within {
  border-color: var(--aipet-color-primary);
  /* focus 时叠加 ring + 保留 composer 上向阴影，浮卡感不丢 */
  box-shadow: var(--aipet-ring-focus), var(--aipet-shadow-composer);
}

/* 内嵌 EP textarea:抹掉默认 border / 内 padding / 背景,纯文本气质 */
.chat-input__textarea :deep(.el-textarea__inner) {
  background: transparent;
  border: 0;
  box-shadow: none;
  padding: 6px 4px;
  font-size: 15px;
  line-height: 1.5;
  color: var(--aipet-color-text-1);
  font-family: var(--aipet-font-family-base);
}

.chat-input__textarea :deep(.el-textarea__inner)::placeholder {
  color: var(--aipet-color-text-3);
}

.chat-input__textarea :deep(.el-textarea__inner):focus {
  box-shadow: none;
  border-color: transparent;
}

/* 圆形发送按钮:右下绝对定位 32×32(Vercel 略小);保留 circle 因为 icon 按钮形态最克制 */
.chat-input__action {
  position: absolute;
  right: var(--aipet-space-2);
  bottom: var(--aipet-space-2);
  width: 32px;
  height: 32px;
  padding: 0;
  font-size: 16px;
}

.chat-input__action :deep(.el-icon) {
  font-size: 16px;
}

.chat-input__action .is-spinning {
  animation: aipet-input-spin 0.8s linear infinite;
}

/* send 成功瞬态 pulse:scale(1)→0.92→1.08→1,320ms 配 ease-emphasized 给"弹一下"
 *  反馈。仅 send 路径触发;cancel 不闪。 */
.chat-input__action.is-pulsing {
  animation: aipet-send-pulse 320ms var(--aipet-ease-emphasized);
}

@keyframes aipet-send-pulse {
  0% {
    transform: scale(1);
  }
  30% {
    transform: scale(0.92);
  }
  60% {
    transform: scale(1.08);
  }
  100% {
    transform: scale(1);
  }
}

@keyframes aipet-input-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
