<script setup lang="ts">
// PetCommandTray (取代 PetContextMenu)：宠物指令托盘。
//
// 设计要点（用户拍板）：
// - 胶囊 tray，纵向 pill list；不是系统 context menu
// - 锚在 pet 窗"中下"区域，优先右中下，右侧空间不足时 fallback 左中下
// - drill-down 二级（root ↔ settings），替换内容不展开 hover submenu
// - inline 改名功能完全删除（改名走 workspace 设置）
// - 关闭路径：点击外部 / Esc / 完成 quiet 操作 / 跳走 workspace
//
// 与 PetReminderBubble 的避让在 App.vue 内实现（commandTrayOpen 状态提升）。

import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useToast } from '@/composables/useToast'
import { bosskeyToggle } from '@/services/bosskey'
import { showWorkspace } from '@/services/window'

interface Props {
  /** webview 视口坐标（来自 contextmenu event）—— fallback 用，tray 实际锚点按 petSize 算 */
  x: number
  y: number
  /** pet 窗当前逻辑尺寸（half 320×320 / full 320×512）；App.vue 透传 */
  petSize: { width: number; height: number }
}

const props = defineProps<Props>()
const emit = defineEmits<{ close: [] }>()

const toast = useToast()

type View = 'root' | 'settings'
const currentView = ref<View>('root')
const trayRef = ref<HTMLDivElement | null>(null)

const TRAY_W = 140
const TRAY_PAD = 8
const PET_BOTTOM_RATIO = 0.55

/** 锚点：优先 pet 中心右侧；右侧空间不足切左侧；两侧都不够靠左边缘。 */
const anchor = computed(() => {
  const top = props.petSize.height * PET_BOTTOM_RATIO
  const centerX = props.petSize.width / 2
  let left = centerX + TRAY_PAD
  if (left + TRAY_W + TRAY_PAD > props.petSize.width) {
    left = centerX - TRAY_PAD - TRAY_W
  }
  if (left < TRAY_PAD) left = TRAY_PAD
  return { top, left }
})

function close() {
  emit('close')
}

async function onQuiet() {
  try {
    const hidden = await bosskeyToggle()
    if (hidden) {
      toast.info('已静音，按 Ctrl+Shift+B 恢复显示。', { duration: 3000 })
    } else {
      toast.info('已恢复显示。', { duration: 2000 })
    }
  } catch (e) {
    console.error('[command-tray] bosskeyToggle failed:', e)
    toast.error(`「静一会儿」失败：${e instanceof Error ? e.message : String(e)}`)
  } finally {
    close()
  }
}

async function onOpenWorkspace(hint: string) {
  try {
    await showWorkspace()
    toast.info(hint, { duration: 2500 })
  } catch (e) {
    console.error('[command-tray] showWorkspace failed:', e)
    toast.error('打开工作台失败，请稍后再试。')
  } finally {
    close()
  }
}

function onDocOrEscapeOrPointer(e: KeyboardEvent | PointerEvent) {
  if (e instanceof KeyboardEvent) {
    if (e.key === 'Escape') {
      if (currentView.value === 'settings') {
        currentView.value = 'root'
        return
      }
      close()
    }
    return
  }
  const path = (e.composedPath?.() ?? []) as EventTarget[]
  const tray = trayRef.value
  if (tray && path.includes(tray)) return
  close()
}

onMounted(() => {
  document.addEventListener('keydown', onDocOrEscapeOrPointer, true)
  document.addEventListener('pointerdown', onDocOrEscapeOrPointer)
})

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onDocOrEscapeOrPointer, true)
  document.removeEventListener('pointerdown', onDocOrEscapeOrPointer)
})
</script>

<template>
  <div
    ref="trayRef"
    class="command-tray"
    :style="{ top: `${anchor.top}px`, left: `${anchor.left}px`, width: `${TRAY_W}px` }"
    role="menu"
    data-no-drag
  >
    <Transition name="drill" mode="out-in">
      <ul v-if="currentView === 'root'" key="root" class="command-tray__list">
        <li>
          <button
            class="command-tray__pill command-tray__pill--disabled"
            disabled
            title="M5 小游戏舱上线后启用"
          >
            和我玩…<span class="command-tray__tag">M5</span>
          </button>
        </li>
        <li>
          <button
            class="command-tray__pill command-tray__pill--disabled"
            disabled
            title="M4 装扮工坊上线后启用"
          >
            换装…<span class="command-tray__tag">M4</span>
          </button>
        </li>
        <li>
          <button class="command-tray__pill" @click="onQuiet">静一会儿</button>
        </li>
        <li>
          <button class="command-tray__pill" @click="currentView = 'settings'">
            设置…
            <span class="command-tray__chevron">›</span>
          </button>
        </li>
      </ul>

      <ul v-else key="settings" class="command-tray__list">
        <li>
          <button class="command-tray__pill command-tray__pill--back" @click="currentView = 'root'">
            ← 返回
          </button>
        </li>
        <li>
          <button
            class="command-tray__pill command-tray__pill--disabled"
            disabled
            title="声音设置 M3 接入"
          >
            声音<span class="command-tray__tag">M3</span>
          </button>
        </li>
        <li>
          <button class="command-tray__pill" @click="onOpenWorkspace('在「设置 → 桌宠」里调整行为')">
            行为
          </button>
        </li>
        <li>
          <button class="command-tray__pill" @click="onOpenWorkspace('在「设置 → 外观」里切换主题/视角')">
            外观
          </button>
        </li>
        <li>
          <button class="command-tray__pill" @click="onOpenWorkspace('已打开工作台')">
            高级设置
          </button>
        </li>
      </ul>
    </Transition>
  </div>
</template>

<style scoped>
.command-tray {
  position: fixed;
  display: flex;
  flex-direction: column;
  padding: 6px;
  background: var(--aipet-color-surface-raised, var(--aipet-color-surface));
  border: 1px solid var(--aipet-color-border-strong, var(--aipet-color-border));
  border-radius: 18px;
  box-shadow: 0 12px 32px -8px rgba(0, 0, 0, 0.22), 0 2px 6px -2px rgba(0, 0, 0, 0.1);
  backdrop-filter: blur(10px);
  z-index: 50;
  font-size: 12px;
  color: var(--aipet-color-text-1);
  pointer-events: auto;
  animation: tray-pop 140ms var(--aipet-ease-standard);
  max-height: calc(100% - 16px);
  overflow-y: auto;
}

@keyframes tray-pop {
  from {
    opacity: 0;
    transform: scale(0.92);
  }
  to {
    opacity: 1;
    transform: scale(1);
  }
}

.command-tray__list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.command-tray__pill {
  appearance: none;
  -webkit-appearance: none;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  padding: 7px 12px;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 999px;
  color: var(--aipet-color-text-1);
  font: inherit;
  font-size: 12px;
  text-align: left;
  cursor: pointer;
  transition: background-color 120ms var(--aipet-ease-standard),
    border-color 120ms var(--aipet-ease-standard);
}

.command-tray__pill:hover:not(:disabled) {
  background: color-mix(in srgb, var(--aipet-color-primary) 8%, transparent);
  border-color: color-mix(in srgb, var(--aipet-color-primary) 24%, transparent);
}

.command-tray__pill:active:not(:disabled) {
  background: color-mix(in srgb, var(--aipet-color-primary) 14%, transparent);
}

.command-tray__pill--disabled {
  color: var(--aipet-color-text-3);
  cursor: not-allowed;
}

.command-tray__pill--back {
  color: var(--aipet-color-text-2);
}

.command-tray__tag {
  font-size: 10px;
  font-weight: 600;
  color: var(--aipet-color-text-3);
  background: var(--aipet-color-surface);
  border-radius: 999px;
  padding: 1px 6px;
}

.command-tray__chevron {
  font-size: 14px;
  color: var(--aipet-color-text-3);
  line-height: 1;
}

/* drill-down 切换：translateX 12px 同时 fade 100ms */
.drill-enter-active,
.drill-leave-active {
  transition: opacity 100ms var(--aipet-ease-standard),
    transform 100ms var(--aipet-ease-standard);
}

.drill-enter-from {
  opacity: 0;
  transform: translateX(12px);
}

.drill-leave-to {
  opacity: 0;
  transform: translateX(-12px);
}
</style>
