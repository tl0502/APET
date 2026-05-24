<script setup lang="ts">
// PetCanvas：透明角色窗主画面（PRD §7.2 角色窗）。
// M1 spike 阶段仅渲染 VRM；#40（ADR-025）接入物理交互状态机后整窗 = AABB 单 body hitbox。
//
// #40 交互模型变更（draggable=true 时）：
// - **不再**在 pointerdown 立即 startDragging（会让 OS 立即接管 mouse → click/longpress 检测失效）
// - 改由 useInteractionRaycaster 状态机 ① pointermove 跨 5px 阈值时调 startDragging
//                                       ② click/dblclick/longpress/rclick/drag 5 事件路由
// - drag 起点同步 invoke interaction_record_drag_count → 30s 滑窗 ≥3 触发 Rust emit 抗议
// draggable=false（onboarding 复用）则关闭交互状态机，PetCanvas 仅作为 VRM 渲染表面。
//
// #16：复用到 onboarding 窗时通过 `:draggable="false"` 关掉拖动 + 物理交互；并 emit `loaded`/`error`
// 让 SoulPledgeView 在 VRM 就绪 / 失败后再开播文案（用户拍板：等 isLoaded === true 再开播）。
//
// #24：尺寸不再硬编码 320×320。`size` prop 决定 canvas + container 实际像素；`view` prop
// 决定相机取景。两者都由父级单一驱动（pet 主窗 App.vue 负责 listen `pet:view-changed` 改 ref，
// 不让 PetCanvas 自己 listen 反向 emit 造成双 source —— onboarding 窗的 SoulPledgeView 用默认
// 'half' + 320×320 不参与 view_preset 体系）。
import { computed, ref, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useVRMModel } from '@/composables/useVRMModel'
import { usePetReaction } from '@/composables/usePetReaction'
import {
  useInteractionRaycaster,
  type InteractionContextMenuEvent,
} from '@/composables/useInteractionRaycaster'
import { usePetInteractionFeedback } from '@/composables/usePetInteractionFeedback'
import PetContextMenu from '@/components/PetContextMenu.vue'
import MoodIcon from '@/components/MoodIcon.vue'
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
  /** #40 是否启用物理交互状态机（5 事件路由 + 右键菜单）。默认与 draggable 同步。 */
  enableInteraction?: boolean
  /** #40 当前 webview window label，用于 recordDragCount 多窗维度。 */
  windowLabel?: string
}

const props = withDefaults(defineProps<Props>(), {
  draggable: true,
  view: 'half',
  size: () => ({ width: 320, height: 320 }),
  enableReaction: true,
  enableInteraction: undefined,
  windowLabel: 'pet',
})

const emit = defineEmits<{
  loaded: []
  error: [string]
  contextmenu: [InteractionContextMenuEvent]
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)
const stageRef = ref<HTMLElement | null>(null)

// public/avatar/avatar.vrm 由 Vite static serve；用户私有，.gitignore 已忽略。
const MODEL_URL = '/avatar/avatar.vrm'

const { isLoaded, errorMessage, runtime } = useVRMModel(canvasRef, MODEL_URL, props.view)

// #29 桌宠对 reminder:fired 的反应（点头）。onboarding 场景传 enable-reaction="false" 跳过。
usePetReaction(runtime, () => props.enableReaction)

// #40 物理交互：默认与 draggable 同步（onboarding draggable=false → 关交互）。
const interactionEnabled = computed(
  () => props.enableInteraction ?? props.draggable,
)
const { contextMenu, closeContextMenu } = useInteractionRaycaster(stageRef, {
  windowLabel: props.windowLabel,
  enabled: () => interactionEnabled.value,
})

// #40 反馈消费（shake / nod / 气泡）：与状态机解耦，监听 Rust emit。
// #41 拆分：mood 显示职责移到 MoodIcon.vue（polling mood_get），本 composable 仅保
// shake + bubble 这类"动作即时反馈"，不再返 mood ref。
const {
  bubble: feedbackBubble,
  shaking,
} = usePetInteractionFeedback(runtime, () => interactionEnabled.value)

// 右键菜单事件出口：父组件可监听以做额外副作用（关闭其它 popover 等）。
watch(contextMenu, (v) => {
  if (v) emit('contextmenu', v)
})

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

/**
 * onboarding 兜底路径：interactionEnabled=false 但 draggable=true 时，
 * 仍走老的"pointerdown 即 startDragging" —— 不进入 click/longpress 状态机。
 * 当前 onboarding (draggable=false) + pet 主窗 (interactionEnabled=true) 都覆盖了，
 * 留这段做"draggable + 关交互"的极端组合兜底（M3+ 可能用到，如 wardrobe room 内的预览）。
 */
async function onLegacyPointerDown(event: PointerEvent) {
  if (!props.draggable || interactionEnabled.value) return
  if (event.button !== 0) return
  if ((event.target as HTMLElement | null)?.closest('[data-no-drag]')) return
  void cancelWander().catch(() => {})
  try {
    await getCurrentWindow().startDragging()
  } catch (e) {
    console.error('[PetCanvas] startDragging failed (legacy path):', e)
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
       原来 PetCanvas 用 startDragging 直接交给 OS，不挂 useSnapWindow listener。
       #40：startDragging 改由 useInteractionRaycaster 在跨阈值时调，data-snap-drag-trigger
       属性保留（useSnapWindow 仍按 capture-phase 在该元素 arm dragSession）。 -->
  <div
    ref="stageRef"
    class="pet-stage"
    :class="{ 'pet-stage--draggable': draggable, 'pet-stage--shake': shaking }"
    :style="stageStyle"
    data-snap-drag-trigger
    @pointerdown="onLegacyPointerDown"
    @pointermove="onPointerMove"
    @pointerleave="onPointerLeave"
  >
    <canvas ref="canvasRef" class="pet-canvas"></canvas>
    <div v-if="!isLoaded && !errorMessage" class="hint">Loading VRM…</div>
    <div v-else-if="errorMessage" class="hint hint-error">
      VRM 加载失败：{{ errorMessage }}<br />
      请把一个 .vrm 文件放在 <code>public/avatar/avatar.vrm</code>
    </div>
    <!-- #41 mood icon：1s polling mood_get，6 mood emoji 浮层；disabled_features 含
         'mood_icon' 时 v-if 内部隐藏。替换原 #40 stub（feedbackMood emit-driven 仅 3 态）。 -->
    <MoodIcon v-if="interactionEnabled" />
    <!-- #40 反应气泡：reaction_table.template flash 2s，自动消失。 -->
    <div v-if="feedbackBubble" class="pet-feedback-bubble" role="status">{{ feedbackBubble }}</div>
    <!-- #40 右键自绘菜单：anchor 到 pointer 位置；点击外部 / Esc 关闭。 -->
    <PetContextMenu
      v-if="contextMenu"
      :x="contextMenu.x"
      :y="contextMenu.y"
      @close="closeContextMenu"
    />
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

/* #40 物理交互最少可见反馈（ADR-025 2a-lite） */

/* shake：200ms 三段位移 keyframe，幅度小（±4px）避免桌宠跑出窗口边界。 */
.pet-stage--shake {
  animation: pet-shake 220ms var(--aipet-ease-standard);
}

@keyframes pet-shake {
  0% { transform: translateX(0); }
  25% { transform: translateX(-4px); }
  50% { transform: translateX(4px); }
  75% { transform: translateX(-2px); }
  100% { transform: translateX(0); }
}

/* #41 mood icon CSS 已迁到 MoodIcon.vue 内 scoped */

/* 反应气泡：reaction_table.template 文案，居中底部浮层 2s 自动消失。
   与 PetReminderBubble 区分：reminder 走顶部 stack，本气泡走底部，避免同时叠加遮挡。 */
.pet-feedback-bubble {
  position: absolute;
  bottom: 8px;
  left: 50%;
  transform: translateX(-50%);
  max-width: calc(100% - 24px);
  padding: 6px 12px;
  background: var(--aipet-color-surface-raised, var(--aipet-color-surface));
  border: 1px solid var(--aipet-color-border-strong, var(--aipet-color-border));
  border-radius: 12px;
  box-shadow: 0 4px 12px -4px rgba(0, 0, 0, 0.18);
  font-size: 12px;
  color: var(--aipet-color-text-1);
  line-height: 1.4;
  text-align: center;
  pointer-events: none;
  animation: pet-bubble-pop 160ms var(--aipet-ease-standard);
  z-index: 4;
}

@keyframes pet-bubble-pop {
  from { opacity: 0; transform: translate(-50%, 4px) scale(0.92); }
  to { opacity: 1; transform: translate(-50%, 0) scale(1); }
}
</style>
