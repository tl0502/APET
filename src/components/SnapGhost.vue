<script setup lang="ts">
// SnapGhost.vue — Phase B (#31 follow-up C)。
//
// 渲染"松手后我会落在这里"的 ghost outline 提示。
// 实现约束：Tauri webview 不能画到 outer rect 外，所以 ghost 画在 source 自己窗内
// （相对 source 当前位置偏移到 finalRect）。可见的偏移段（最多 ATTACH_ZONE 60px）
// 传达"未来落点"语义；ghost 大部分与 source 当前位置重叠（视觉上是"重影"）。
//
// 用法：在 .chat-window 内挂 `<SnapGhost source-label="chat" />`。
//   ghost rect 仅在 dragSession.preview 状态显示；source 端 onMounted/onBeforeUnmount
//   随 useSnapWindow 复合管理。

import { computed } from 'vue'
import { previewFinalRect } from '@/lib/snap/dragSession'
import { windowRegistry } from '@/lib/snap/windowRegistry'

const props = defineProps<{
  /** 本组件挂载所在的 source 窗 label（'chat' / 'pet' 等）。
   *  ghost 偏移基于 windowRegistry.get(sourceLabel)?.rect 与 previewFinalRect 之差。 */
  sourceLabel: string
}>()

/** ghost 应渲染的偏移 + 尺寸（相对 source 当前 outer rect）。
 *  null = 不渲染（非 preview 状态 / source 不在 registry）。 */
const ghost = computed<{ dx: number; dy: number; w: number; h: number } | null>(() => {
  const final = previewFinalRect.value
  if (!final) return null
  const self = windowRegistry.get(props.sourceLabel)
  if (!self) return null
  return {
    dx: final.x - self.rect.x,
    dy: final.y - self.rect.y,
    w: final.w,
    h: final.h,
  }
})
</script>

<template>
  <div
    v-if="ghost"
    class="snap-ghost"
    :style="{
      transform: `translate(${ghost.dx}px, ${ghost.dy}px)`,
      width: `${ghost.w}px`,
      height: `${ghost.h}px`,
    }"
    aria-hidden="true"
  ></div>
</template>

<style scoped>
/* ghost outline：绝对定位在 source 窗左上角（dx/dy=0 时与 source 重合），
   transform translate 到 finalRect 偏移。Tauri webview 限制：ghost 超出 source 窗的
   像素会被裁切，仅"重叠 + 偏移露出"那部分可见（这正是设计意图：可见的偏移就是
   "松手会移动这么远"的视觉提示）。

   视觉：12px 圆角 + dashed 主色 border + 主色 8% 填充。无动画（intensity 已传达逼近度）。 */
.snap-ghost {
  position: absolute;
  top: 0;
  left: 0;
  pointer-events: none;
  border: 1.5px dashed
    color-mix(in srgb, var(--aipet-color-primary) 60%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, var(--aipet-color-primary) 8%, transparent);
  box-sizing: border-box;
  /* 不抢戏：透明度由 preview state 控制可见；z-index 略低于 chat 内容，避免遮挡输入 */
  z-index: 0;
  transition: transform 60ms linear;
}
</style>
