<script setup lang="ts">
// SashHandle (#33 phase B-redo)：master 与 detail 之间的可拖动分隔条。
//
// 设计：
// - 3px 宽热区 + ::after 1px 视觉线；hover 时整条变 primary 色
// - mousedown 启动 drag session：document.body cursor: col-resize / userSelect: none
// - mousemove rAF 节流（避免 60Hz+ 高频触发）
// - mouseup 结束 session 自动清掉 listener + 复位 body cursor
// - 父组件通过 v-model:width 双向绑定 store.masterWidth（store 内部 debounce 落 KV）

import { onBeforeUnmount, ref } from 'vue'

const props = defineProps<{
  /** 当前宽度（受控）；mousedown 时作为 dragStart 基准 */
  width: number
  /** 拖拽范围下限 */
  min?: number
  /** 拖拽范围上限 */
  max?: number
}>()

const emit = defineEmits<{
  'update:width': [value: number]
}>()

const dragging = ref(false)
let dragStartX = 0
let dragStartWidth = 0
let pendingFrame: number | null = null
let pendingDelta = 0

function applyDelta() {
  pendingFrame = null
  const min = props.min ?? 180
  const max = props.max ?? 380
  const next = Math.max(min, Math.min(max, Math.round(dragStartWidth + pendingDelta)))
  emit('update:width', next)
}

function onMouseMove(e: MouseEvent) {
  pendingDelta = e.clientX - dragStartX
  if (pendingFrame !== null) return
  pendingFrame = requestAnimationFrame(applyDelta)
}

function onMouseUp() {
  dragging.value = false
  document.body.style.cursor = ''
  document.body.style.userSelect = ''
  window.removeEventListener('mousemove', onMouseMove)
  window.removeEventListener('mouseup', onMouseUp)
  if (pendingFrame !== null) {
    cancelAnimationFrame(pendingFrame)
    pendingFrame = null
  }
}

function onMouseDown(e: MouseEvent) {
  if (e.button !== 0) return // 只响应左键
  e.preventDefault()
  dragging.value = true
  dragStartX = e.clientX
  dragStartWidth = props.width
  document.body.style.cursor = 'col-resize'
  document.body.style.userSelect = 'none'
  window.addEventListener('mousemove', onMouseMove)
  window.addEventListener('mouseup', onMouseUp)
}

onBeforeUnmount(() => {
  // 组件 unmount 时若仍在 drag → 清理全局 listener 防 leak
  if (dragging.value) onMouseUp()
})
</script>

<template>
  <div
    class="sash"
    :class="{ 'sash--dragging': dragging }"
    role="separator"
    aria-orientation="vertical"
    aria-label="调整列宽"
    @mousedown="onMouseDown"
  />
</template>

<style scoped>
.sash {
  flex: 0 0 3px;
  width: 3px;
  height: 100%;
  cursor: col-resize;
  position: relative;
  background: transparent;
  transition: background 100ms ease;
  z-index: 3;
}

.sash:hover {
  background: color-mix(in srgb, var(--aipet-color-primary) 30%, transparent);
}

.sash--dragging {
  background: color-mix(in srgb, var(--aipet-color-primary) 50%, transparent);
}

/* ::after 渲染 1px 视觉线（常态可见，hover / drag 时变深） */
.sash::after {
  content: '';
  position: absolute;
  left: 1px;
  top: 0;
  bottom: 0;
  width: 1px;
  background: var(--aipet-color-border-faint);
  pointer-events: none;
  transition: background 100ms ease;
}

.sash:hover::after {
  background: var(--aipet-color-border-strong);
}

.sash--dragging::after {
  background: var(--aipet-color-primary);
}
</style>
