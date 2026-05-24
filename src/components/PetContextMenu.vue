<script setup lang="ts">
// PetContextMenu：桌宠右键自绘浮层菜单（#40，模块 N 主干）。
//
// 设计要点（ADR-025 + issue #40 拍板）：
// - 5 项：叫它... / 换装...（M4 灰显） / 和我玩...（M5 灰显） / 静一会儿（#42 接口） / 设置...
// - 自绘 Vue 浮层（不用 Tauri Menu API）—— 与桌宠位置 anchor，便于 M3+ 加表情 / 头像
// - 「叫它...」复用 NicknameService —— 点击后菜单本体切换为 input 模式（不开新对话框），
//   确认调 nickname_set_user；取消回菜单态。
// - 「静一会儿」5min BossKey：#42 落地前 stub 一个 toast 提示，留 follow-up。
// - 「设置...」调 showWorkspace（workspaceLayout.category 由用户在 workspace 内自选）。
//
// 关闭路径：点击外部 / Esc / 完成 nickname / 跳走 workspace 后 emit close。
//
// 不使用 Tauri Menu 的理由：
// - 自由度高（表情图标 / hover preview / inline input 都可以塞）
// - 跨平台一致渲染（不需要 trust OS menu 样式 fit 桌宠风格）
// - 桌宠位置 anchor 灵活（OS menu 是屏幕坐标，需要换算）

import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { useToast } from '@/composables/useToast'
import { bosskeyToggle } from '@/services/bosskey'
import { setUserNickname } from '@/services/nickname'
import { showWorkspace } from '@/services/window'

interface Props {
  /** webview 视口坐标。父组件传从 contextmenu event 的 clientX/Y。 */
  x: number
  y: number
}

const props = defineProps<Props>()
const emit = defineEmits<{ close: [] }>()

const toast = useToast()

const menuRef = ref<HTMLDivElement | null>(null)
const nicknameMode = ref(false)
const nicknameInput = ref('')
const nicknameBusy = ref(false)

/** 视口边界保护：菜单跑出 webview 时夹回（透明 320×320 / 320×512 角色窗内空间紧）。 */
const computedPos = computed(() => {
  const MARGIN = 4
  const w = 160
  const h = 200
  const vw = window.innerWidth
  const vh = window.innerHeight
  let x = props.x
  let y = props.y
  if (x + w + MARGIN > vw) x = Math.max(MARGIN, vw - w - MARGIN)
  if (y + h + MARGIN > vh) y = Math.max(MARGIN, vh - h - MARGIN)
  return { x, y }
})

function close() {
  emit('close')
}

function onNicknameStart() {
  nicknameMode.value = true
  void nextTick(() => {
    const el = menuRef.value?.querySelector<HTMLInputElement>('input.nickname-input')
    el?.focus()
    el?.select()
  })
}

async function onNicknameConfirm() {
  const name = nicknameInput.value.trim()
  if (!name) {
    onNicknameCancel()
    return
  }
  nicknameBusy.value = true
  try {
    await setUserNickname(name)
    toast.success(`好的，记住了：${name}`, { duration: 2000 })
    close()
  } catch (e) {
    console.error('[ctx-menu] setUserNickname failed:', e)
    toast.error('改名失败，请稍后再试。')
  } finally {
    nicknameBusy.value = false
  }
}

function onNicknameCancel() {
  nicknameMode.value = false
  nicknameInput.value = ''
}

async function onQuiet() {
  // #41 接入：调 bosskey_toggle 隐藏 4 窗（pet/chat/workspace/pomodoro），用户按
  // Ctrl+Shift+B 恢复（与 #42 boss key 快捷键复用）。
  // M3 follow-up：bosskey_toggle 增 ttl_ms 参数实现"5min 自动恢复"，本期不实现以免改动 #42 已合 service。
  try {
    const hidden = await bosskeyToggle()
    if (hidden) {
      toast.info('已静音，按 Ctrl+Shift+B 恢复显示。', { duration: 3000 })
    } else {
      // 罕见路径：用户已经在隐藏态时点了"静一会儿" → toggle 反向 = 显示出来
      toast.info('已恢复显示。', { duration: 2000 })
    }
  } catch (e) {
    console.error('[ctx-menu] bosskeyToggle failed:', e)
    toast.error(`「静一会儿」失败：${e instanceof Error ? e.message : String(e)}`)
  } finally {
    close()
  }
}

async function onSettings() {
  try {
    await showWorkspace()
  } catch (e) {
    console.error('[ctx-menu] showWorkspace failed:', e)
    toast.error('打开工作台失败，请稍后再试。')
  } finally {
    close()
  }
}

function onDocOrEscapeOrPointer(e: KeyboardEvent | PointerEvent) {
  if (e instanceof KeyboardEvent) {
    if (e.key === 'Escape') {
      if (nicknameMode.value) {
        onNicknameCancel()
        return
      }
      close()
    }
    return
  }
  // pointerdown 外部 → 关菜单。同 webview 内的点击会 bubble；用 menuRef 边界过滤。
  const path = (e.composedPath?.() ?? []) as EventTarget[]
  const menu = menuRef.value
  if (menu && path.includes(menu)) return
  close()
}

onMounted(() => {
  // capture phase 听 keydown 防止其他 listener 抢 Esc。
  document.addEventListener('keydown', onDocOrEscapeOrPointer, true)
  // pointerdown 走 bubble，让菜单内点击 menuRef 边界判断后短路。
  document.addEventListener('pointerdown', onDocOrEscapeOrPointer)
})

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onDocOrEscapeOrPointer, true)
  document.removeEventListener('pointerdown', onDocOrEscapeOrPointer)
})
</script>

<template>
  <!-- data-no-drag：菜单按钮区不触发桌宠拖动（与 PetReminderBubble 同款隔离）。 -->
  <div
    ref="menuRef"
    class="ctx-menu"
    :style="{ left: `${computedPos.x}px`, top: `${computedPos.y}px` }"
    role="menu"
    data-no-drag
  >
    <template v-if="!nicknameMode">
      <button class="ctx-menu__item" role="menuitem" @click="onNicknameStart">
        叫它…
      </button>
      <button
        class="ctx-menu__item ctx-menu__item--disabled"
        role="menuitem"
        disabled
        title="M4 装扮工坊上线后启用"
      >
        换装…<span class="ctx-menu__tag">M4</span>
      </button>
      <button
        class="ctx-menu__item ctx-menu__item--disabled"
        role="menuitem"
        disabled
        title="M5 小游戏舱上线后启用"
      >
        和我玩…<span class="ctx-menu__tag">M5</span>
      </button>
      <div class="ctx-menu__sep" />
      <button class="ctx-menu__item" role="menuitem" @click="onQuiet">
        静一会儿
      </button>
      <button class="ctx-menu__item" role="menuitem" @click="onSettings">
        设置…
      </button>
    </template>

    <template v-else>
      <div class="ctx-menu__label">叫它什么？</div>
      <input
        v-model="nicknameInput"
        class="nickname-input"
        type="text"
        maxlength="32"
        placeholder="比如：默默"
        :disabled="nicknameBusy"
        @keydown.enter.prevent="onNicknameConfirm"
        @keydown.esc.prevent="onNicknameCancel"
      />
      <div class="ctx-menu__row">
        <button
          class="ctx-menu__btn ctx-menu__btn--primary"
          :disabled="nicknameBusy || !nicknameInput.trim()"
          @click="onNicknameConfirm"
        >
          确认
        </button>
        <button class="ctx-menu__btn" :disabled="nicknameBusy" @click="onNicknameCancel">
          取消
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.ctx-menu {
  position: fixed;
  /* anchor 由 :style 注入 */
  min-width: 160px;
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 1px;
  background: var(--aipet-color-surface-raised, var(--aipet-color-surface));
  border: 1px solid var(--aipet-color-border-strong, var(--aipet-color-border));
  border-radius: 10px;
  box-shadow: 0 12px 32px -8px rgba(0, 0, 0, 0.22), 0 2px 6px -2px rgba(0, 0, 0, 0.1);
  backdrop-filter: blur(10px);
  z-index: 50;
  font-size: 13px;
  color: var(--aipet-color-text-1);
  /* 浮层接收 pointer，桌宠 cursor:grab 不应穿透到菜单按钮 */
  pointer-events: auto;
}

.ctx-menu__item {
  appearance: none;
  -webkit-appearance: none;
  border: none;
  background: transparent;
  color: inherit;
  text-align: left;
  font: inherit;
  padding: 6px 10px;
  border-radius: 6px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  transition: background-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.ctx-menu__item:hover:not(:disabled) {
  background: var(--aipet-color-surface);
}

.ctx-menu__item--disabled {
  color: var(--aipet-color-text-3);
  cursor: not-allowed;
}

.ctx-menu__tag {
  font-size: 10px;
  font-weight: 600;
  color: var(--aipet-color-text-3);
  background: var(--aipet-color-surface);
  border-radius: 4px;
  padding: 1px 5px;
}

.ctx-menu__sep {
  height: 1px;
  background: var(--aipet-color-border);
  margin: 4px 2px;
}

.ctx-menu__label {
  font-size: 12px;
  color: var(--aipet-color-text-2);
  padding: 4px 8px 2px;
}

.nickname-input {
  appearance: none;
  -webkit-appearance: none;
  margin: 0 4px;
  padding: 6px 8px;
  border: 1px solid var(--aipet-color-border);
  border-radius: 6px;
  background: var(--aipet-color-bg);
  color: var(--aipet-color-text-1);
  font: inherit;
  font-size: 13px;
  outline: none;
  transition: border-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.nickname-input:focus {
  border-color: var(--aipet-color-primary);
}

.ctx-menu__row {
  display: flex;
  gap: 6px;
  padding: 6px 4px 2px;
  justify-content: flex-end;
}

.ctx-menu__btn {
  appearance: none;
  -webkit-appearance: none;
  border: 1px solid var(--aipet-color-border);
  background: var(--aipet-color-bg);
  color: var(--aipet-color-text-2);
  font: inherit;
  font-size: 12px;
  padding: 4px 10px;
  border-radius: 6px;
  cursor: pointer;
}

.ctx-menu__btn:hover:not(:disabled) {
  border-color: var(--aipet-color-border-strong, var(--aipet-color-border));
  color: var(--aipet-color-text-1);
  background: var(--aipet-color-surface);
}

.ctx-menu__btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.ctx-menu__btn--primary {
  background: var(--aipet-color-primary);
  border-color: var(--aipet-color-primary);
  color: #fff;
}

.ctx-menu__btn--primary:hover:not(:disabled) {
  background: color-mix(in srgb, var(--aipet-color-primary) 88%, #000);
  border-color: color-mix(in srgb, var(--aipet-color-primary) 88%, #000);
  color: #fff;
}
</style>
