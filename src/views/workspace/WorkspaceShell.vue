<script setup lang="ts">
// WorkspaceShell：ActivityBar + DockviewVue 主区水平布局
//
// 关键技术（spike #32 实操坑落地）：
// - 坑 1 ResizeObserver：<DockviewVue> 是空 div，只在 onMounted 调一次 api.layout(w,h)
//   后不再跟随容器尺寸变化（dockview-vue 6.x 设计缺陷）。本组件外挂 RO 监听 dock-host
//   容器尺寸，rAF + 50ms trailing 节流喂 api.layout()。spike 验证 50ms 够用不闪烁。
// - 坑 2 component registry：dockview 通过 `findComponent(parent, name)` 查 Vue 全局
//   注册表；main.ts 已 app.component(id, comp) 注册 3 个 placeholder（同 SFC 多 id 注册）。
// - 坑 4 popout：不暴露 popout API；MVP 不需要，Tauri WebView2 结构性不可行。
//
// dispose 链：
// - onBeforeUnmount disconnect RO + adapter.dispose（adapter 内部 api.clear，然后 dockview-vue
//   SFC unmount 时自动 dispose dockview 实例）
//
// 启动布局还原顺序：
//   1) onReady (dockview ready) → adapter.dockview ready
//   2) WorkspaceApp.vue onMounted 内 await mgr.loadLayoutFromKv() → deserialize 还原 layout
//      （若失败 / 无 KV → 走 default 三 panel：openPanel 3 次）
//   3) await mgr.loadLastActiveFromKv() → revealPanel(savedId)

import { onBeforeUnmount, ref } from 'vue'
import { DockviewVue, type DockviewReadyEvent } from 'dockview-vue'

import { DockviewAdapter } from '@/lib/workspace/dockviewAdapter'
import { useWorkspaceManager } from '@/composables/useWorkspaceManager'

const mgr = useWorkspaceManager()

const hostRef = ref<HTMLElement | null>(null)
let adapter: DockviewAdapter | null = null
let resizeObserver: ResizeObserver | null = null
let pendingLayoutFrame: number | null = null
let pendingLayoutTimer: ReturnType<typeof setTimeout> | null = null
let windowResizeHandler: (() => void) | null = null

const emit = defineEmits<{ ready: [] }>()

function onReady(event: DockviewReadyEvent) {
  const api = event.api
  adapter = new DockviewAdapter(api)
  mgr.bindAdapter(adapter)

  // spike #32 坑 1：DockviewVue 的内部 onMounted 只调一次 api.layout(clientWidth, clientHeight)；
  // 之后窗口 resize / 容器尺寸变化都不会跟随。必须外挂 ResizeObserver。
  // rAF 合帧 + 50ms trailing：避免 dragstart 期间高频回调撑爆 layout 重算。
  const host = hostRef.value
  if (!host) {
    console.warn('[WorkspaceShell] dock host element not found at onReady — RO not attached')
    return
  }

  const triggerLayout = (fireReady = false) => {
    if (pendingLayoutFrame !== null) return
    pendingLayoutFrame = requestAnimationFrame(() => {
      pendingLayoutFrame = null
      const rect = host.getBoundingClientRect()
      // dockview-core 内部 layout 期望整数像素；rect width/height 在高 DPI 有小数尾巴
      api.layout(Math.round(rect.width), Math.round(rect.height))
      // review P0 修复（F-3.2 前端）：emit('ready') 必须等首次 layout 真生效后才发，
      // 否则 WorkspaceApp 收到 ready 立刻 loadLayoutFromKv → fromJSON 在错的尺寸下
      // 还原 → split 比例错乱 / 0 宽 panel 被 hide。fireReady=true 标记本次是首次同步。
      if (fireReady) emit('ready')
    })
  }

  const scheduleLayout = () => {
    if (pendingLayoutTimer !== null) clearTimeout(pendingLayoutTimer)
    pendingLayoutTimer = setTimeout(() => {
      pendingLayoutTimer = null
      triggerLayout()
    }, 50)
  }

  resizeObserver = new ResizeObserver(scheduleLayout)
  resizeObserver.observe(host)

  // 首次同步：onMounted dockview 已调一次 api.layout 但用的是 mount-time 尺寸，
  // 此刻容器尺寸可能已变（如 sidebar/devtools 已展开）；再调一次保险。
  // fireReady=true → rAF 内 layout 之后才 emit ready，确保 WorkspaceApp restoreLayout
  // 拿到的是已正确 layout 过的 dockview（修 F-3.2 前端 race）。
  triggerLayout(true)

  // 兜底 #2：极少数情况下 webview 内 RO 不触发（spike 已知低概率），监听 window resize 作 fallback。
  windowResizeHandler = scheduleLayout
  window.addEventListener('resize', scheduleLayout)
}

onBeforeUnmount(() => {
  if (pendingLayoutFrame !== null) cancelAnimationFrame(pendingLayoutFrame)
  if (pendingLayoutTimer !== null) clearTimeout(pendingLayoutTimer)
  resizeObserver?.disconnect()
  resizeObserver = null

  if (windowResizeHandler) {
    window.removeEventListener('resize', windowResizeHandler)
    windowResizeHandler = null
  }

  if (adapter) {
    adapter.dispose()
    adapter = null
  }
})
</script>

<template>
  <div class="workspace-shell">
    <slot name="activity" />
    <div ref="hostRef" class="workspace-shell__dock-host">
      <!-- spike #32 坑 1 配套：DockviewVue 必须显式 100% 尺寸样式，否则其内部 div 高度
           collapse 到 0，渲染 onlyWhenVisible 模式下没 panel 显示；always 模式下 0×0
           不可见。 -->
      <DockviewVue
        class="workspace-shell__dock"
        style="height: 100%; width: 100%; display: block;"
        @ready="onReady"
      />
    </div>
  </div>
</template>

<style scoped>
.workspace-shell {
  flex: 1 1 auto;
  display: flex;
  flex-direction: row;
  min-height: 0;
  min-width: 0;
}

.workspace-shell__dock-host {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  position: relative;
  background: var(--aipet-color-bg);
  overflow: hidden;
}

.workspace-shell__dock {
  width: 100%;
  height: 100%;
}
</style>
