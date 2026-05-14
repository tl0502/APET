<script setup lang="ts">
// ImageCropperModal（#25 用户头像裁剪）：cropperjs v1 + ElDialog 包装。
//
// 用法（父组件）：
//   <ImageCropperModal v-model:open="visible" :src="srcDataUrl" @confirm="onCrop" />
//   onCrop(dataUrl: string) => 后端落盘
//
// 设计：
// - 1:1 aspectRatio + viewMode:1（裁剪框不超出图片）+ autoCropArea:0.9
// - 圆形预览靠 CSS border-radius:50% 给视觉提示；cropperjs 输出仍是方形 PNG（落盘后由
//   消费方加 CSS object-fit:cover + border-radius:50% 即可圆形显示）
// - 输出统一 512×512 PNG（与 VRM 截图分辨率对齐，足够 retina 大头像场景用）
// - ElDialog 的"显示完毕"事件 @opened 才 new Cropper —— 否则 img 元素尺寸 0，cropper 初始化
//   会拿到错误的 image natural size（v1 的已知边角）
// - close → destroy cropper 释放 WebGL/canvas 资源
import { computed, nextTick, ref, watch } from 'vue'
import Cropper from 'cropperjs'
import 'cropperjs/dist/cropper.css'
import { ElButton, ElDialog } from 'element-plus'

interface Props {
  open: boolean
  /** 待裁剪图的 dataURL（由父组件通过 avatar_read_to_data_url 拿到）。 */
  src: string
}
const props = defineProps<Props>()

const emit = defineEmits<{
  'update:open': [value: boolean]
  /** 用户确认 → 输出 512×512 PNG dataURL，父组件落盘。 */
  confirm: [dataUrl: string]
  cancel: []
}>()

const OUTPUT_SIZE = 512

const imgRef = ref<HTMLImageElement | null>(null)
let cropper: Cropper | null = null
const ready = ref(false)

const internalOpen = computed({
  get: () => props.open,
  set: (v) => emit('update:open', v),
})

function teardown() {
  cropper?.destroy()
  cropper = null
  ready.value = false
}

/** ElDialog @opened 触发 —— 此时 img 已 layout，Cropper 能拿到正确的 natural size。 */
async function onOpened() {
  await nextTick()
  if (!imgRef.value) return
  teardown()
  cropper = new Cropper(imgRef.value, {
    aspectRatio: 1,
    viewMode: 1,
    dragMode: 'move',
    autoCropArea: 0.9,
    background: false,
    movable: true,
    scalable: false,
    rotatable: false,
    zoomable: true,
    zoomOnTouch: true,
    zoomOnWheel: true,
    cropBoxMovable: true,
    cropBoxResizable: true,
    toggleDragModeOnDblclick: false,
    ready: () => {
      ready.value = true
    },
  })
}

function onCancel() {
  emit('cancel')
  internalOpen.value = false
}

function onConfirm() {
  if (!cropper || !ready.value) return
  // getCroppedCanvas 在 cropper ready 后才有效；imageSmoothingQuality:'high' 给最好下采样
  const canvas = cropper.getCroppedCanvas({
    width: OUTPUT_SIZE,
    height: OUTPUT_SIZE,
    imageSmoothingEnabled: true,
    imageSmoothingQuality: 'high',
  })
  if (!canvas) return
  const dataUrl = canvas.toDataURL('image/png')
  emit('confirm', dataUrl)
  internalOpen.value = false
}

// open: false → true 时清旧状态；true → false 时拆 cropper
watch(
  () => props.open,
  (v) => {
    if (!v) teardown()
  },
)
</script>

<template>
  <ElDialog
    v-model="internalOpen"
    title="裁剪头像"
    width="640"
    :close-on-click-modal="false"
    destroy-on-close
    align-center
    @opened="onOpened"
    @closed="teardown"
  >
    <div class="cropper-wrap">
      <img
        v-if="src"
        ref="imgRef"
        :src="src"
        alt="待裁剪"
        class="cropper-img"
      />
    </div>
    <p class="cropper-hint">
      拖动调整位置，滚轮缩放，或拉裁剪框边角。输出 {{ OUTPUT_SIZE }}×{{ OUTPUT_SIZE }} PNG。
    </p>
    <template #footer>
      <ElButton @click="onCancel">取消</ElButton>
      <ElButton type="primary" :disabled="!ready" @click="onConfirm">确认裁剪</ElButton>
    </template>
  </ElDialog>
</template>

<style scoped>
/* cropperjs 需要 img 在固定高度容器内才能正确计算裁剪区域;给 400px 默认高度避免拉伸过长。 */
.cropper-wrap {
  width: 100%;
  height: 400px;
  background: var(--aipet-color-surface-soft);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  overflow: hidden;
}
.cropper-img {
  display: block;
  max-width: 100%;
  /* cropperjs 会接管 img,初始 max-width 100% 防溢出;cropper 接管后会改 transform */
}
.cropper-hint {
  margin: var(--aipet-space-2) 0 0 0;
  color: var(--aipet-color-text-3);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
}

/* cropperjs 的圆形 stencil:CSS 视觉提示;不影响 toDataURL 输出(仍是方形 PNG) */
:deep(.cropper-view-box),
:deep(.cropper-face) {
  border-radius: 50%;
}
:deep(.cropper-line),
:deep(.cropper-point) {
  /* 圆形 stencil 下边角控制点显得突兀,弱化掉 */
  background-color: transparent;
}
:deep(.cropper-point.point-se) {
  /* 唯一保留右下角拖拽点让用户能改尺寸,弱化为透明背景 + 浅边 */
  background-color: rgba(255, 255, 255, 0.5);
  width: 8px;
  height: 8px;
}
</style>
