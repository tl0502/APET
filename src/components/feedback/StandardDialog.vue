<script setup lang="ts">
// StandardDialog：项目统一弹窗外观与交互（issue #8）。
// - 包 EP ElDialog，外观由 components.css 的 .aipet-dialog 统一覆盖
// - 关闭路径：点 X / 点遮罩 / 按 ESC 三条全保留（loading=true 时屏蔽避免误关）
// - loading=true：body 加 spinner overlay；footer 整体灰禁（pointer-events:none + opacity:0.5）
// - footer slot 自由（不内置 cancel/confirm，PRD §6 弹窗语义多样）
import { useSlots } from 'vue'
import { ElDialog } from 'element-plus'

interface Props {
  modelValue: boolean
  title?: string
  width?: number | string
  loading?: boolean
  closeOnClickModal?: boolean
  closeOnPressEscape?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  title: '',
  width: 480,
  loading: false,
  closeOnClickModal: true,
  closeOnPressEscape: true,
})

const emit = defineEmits<{ 'update:modelValue': [boolean] }>()

const slots = useSlots()

function dialogWidth(): string {
  return typeof props.width === 'number' ? `${props.width}px` : props.width
}

function onUpdate(value: boolean) {
  emit('update:modelValue', value)
}
</script>

<template>
  <ElDialog
    :model-value="modelValue"
    :title="title"
    :width="dialogWidth()"
    :close-on-click-modal="closeOnClickModal && !loading"
    :close-on-press-escape="closeOnPressEscape && !loading"
    :show-close="!loading"
    class="aipet-dialog"
    append-to-body
    @update:model-value="onUpdate"
  >
    <div class="aipet-dialog__body" :class="{ 'is-loading': loading }">
      <slot />
      <div v-if="loading" class="aipet-dialog__loading">
        <span class="aipet-dialog__spinner" aria-label="loading"></span>
      </div>
    </div>
    <template v-if="slots.footer" #footer>
      <div class="aipet-dialog__footer" :class="{ 'is-loading': loading }">
        <slot name="footer" />
      </div>
    </template>
  </ElDialog>
</template>
