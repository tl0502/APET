<script setup lang="ts">
// ShortcutConfirmView：Onboarding Step 3 — 确认 chat 全局快捷键（flows §1.2）。
//
// 关键设计：
// 1. 默认显示后端 getChatShortcut() 返回值（启动期 register 成功的 current_chat）；
//    返回 null（启动期注册失败）时 fallback 字面 DEFAULT_SHORTCUT，与后端 const 对齐
// 2. 进入时自动 probe；如果探测的就是当前已注册的，后端 fast-path 会返 available=true,
//    不会假报占用（#21 C3a fix: probe_global_shortcut fast-path）
// 3. "改键"= 进入 capturing 模式,逐步输入：
//    - 只按 modifier（Ctrl/Alt/Shift/Meta）→ 实时显示 "Ctrl + Alt + …"，等用户加普通键
//    - 加按普通键 → finalize（要求至少 1 modifier + 1 普通键，避免日常打字误触）
//    - Esc 取消保留原值
//    - 不支持的键（符号、Backspace 等）→ 静默提示 hint,等用户重按
//   参考 TanStack useHotkeyRecorder / react-hotkeys-hook useRecordHotkeys / VSCode keybinding picker。
// 4. 占用 / 仍在捕获 / 探测中 → "用这个"按钮 disable
// 5. "用这个"= setShortcutChat（幂等，重写当前值也安全）→ emit('done')
// 6. 摸鱼快捷键 M2 上线后再加；本 view 只确认 chat 一键
//
// P1 美化（Vercel/Apple-Bear）:
// - ✓ / ⚠ 文本 glyph 换 ElIcon（CircleCheckFilled / WarningFilled）走 EP design system
// - 状态文本颜色仍用 success/danger token,保持语义清晰

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { ElButton, ElIcon } from 'element-plus'
import { CircleCheckFilled, WarningFilled } from '@element-plus/icons-vue'
import { getChatShortcut, probeGlobalShortcut, setShortcutChat } from '@/services/shortcut'
import type { ProbeResult } from '@/types/shortcut'
import { useToast } from '@/composables/useToast'

const emit = defineEmits<{ done: [] }>()
const toast = useToast()

/** 与后端 services/shortcuts.rs::DEFAULT_SHORTCUT_CHAT 对齐（fallback 用）。 */
const DEFAULT_SHORTCUT = 'Ctrl+Alt+Space'

type Mode = 'display' | 'capturing'

const mode = ref<Mode>('display')
const shortcut = ref<string>(DEFAULT_SHORTCUT)
const probeResult = ref<ProbeResult | null>(null)
const probing = ref(false)
const submitting = ref(false)
/** capturing 期间已按下的 modifier（实时显示）；非 capturing 期间永远为空。 */
const pendingMods = ref<string[]>([])
const captureHint = ref<string | null>(null)

onMounted(async () => {
  // 拉后端真实状态（current_chat_shortcut）；失败 fallback DEFAULT
  try {
    const current = await getChatShortcut()
    if (current) shortcut.value = current
  } catch (e) {
    console.warn('[ShortcutConfirmView] getChatShortcut failed:', e)
  }
  await probe(shortcut.value)
})

async function probe(s: string) {
  probing.value = true
  try {
    probeResult.value = await probeGlobalShortcut(s)
  } catch (e) {
    probeResult.value = {
      available: false,
      error: e instanceof Error ? e.message : String(e),
    }
  } finally {
    probing.value = false
  }
}

const MODIFIER_KEY_NAMES = new Set(['Control', 'Alt', 'Shift', 'Meta'])

/** 把 KeyboardEvent.key 翻译为 Tauri Shortcut friendly 字符串的非 modifier 部分。null = 不支持。 */
function formatKey(k: string): string | null {
  if (MODIFIER_KEY_NAMES.has(k)) return null
  if (k === ' ' || k === 'Spacebar') return 'Space'
  if (k === 'Enter' || k === 'Tab') return k
  if (k.startsWith('Arrow')) return k.slice(5) // ArrowUp → Up
  if (/^F([1-9]|1[0-2])$/.test(k)) return k
  if (/^[a-z]$/i.test(k)) return k.toUpperCase()
  if (/^[0-9]$/.test(k)) return k
  return null
}

function modsFromEvent(e: KeyboardEvent): string[] {
  const mods: string[] = []
  if (e.ctrlKey) mods.push('Ctrl')
  if (e.altKey) mods.push('Alt')
  if (e.shiftKey) mods.push('Shift')
  if (e.metaKey) mods.push('Meta')
  return mods
}

function onKeydown(e: KeyboardEvent) {
  e.preventDefault()
  e.stopPropagation()
  if (e.key === 'Escape') {
    stopCapture()
    return
  }
  // 1) modifier-only keydown：实时显示 pending 状态，等用户加普通键
  if (MODIFIER_KEY_NAMES.has(e.key)) {
    pendingMods.value = modsFromEvent(e)
    captureHint.value = null
    return
  }
  // 2) 普通键 keydown：尝试 finalize
  const mods = modsFromEvent(e)
  const key = formatKey(e.key)
  if (key === null) {
    captureHint.value = '只支持字母 / 数字 / 空格 / Enter / 方向键 / F 键(请重按)'
    return
  }
  if (mods.length === 0) {
    captureHint.value = '至少要 1 个修饰键(Ctrl/Alt/Shift/Meta)'
    return
  }
  // finalize：组合合法,退出捕获 + 自动 probe 新值
  captureHint.value = null
  pendingMods.value = []
  const combo = [...mods, key].join('+')
  shortcut.value = combo
  stopCapture()
  void probe(combo)
}

/** keyup：松开 modifier 时同步更新 pendingMods,让 UI 不停留在已松开的 modifier 上。 */
function onKeyup(e: KeyboardEvent) {
  if (mode.value !== 'capturing') return
  if (!MODIFIER_KEY_NAMES.has(e.key)) return
  // 重读当前帧的 modifier 状态（已扣除本次松开的那个）
  pendingMods.value = modsFromEvent(e)
}

function startCapture() {
  if (submitting.value) return
  mode.value = 'capturing'
  pendingMods.value = []
  captureHint.value = null
  document.addEventListener('keydown', onKeydown, { capture: true })
  document.addEventListener('keyup', onKeyup, { capture: true })
}

function stopCapture() {
  mode.value = 'display'
  pendingMods.value = []
  captureHint.value = null
  document.removeEventListener('keydown', onKeydown, { capture: true })
  document.removeEventListener('keyup', onKeyup, { capture: true })
}

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onKeydown, { capture: true })
  document.removeEventListener('keyup', onKeyup, { capture: true })
})

async function onConfirm() {
  if (submitting.value) return
  if (mode.value === 'capturing') return
  if (!probeResult.value?.available) return
  submitting.value = true
  try {
    // set 永远调（幂等，含 unregister 旧 + register 新 + 落 KV）；后端在新键 = 旧键时
    // 也安全（先 unregister 再 register 同 key，config.set 同值无副作用）
    await setShortcutChat(shortcut.value)
    emit('done')
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    console.error('[ShortcutConfirmView] setShortcutChat failed:', e)
    toast.error(`保存失败：${msg}`, { duration: 5000 })
    submitting.value = false
  }
}
</script>

<template>
  <section
    class="shortcut-confirm"
    role="dialog"
    aria-modal="true"
    aria-labelledby="shortcut-title"
  >
    <h1 id="shortcut-title" class="shortcut-confirm__title">用什么键把我叫出来?</h1>
    <p class="shortcut-confirm__hint">
      随时按这个组合,我就跳出来听你说。以后还能改。
    </p>

    <div class="shortcut-row">
      <div
        :class="['shortcut-box', { 'shortcut-box--capturing': mode === 'capturing' }]"
      >
        <span v-if="mode === 'display'" class="shortcut-box__combo">{{ shortcut }}</span>
        <span v-else-if="pendingMods.length > 0" class="shortcut-box__combo">
          {{ pendingMods.join('+') }}+<span class="shortcut-box__pending">…</span>
        </span>
        <span v-else class="shortcut-box__hint">
          按下组合键<kbd class="shortcut-box__kbd">Esc</kbd>取消
        </span>
      </div>
      <ElButton v-if="mode === 'display'" @click="startCapture">改键</ElButton>
      <ElButton v-else @click="stopCapture">取消</ElButton>
    </div>

    <p
      v-if="mode === 'display' && probeResult && !probing"
      :class="[
        'shortcut-status',
        probeResult.available ? 'shortcut-status--ok' : 'shortcut-status--bad',
      ]"
    >
      <template v-if="probeResult.available">
        <ElIcon class="shortcut-status__icon"><CircleCheckFilled /></ElIcon>
        可用
      </template>
      <template v-else>
        <ElIcon class="shortcut-status__icon"><WarningFilled /></ElIcon>
        已被其他应用占用,请改个组合
      </template>
    </p>
    <p v-else-if="mode === 'display' && probing" class="shortcut-status">检测中...</p>
    <p v-if="captureHint" class="shortcut-status shortcut-status--bad">{{ captureHint }}</p>

    <p class="shortcut-confirm__footer-hint">摸鱼模式快捷键(M2 上线后再设)。</p>

    <div class="shortcut-confirm__actions">
      <ElButton
        type="primary"
        :disabled="submitting || probing || mode === 'capturing' || !probeResult?.available"
        :loading="submitting"
        @click="onConfirm"
      >
        用这个
      </ElButton>
    </div>
  </section>
</template>

<style scoped>
.shortcut-confirm {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  width: 100%;
  height: 100%;
  padding: var(--aipet-space-6) var(--aipet-space-8) var(--aipet-space-8);
  background: var(--aipet-color-bg);
  box-sizing: border-box;
  user-select: none;
}

.shortcut-confirm__title {
  margin: 0 0 var(--aipet-space-2);
  font-size: var(--aipet-font-size-2xl);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  text-align: center;
  line-height: var(--aipet-line-height-display);
  letter-spacing: -0.01em;
}

.shortcut-confirm__hint {
  margin: 0 0 var(--aipet-space-6);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
  text-align: center;
}

.shortcut-row {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-3);
  margin-bottom: var(--aipet-space-2);
}

.shortcut-box {
  flex: 1 1 auto;
  padding: var(--aipet-space-3) var(--aipet-space-4);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-base);
  color: var(--aipet-color-text-1);
  transition: border-color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.shortcut-box--capturing {
  border-color: var(--aipet-color-primary);
  background: color-mix(in srgb, var(--aipet-color-primary) 10%, var(--aipet-color-surface));
  box-shadow: var(--aipet-ring-focus);
}

.shortcut-box__combo {
  letter-spacing: 0.5px;
}

.shortcut-box__pending {
  color: var(--aipet-color-text-3);
  font-style: italic;
}

.shortcut-box__hint {
  font-family: var(--aipet-font-family-base);
  color: var(--aipet-color-text-2);
}

.shortcut-box__kbd {
  display: inline-block;
  margin: 0 var(--aipet-space-1);
  padding: 0 var(--aipet-space-1);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-code-bg);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
}

.shortcut-status {
  display: flex;
  align-items: center;
  gap: var(--aipet-space-1);
  margin: 0 0 var(--aipet-space-4);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
}

.shortcut-status__icon {
  /* 跟随文本 currentColor;轻微 baseline 修正让圆形跟汉字下沿对齐 */
  font-size: 14px;
  vertical-align: middle;
}

.shortcut-status--ok {
  color: var(--aipet-color-success);
}

.shortcut-status--bad {
  color: var(--aipet-color-danger);
}

.shortcut-confirm__footer-hint {
  margin: var(--aipet-space-4) 0 0;
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  text-align: center;
}

.shortcut-confirm__actions {
  display: flex;
  justify-content: center;
  margin-top: auto;
}
</style>
