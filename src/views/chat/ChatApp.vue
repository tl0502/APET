<script setup lang="ts">
// ChatApp (#33 phase A 重写)：chat 磁吸窗 chrome 壳层。
//
// 重构前：1338 行单文件混合（chrome + 磁吸 + 业务状态 + 业务行为）。
// 重构后：仅 chrome 层 —— window-root + SnapGhost + .app-surface > ChatBody。
//
// 业务层下沉到：
// - src/stores/conversation.ts（Pinia singleton；conversations / messages / streaming / draft / send / cancel ...）
// - src/components/chat/ChatBody.vue（sidebar + content-header + message-scroll + composer）
//
// 本文件唯一职责：
// 1) 磁吸 hooks（useSnapWindow / useFocusAOT）—— chat 磁吸窗专有，HubPanel 不调
// 2) Windows 11 transparency bug workaround —— 真透明窗专有
// 3) snap preview / field 视觉反馈 CSS（.app-surface 上的 5 edge variant box-shadow + glow）
// 4) selfLean transform 应用到 .window-root（拖动 chat 接近 pet 时 ≤3px 视觉吸引）
// 5) AppBody 的 ✕ close 走 hideChat（不退窗、进托盘）

import { computed, onMounted } from 'vue'
import { LogicalSize, getCurrentWindow } from '@tauri-apps/api/window'

import ChatBody from '@/components/chat/ChatBody.vue'
import SnapGhost from '@/components/SnapGhost.vue'
import { useFocusAOT } from '@/composables/useFocusAOT'
import { useSnapWindow } from '@/composables/useSnapWindow'
import { hideChat } from '@/services/window'

// #30 磁吸窗口系统：chat 作为参与磁吸的窗口，挂 listener + dragSession + persistence
const {
  isPreviewAnchor: chatIsPreviewAnchor,
  previewEdgeFor: chatPreviewEdge,
  previewIntensityFor: chatPreviewIntensity,
  isFieldAnchor: chatIsFieldAnchor,
  fieldIntensityFor: chatFieldIntensity,
  selfLean: chatSelfLean,
} = useSnapWindow('chat')

const chatSnapPreviewClass = computed(() => {
  const cls: Record<string, boolean> = {
    'snap-preview': chatIsPreviewAnchor.value,
    'snap-field-anchor': chatIsFieldAnchor.value,
  }
  if (chatIsPreviewAnchor.value && chatPreviewEdge.value) {
    cls[`snap-preview--edge-${chatPreviewEdge.value}`] = true
  }
  return cls
})

const chatSnapPreviewStyle = computed(() => ({
  '--snap-preview-intensity': String(chatPreviewIntensity.value),
  '--snap-field-intensity': String(chatFieldIntensity.value),
}))

// #30 follow-up H：focus-driven AOT
useFocusAOT()

// Phase F (#31 follow-up C)：self-lean transform 应用到最外层 .window-root
const chatLeanStyle = computed(() => {
  const lean = chatSelfLean.value
  if (!lean) return {}
  return { transform: `translate(${lean.dx.toFixed(2)}px, ${lean.dy.toFixed(2)}px)` }
})

onMounted(async () => {
  // #30 Windows 11 transparency bug workaround：transparent:true + decorations:false 时
  // 首次绘制 webview 背景为白色直到首次 resize 才变透明（Tauri #4881 / #10318 / #8308）。
  // 启动期主动 set_size(currentSize) 触发一次 redraw 规避。
  try {
    const w = getCurrentWindow()
    const sz = await w.outerSize()
    const scale = await w.scaleFactor()
    const logical = sz.toLogical(scale)
    await w.setSize(new LogicalSize(logical.width, logical.height))
  } catch (e) {
    console.warn('[ChatApp] transparency redraw workaround failed:', e)
  }
})

async function handleClose() {
  await hideChat()
}
</script>

<template>
  <!-- window-root → SnapGhost + app-surface > ChatBody。
       app-surface 是唯一实体层（14px 圆角 + overflow:hidden + shadow-float），
       transparent webview 让圆角外透明、桌面透出。
       snap-preview / snap-preview--edge-* 5 variant 在被拖目标时覆盖式注入边描线 + glow。 -->
  <div class="window-root" :style="chatLeanStyle">
    <SnapGhost source-label="chat" />
    <div class="app-surface" :class="chatSnapPreviewClass" :style="chatSnapPreviewStyle">
      <ChatBody @close="handleClose" />
    </div>
  </div>
</template>

<style scoped>
/* === window-root（全透明缓冲层）===
   100% × 100% 占满 webview；最外层 transform 注入（不影响内部 layout）。 */
.window-root {
  width: 100%;
  height: 100%;
  background: transparent;
  transition: transform 160ms var(--aipet-ease-standard);
}

/* === app-surface（唯一实体层，L0 windowbg）===
   14px CSS 圆角 + overflow:hidden 把所有子内容裁成圆角。
   transparent 窗口 + 此元素 opaque → 圆角外为透明 webview → 桌面透出。
   shadow-float 提供浮起感；snap-preview modifier 在被拖目标时覆盖式注入边描线 + glow。 */
.app-surface {
  width: 100%;
  height: 100%;
  background: var(--aipet-color-bg);
  border-radius: var(--aipet-radius-window);
  overflow: hidden;
  display: flex;
  flex-direction: column;
  box-shadow: var(--aipet-shadow-float);
  transition: box-shadow 180ms var(--aipet-ease-standard);
}

/* Snap-preview state（拖 pet 接近 chat 时反馈）：
   覆盖式 box-shadow（含 2px primary 描边 + 24px primary glow + 浮起阴影）。
   transparent:true 让 box-shadow 自然溢出 .app-surface 边界（无需 window-root padding）。 */
.app-surface.snap-preview {
  box-shadow:
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}

.app-surface.snap-preview--edge-right {
  box-shadow:
    inset -3px 0 22px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 60%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}

.app-surface.snap-preview--edge-left {
  box-shadow:
    inset 3px 0 22px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 60%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}

.app-surface.snap-preview--edge-top {
  box-shadow:
    inset 0 3px 22px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 60%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}

.app-surface.snap-preview--edge-bottom {
  box-shadow:
    inset 0 -3px 22px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 60%),
        transparent
      ),
    0 0 0 2px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 75%),
        transparent
      ),
    0 0 24px
      color-mix(
        in srgb,
        var(--aipet-color-primary) calc(var(--snap-preview-intensity, 0) * 40%),
        transparent
      ),
    var(--aipet-shadow-float);
}
</style>
