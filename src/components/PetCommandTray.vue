<script setup lang="ts">
// PetCommandTray：宠物指令托盘。
//
// 设计要点（用户拍板）：
// - 胶囊 tray，纵向 pill list；不是系统 context menu
// - 固定锚于 pet-command overlay 窗内（overlay 由 Rust pet_overlay.rs 负责定位到 pet 右侧）
// - drill-down 二级（root ↔ settings），由父级受控（PetCommandOverlayApp 维护 view ref）
//   方便外层 Esc handler 决定"二级先返一级"还是"一级关闭"
// - inline 改名功能完全删除（改名走 workspace 设置）
// - 关闭路径全部 emit `close` 单点；外部 OverlayApp + pet 窗 App 负责跨窗 close intent 协议
//
// 2026-05-25 结构重构：
// - 删除 x / y / petSize props：overlay 模式下 Rust 端已定好窗口位置，tray 只需在窗内自然铺开
// - 删除 anchor computed：无双重定位
// - position: absolute + inset: 0 取代 position: fixed + top/left 算法

import { computed } from 'vue'
import { useToast } from '@/composables/useToast'
import { bosskeyToggle } from '@/services/bosskey'
import { showWorkspace } from '@/services/window'

interface Props {
  /** 受控 drill-down 视图（v-model:view）。OverlayApp 维护此 ref，让 Esc handler 能"二级先返一级"。 */
  view?: 'root' | 'settings'
}

const props = withDefaults(defineProps<Props>(), { view: 'root' })
const emit = defineEmits<{
  close: []
  'update:view': ['root' | 'settings']
}>()

const toast = useToast()

function close() {
  emit('close')
}

function setView(v: 'root' | 'settings') {
  emit('update:view', v)
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
</script>

<template>
  <div class="command-tray" role="menu" data-no-drag>
    <Transition name="drill" mode="out-in">
      <ul v-if="view === 'root'" key="root" class="command-tray__list">
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
          <button class="command-tray__pill" @click="setView('settings')">
            设置…
            <span class="command-tray__chevron">›</span>
          </button>
        </li>
      </ul>

      <ul v-else key="settings" class="command-tray__list">
        <li>
          <button class="command-tray__pill command-tray__pill--back" @click="setView('root')">
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
  /* 2026-05-26 紧凑化：原 160×220 与 pet 320×320 视觉竞争（50% × 69%），
     改 140×196（44% × 61%）让 tray 视觉从属 pet。
     overlay 模式下 Rust 端负责窗口定位，tray 只需在窗内自然铺满。
     absolute + inset: 5px 保留四边小间距给 box-shadow 呼吸空间。 */
  position: absolute;
  inset: 5px;
  display: flex;
  flex-direction: column;
  padding: 5px;
  background: var(--aipet-color-surface-raised, var(--aipet-color-surface));
  border: none;
  border-radius: 18px;
  box-shadow: 0 12px 32px -8px rgba(0, 0, 0, 0.22), 0 2px 6px -2px rgba(0, 0, 0, 0.1);
  backdrop-filter: blur(10px);
  z-index: 50;
  font-size: 12px;
  color: var(--aipet-color-text-1);
  pointer-events: auto;
  animation: tray-pop 140ms var(--aipet-ease-standard);
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
  gap: 5px;
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
  opacity: 0.5;
  background: transparent;
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
  color: var(--aipet-color-text-2);
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
