<script setup lang="ts">
// PetCanvas：320×320 透明角色窗主画面（PRD §7.2 角色窗）。
// M1 spike 阶段仅渲染 VRM；hitbox 上报推到后续 task（A.6）。
// #10 拖动：pointerdown → getCurrentWindow().startDragging()（系统级拖动，OS 接管 mouse）。
// 整窗 100% 可拖（无按钮容器 / 穿透按钮，M2 W3 控制按钮区上线时再加 [data-no-drag] 隔离）。
//
// #16：复用到 onboarding 窗时通过 `:draggable="false"` 关掉拖动（onboarding 窗用系统 decorations
// 标题栏拖动，且窗口固定不应被 webview 内 startDragging 移动）；并 emit `loaded`/`error` 让
// SoulPledgeView 在 VRM 就绪 / 失败后再开播文案（用户拍板：等 isLoaded === true 再开播）。
import { onBeforeUnmount, ref, watch } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useVRMModel } from '@/composables/useVRMModel'
import type { AvatarView } from '@/services/vrm'

interface Props {
  draggable?: boolean
  /** 取景模式，'half'（默认，胸口以上）/ 'full'（全身）。运行期变化会触发 runtime.setView() */
  view?: AvatarView
}

const props = withDefaults(defineProps<Props>(), {
  draggable: true,
  view: 'half',
})

const emit = defineEmits<{
  loaded: []
  error: [string]
}>()

const canvasRef = ref<HTMLCanvasElement | null>(null)

// public/avatar/avatar.vrm 由 Vite static serve；用户私有，.gitignore 已忽略。
const MODEL_URL = '/avatar/avatar.vrm'

const { isLoaded, errorMessage, runtime } = useVRMModel(canvasRef, MODEL_URL, props.view)

// 父级运行期切视角（如 Settings 改"角色窗 → 视角"）：用 setView 平移相机，不重挂模型。
// 注意：父级若同时改 canvas 尺寸（aspect ratio 联动），需自行调 runtime.resize()，
// 或更简单地让本组件 :key 绑定到外部 size 上强制重 mount（M1 阶段尺寸固定，暂不需要）。
watch(
  () => props.view,
  (v) => {
    runtime.setView(v)
  },
)

// dev-only：按 V 键切换全身/半身视角，便于实测 full 视角参数（camera pos / lookAt fallback）。
// vite import.meta.env.DEV 在 prod build 时为 false，整块会被 esbuild tree-shake 剥离 → 不进生产包。
// 注意：和父组件 props.view 同步是单向的（dev 切换不通知父），M1 后由 Settings UI 接管，此 dev hook 可删除。
if (import.meta.env.DEV) {
  let devView: AvatarView = props.view
  const onKeydown = (e: KeyboardEvent) => {
    if (e.key.toLowerCase() !== 'v') return
    devView = devView === 'half' ? 'full' : 'half'
    runtime.setView(devView)
    console.log(`[PetCanvas:dev] view toggled → ${devView}`)
  }
  window.addEventListener('keydown', onKeydown)
  onBeforeUnmount(() => window.removeEventListener('keydown', onKeydown))
}

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
  try {
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
  <div
    class="pet-stage"
    :class="{ 'pet-stage--draggable': draggable }"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerleave="onPointerLeave"
  >
    <canvas ref="canvasRef" class="pet-canvas" width="320" height="320"></canvas>
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
  width: 320px;
  height: 320px;
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
  width: 320px;
  height: 320px;
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
