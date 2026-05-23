<script setup lang="ts">
// PetCanvas：透明角色窗主画面（PRD §7.2 角色窗）。
// M1 spike 阶段仅渲染 VRM；hitbox 上报推到后续 task（A.6）。
// #10 拖动：pointerdown → getCurrentWindow().startDragging()（系统级拖动，OS 接管 mouse）。
// 整窗 100% 可拖（无按钮容器 / 穿透按钮，M2 W3 控制按钮区上线时再加 [data-no-drag] 隔离）。
//
// #16：复用到 onboarding 窗时通过 `:draggable="false"` 关掉拖动（onboarding 窗用系统 decorations
// 标题栏拖动，且窗口固定不应被 webview 内 startDragging 移动）；并 emit `loaded`/`error` 让
// SoulPledgeView 在 VRM 就绪 / 失败后再开播文案（用户拍板：等 isLoaded === true 再开播）。
//
// #24：尺寸不再硬编码 320×320。`size` prop 决定 canvas + container 实际像素；`view` prop
// 决定相机取景。两者都由父级单一驱动（pet 主窗 App.vue 负责 listen `pet:view-changed` 改 ref，
// 不让 PetCanvas 自己 listen 反向 emit 造成双 source —— onboarding 窗的 SoulPledgeView 用默认
// 'half' + 320×320 不参与 view_preset 体系）。
import { computed, ref, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useVRMModel } from '@/composables/useVRMModel'
import { usePetReaction } from '@/composables/usePetReaction'
import { cancelWander } from '@/services/livingPet'
import type { AvatarView } from '@/services/vrm'

interface Props {
  draggable?: boolean
  /** 取景模式，'half'（默认，胸口以上）/ 'full'（全身）。运行期变化会触发 runtime.setView() */
  view?: AvatarView
  /** 容器逻辑像素尺寸。默认 320×320（兼容 onboarding SoulPledgeView 不传场景）。 */
  size?: { width: number; height: number }
  /** #29 是否对 reminder:fired 事件作出反应（点头）。onboarding 场景应传 false。 */
  enableReaction?: boolean
}

const props = withDefaults(defineProps<Props>(), {
  draggable: true,
  view: 'half',
  size: () => ({ width: 320, height: 320 }),
  enableReaction: true,
})

const emit = defineEmits<{
  loaded: []
  error: [string]
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)

// public/avatar/avatar.vrm 由 Vite static serve；用户私有，.gitignore 已忽略。
const MODEL_URL = '/avatar/avatar.vrm'

const { isLoaded, errorMessage, runtime } = useVRMModel(canvasRef, MODEL_URL, props.view)

// #29 桌宠对 reminder:fired 的反应（点头）。onboarding 场景传 enable-reaction="false" 跳过。
usePetReaction(runtime, () => props.enableReaction)

const stageStyle = computed(() => ({
  width: `${props.size.width}px`,
  height: `${props.size.height}px`,
}))

watch(
  () => props.view,
  (v) => {
    runtime.setView(v)
  },
)

watch(
  () => [props.size.width, props.size.height] as const,
  ([w, h]) => {
    runtime.resize(w, h)
  },
)

// 把 composable 的 ref 翻译成 event；SoulPledgeView 只关心"何时可以开播"
// （成功 → loaded；失败 → error，view 也应开播，不能让 VRM 缺失卡死宣誓流程）。
watch(isLoaded, (v) => {
  if (v) emit('loaded')
})
watch(errorMessage, (v) => {
  if (v) emit('error', v)
})

async function onPointerDown(event: PointerEvent) {
  if (!props.draggable) return
  // 主键 (button=0) 才触发拖动，避免与 M2 后续右键菜单冲突。
  if (event.button !== 0) return
  // closest('[data-no-drag]') 兜底：M2 控制按钮容器上线后只需在该元素加 attr 即可隔离，无需改本处。
  if ((event.target as HTMLElement | null)?.closest('[data-no-drag]')) return
  // dev 期诊断：若 handler 都没跑，问题在 pointer event 传递；若跑到但 startDragging 抛错，看 catch
  if (import.meta.env.DEV) {
    console.log('[PetCanvas] pointerdown @', event.clientX, event.clientY, 'target=', event.target)
  }
  // #21 收尾 L1：拖动前先取消正在进行的 wander tween。fire-and-forget 不 await
  // 避免 IPC 往返延迟 startDragging（IPC <5ms 但保守起见不等）。无 wander 时是 no-op。
  void cancelWander().catch((e) => {
    console.warn('[PetCanvas] cancelWander failed (non-fatal):', e)
  })
  try {
    // ⚠️ 隐式契约（A3 注释 2026-05-19）：
    // useSnapWindow.onPointerDown 已在 capture 阶段先跑（registered with capture:true 在 window 上），
    // arm 了 dragSession。本处必须调 startDragging() 让 OS 接管拖动，否则：
    //   - 没有 OS-level move → 没有 tauri://move 事件 → useSnapWindow 的 onMoved 路径不触发
    //   - dragSession 永远卡在 armed 态，1s 后 ARMED_TIMEOUT_MS 自动回 idle
    //   - 表现：用户拖 pet 完全无反应，无错误日志
    // 替代方案（M3 follow-up）：把 startDragging 移到 useSnapWindow 内部，让 composable 自给自足。
    await getCurrentWindow().startDragging()
  } catch (e) {
    console.error('[PetCanvas] startDragging failed:', e)
  }
}

// 视线跟随：canvas 内鼠标位置 → NDC（x/y ∈ [-1,1]，y 翻转）。
// 整窗只有 320×320，pointermove 频次可接受不做节流；
// 离开 canvas → 视线回中（避免桌宠"盯着窗外某个固定方向"显得呆滞）。
function onPointerMove(event: PointerEvent) {
  const canvas = canvasRef.value
  if (!canvas) return
  const rect = canvas.getBoundingClientRect()
  if (rect.width === 0 || rect.height === 0) return
  const x = ((event.clientX - rect.left) / rect.width) * 2 - 1
  const y = -(((event.clientY - rect.top) / rect.height) * 2 - 1)
  runtime.setCursorNdc({ x, y })
}

function onPointerLeave() {
  runtime.setCursorNdc(null)
}
</script>

<template>
  <!-- T4 (#31)：data-snap-drag-trigger 让 useSnapWindow.onPointerDown 把 pet 整窗 VRM 区
       识别为 drag 起点，arm dragSession + forest snapshot；
       这样拖 pet 也走 preview / ESC cancel / commit tween 流程，与拖 chat 一致。
       原来 PetCanvas 用 startDragging 直接交给 OS，不挂 useSnapWindow listener。 -->
  <div
    class="pet-stage"
    :class="{ 'pet-stage--draggable': draggable }"
    :style="stageStyle"
    data-snap-drag-trigger
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerleave="onPointerLeave"
  >
    <canvas ref="canvasRef" class="pet-canvas"></canvas>
    <div v-if="!isLoaded && !errorMessage" class="hint">Loading VRM…</div>
    <div v-else-if="errorMessage" class="hint hint-error">
      VRM 加载失败：{{ errorMessage }}<br />
      请把一个 .vrm 文件放在 <code>public/avatar/avatar.vrm</code>
    </div>
  </div>
</template>

<style scoped>
.pet-stage {
  position: relative;
  /* width / height 由 :style="stageStyle" 注入（来自 props.size），#24 视角档位联动 */
}

/* draggable=true（pet 角色窗默认）：grab cursor 让透明像素也能接 pointerdown */
.pet-stage--draggable {
  cursor: grab;
}

.pet-stage--draggable:active {
  cursor: grabbing;
}

.pet-canvas {
  display: block;
  width: 100%;
  height: 100%;
}

.hint {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
  background: var(--aipet-color-surface);
  padding: var(--aipet-space-1) var(--aipet-space-3);
  border-radius: var(--aipet-radius-base);
  box-shadow: var(--aipet-shadow-sm);
  text-align: center;
  pointer-events: none;
  white-space: nowrap;
}

.hint-error {
  background: var(--aipet-color-error-surface);
  color: var(--aipet-color-danger);
  font-size: var(--aipet-font-size-xs);
  max-width: 290px;
  white-space: normal;
  line-height: var(--aipet-line-height-base);
}

.hint-error code {
  background: var(--aipet-color-code-bg);
  padding: 1px var(--aipet-space-1);
  border-radius: var(--aipet-radius-sm);
  font-family: var(--aipet-font-family-mono);
}
</style>
