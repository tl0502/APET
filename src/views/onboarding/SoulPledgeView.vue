<script setup lang="ts">
// SoulPledgeView：灵魂宣誓页（issue #16，ADR-008 温暖叙述版 v1.0）。
//
// 目标：替代传统隐私同意页 —— 由默默 momo 第一人称叙述 5 段文案，"我懂了"等同写入
// consent.granted=true + method='soul_pledge' + version=CURRENT_CONSENT_VERSION。
//
// 关键设计（用户已拍板）：
// 1. 等 PetCanvas emit('loaded') 或 emit('error') 后再开播文案（VRM 加载期间空白页 → 不连贯）
// 2. 文案播放节奏 = 600ms 基础 + 60ms/字（自适应字数，比固定 1.2s/段更自然）
// 3. 中途点击 stage → 清所有 timer + 立即全显 + 启用按钮
// 4. 默认 focus 在"再看一眼条款"（非"我懂了"，避免误回车）
// 5. "退出"路径 → getCurrentWindow().close() → Rust on_window_event 走 app.exit(0)
// 6. Alt+F4 等同退出（同上路径，统一）
// 7. 不做 reconsent UI 分支（v2 上线时再扩；M3 范围）
//
// dev mode 入口：浏览器直访 http://localhost:1420/onboarding.html
//   IPC 调用（grantConsentSafe / invoke onboarding_complete / window.close）在浏览器无 Tauri
//   上下文会抛 → 全部 try/catch + console.warn，不阻断界面验收。
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { ElButton } from 'element-plus'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { marked } from 'marked'
import PetCanvas from '@/components/PetCanvas.vue'
import StandardDialog from '@/components/feedback/StandardDialog.vue'
import { useToast } from '@/composables/useToast'
import { grantConsentSafe } from '@/services/consent'
// Vite ?raw 导入：构建期把文件内容作为字符串嵌入 chunk（避免 fetch resource 引入
// 运行时复杂度 + tauri.conf bundle.resources 路径解析）。tauri.conf 仍保留
// bundle.resources 配置是为了 #17 状态机将来需要 fs.readResource 时的路径占位。
import soulPledgeText from '../../../src-tauri/assets/onboarding/soul_pledge_v1.txt?raw'
import dataPolicyMarkdown from '../../../src-tauri/assets/legal/data_policy_v1.md?raw'

const toast = useToast()

// === 文案分段 ===
// soul_pledge_v1.txt 用空行分段；trim 防尾随换行进 paragraphs。
const paragraphs = soulPledgeText
  .split(/\n\s*\n/)
  .map((p) => p.trim())
  .filter((p) => p.length > 0)

// === 数据策略 markdown → HTML ===
// 后处理：把所有 <a href="...">text</a> 替换为纯文字 + 保留 title 显示链接
// （issue body "再看一眼条款"明确"不开外链"；用户能看到 URL 文字但点击无效）。
const dataPolicyHtml = (() => {
  const raw = marked.parse(dataPolicyMarkdown) as string
  return raw.replace(
    /<a\s+([^>]*?)href="([^"]*?)"([^>]*?)>(.*?)<\/a>/g,
    (_m, _pre, href, _post, text) => `<span class="onboarding-link-disabled" title="${href}">${text}</span>`,
  )
})()

// === 播放状态 ===
/** 已渲染段数（0 → paragraphs.length）。 */
const visibleCount = ref(0)
/** PetCanvas 加载完成（VRM 渲染成功或失败都视为"可开播"）。 */
const stageReady = ref(false)
/** "我懂了"按钮 loading（防双击 + 等 IPC 完成）。 */
const agreeing = ref(false)
/** "再看一眼条款" 弹窗 v-model。 */
const showPolicy = ref(false)

const isPlaying = computed(() => visibleCount.value < paragraphs.length)
const buttonsEnabled = computed(() => !isPlaying.value && !agreeing.value)

const timers: number[] = []

function startPlaying() {
  // 累加 setTimeout：第 i 段在 sum(prev delays) 后出现；不等前段播完再启下段，
  // 视觉上是"持续涌现"而非"一段一停"，节奏更顺。
  let cumulative = 0
  for (let i = 0; i < paragraphs.length; i++) {
    const chars = paragraphs[i].length
    const delay = 600 + chars * 60
    const fireAt = cumulative
    const handle = window.setTimeout(() => {
      visibleCount.value = i + 1
    }, fireAt)
    timers.push(handle)
    cumulative += delay
  }
}

function skipToEnd() {
  if (!isPlaying.value) return
  timers.forEach((t) => clearTimeout(t))
  timers.length = 0
  visibleCount.value = paragraphs.length
}

function onStageClick() {
  // 点击 stage 任意处 = 跳过播放。按钮区也会冒泡触发本 handler，但 skipToEnd 在 !isPlaying
  // 时 return early（幂等），所以按钮 + skip 双触发无副作用。
  // 这正是设计意图：用户在文案播放中点 disabled 的"我懂了"区域时，click 冒泡到 stage 触发
  // skip → 按钮立即启用 → 用户再点一次才真正 grant（两段 UX 拆开比一气呵成更谨慎）。
  skipToEnd()
}

// === VRM 加载完成（或失败）后开播 ===
function onCanvasLoaded() {
  if (stageReady.value) return
  stageReady.value = true
  startPlaying()
}

function onCanvasError(message: string) {
  console.warn('[SoulPledgeView] VRM error, starting playback anyway:', message)
  if (stageReady.value) return
  stageReady.value = true
  startPlaying()
}

// === 默认 focus：按钮启用后 focus 到"再看一眼条款" ===
// 用户拍板：默认 focus 在最慎重选项，避免误回车 grant consent。
// 用 unwatch 让 focus 仅在"首次启用"时跑：避免 grant 失败 → agreeing 翻 false → buttonsEnabled
// 翻 true → 焦点又飞回"再看一眼条款"扰乱用户视觉。
const policyBtnRef = ref<InstanceType<typeof ElButton> | null>(null)

const unwatchFocus = watch(buttonsEnabled, async (enabled) => {
  if (!enabled) return
  unwatchFocus()
  // 等下一帧让 disabled 真正翻 false 后再 focus（disabled 状态下 focus() 无效）
  await new Promise((resolve) => requestAnimationFrame(resolve))
  const el = (policyBtnRef.value as unknown as { $el?: HTMLElement })?.$el
  if (el && typeof el.focus === 'function') el.focus()
})

// === 按钮事件 ===

async function onAgree() {
  if (!buttonsEnabled.value) return
  agreeing.value = true

  // 分两段 try：grant（写 DB）与 complete（切窗）失败语义不同。
  // - grant 失败：DB 没写 → 用户可重试 → 复位 agreeing 让按钮启用
  // - grant 成功 + complete 失败：DB 已写（consent.granted=1），但窗口没切 → toast 改"已保存
  //   但切窗失败"，避免"同意写入失败"误导（grant 是 UPDATE id=1 idempotent，复位让用户能再
  //   点"我懂了"重试 invoke('onboarding_complete')；重复 grant 也是安全 no-op）
  try {
    // grantConsentSafe 内部先 fetch CURRENT_CONSENT_VERSION 再 grant（防硬编码 stale）
    await grantConsentSafe('soul_pledge')
  } catch (e) {
    console.error('[SoulPledgeView] grant failed:', e)
    toast.error(`同意写入失败：${e instanceof Error ? e.message : String(e)}`, { duration: 5000 })
    agreeing.value = false
    return
  }

  try {
    // onboarding_complete：后端统一切窗 + emit step-done；成功后本视图被 hide
    await invoke('onboarding_complete')
  } catch (e) {
    console.error('[SoulPledgeView] complete failed (consent already saved):', e)
    toast.warn('同意已保存，但窗口切换失败，请重试或重启应用。', { duration: 8000 })
    agreeing.value = false
  }
}

function onShowPolicy() {
  if (!buttonsEnabled.value) return
  showPolicy.value = true
}

async function onExit() {
  if (!buttonsEnabled.value) return
  // close → Rust on_window_event CloseRequested(onboarding) → app.exit(0)（不写 consent）
  // 浏览器 dev mode 下 getCurrentWindow() 抛 → 退化为 console.warn
  try {
    await getCurrentWindow().close()
  } catch (e) {
    console.warn('[SoulPledgeView] window.close failed (likely browser dev mode):', e)
  }
}

// === ESC 键 = 退出（a11y 键盘可达） ===
// 用 capture 阶段在 document 上监听：bubble 顺序是 target → body → window，EP ElDialog 的
// ESC handler 在 body 上响应（早于 window），会先把 showPolicy 改 false，导致 window 上的
// 守护 `if (showPolicy.value) return` 看到的是已变更后的 false → 触发 onExit。
// capture 阶段从 document 往下，本 handler 在 EP 之前看到 showPolicy 仍是 true，能正确守护。
function onKeydown(e: KeyboardEvent) {
  // 文案播放期不响应 ESC（按钮还 disabled，避免误触退出）；可以播放完再用
  if (!buttonsEnabled.value) return
  // 弹窗打开时让 StandardDialog 自己处理 ESC（capture 阶段时 showPolicy 仍是用户当前看到的值）
  if (showPolicy.value) return
  if (e.key === 'Escape') {
    void onExit()
  }
}

document.addEventListener('keydown', onKeydown, { capture: true })

onBeforeUnmount(() => {
  document.removeEventListener('keydown', onKeydown, { capture: true })
  timers.forEach((t) => clearTimeout(t))
})
</script>

<template>
  <section
    class="soul-pledge"
    role="dialog"
    aria-modal="true"
    aria-labelledby="soul-pledge-title"
    @click="onStageClick"
  >
    <h1 id="soul-pledge-title" class="visually-hidden">灵魂宣誓 — momo 的承诺</h1>

    <div class="soul-pledge__avatar">
      <PetCanvas :draggable="false" @loaded="onCanvasLoaded" @error="onCanvasError" />
    </div>

    <div class="soul-pledge__content" aria-live="polite">
      <p
        v-for="(p, i) in paragraphs"
        :key="i"
        class="soul-pledge__paragraph"
        :class="{ 'is-visible': i < visibleCount }"
        :aria-hidden="i >= visibleCount"
      >
        {{ p }}
      </p>
      <button
        v-show="!isPlaying"
        type="button"
        class="soul-pledge__policy-link"
        :disabled="!buttonsEnabled"
        @click.stop="onShowPolicy"
      >
        — 想了解技术细节，看看完整数据策略 —
      </button>
    </div>

    <div class="soul-pledge__actions" role="group" aria-label="操作">
      <ElButton
        ref="policyBtnRef"
        :disabled="!buttonsEnabled"
        @click="onShowPolicy"
      >
        再看一眼条款
      </ElButton>
      <ElButton :disabled="!buttonsEnabled" @click="onExit">退出</ElButton>
      <ElButton
        type="primary"
        :disabled="!buttonsEnabled"
        :loading="agreeing"
        @click="onAgree"
      >
        我懂了，一起开始
      </ElButton>
    </div>

    <StandardDialog
      v-model="showPolicy"
      title="完整数据策略 v1.0"
      :width="560"
    >
      <!-- dataPolicyHtml 来自构建期嵌入的本地 data_policy_v1.md（?raw + marked），非用户输入；
           外链已替换为 .onboarding-link-disabled span，无 XSS 风险 -->
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div class="soul-pledge__policy" v-html="dataPolicyHtml"></div>
      <template #footer>
        <ElButton type="primary" @click="showPolicy = false">我读完了</ElButton>
      </template>
    </StandardDialog>
  </section>
</template>

<style scoped>
.soul-pledge {
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 100%;
  height: 100%;
  padding: var(--aipet-space-6) var(--aipet-space-8) var(--aipet-space-8);
  background: var(--aipet-color-bg);
  box-sizing: border-box;
  user-select: none;
  /* 鼠标 cursor 提示"可点击全显" */
  cursor: pointer;
}

.soul-pledge__avatar {
  /* PetCanvas 320×320 居中显示在顶部 */
  flex: 0 0 auto;
  margin-bottom: var(--aipet-space-2);
}

.soul-pledge__content {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: stretch;
  width: 100%;
  max-width: 400px;
  /* 文案区是阅读区，cursor 恢复 default（防 stage 全 pointer 让人误以为不能选） */
  cursor: default;
}

.soul-pledge__paragraph {
  margin: 0 0 var(--aipet-space-3);
  font-size: var(--aipet-font-size-base);
  line-height: var(--aipet-line-height-base);
  color: var(--aipet-color-text-1);
  text-align: center;
  /* 渐进出现动画 */
  opacity: 0;
  transform: translateY(8px);
  transition:
    opacity var(--aipet-duration-base) var(--aipet-ease-standard),
    transform var(--aipet-duration-base) var(--aipet-ease-standard);
}

.soul-pledge__paragraph.is-visible {
  opacity: 1;
  transform: translateY(0);
}

.soul-pledge__policy-link {
  margin-top: var(--aipet-space-3);
  padding: var(--aipet-space-2);
  border: none;
  background: transparent;
  color: var(--aipet-color-text-3);
  font: inherit;
  font-size: var(--aipet-font-size-sm);
  text-align: center;
  cursor: pointer;
  transition: color var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.soul-pledge__policy-link:hover,
.soul-pledge__policy-link:focus-visible {
  color: var(--aipet-color-primary);
  outline: none;
}

.soul-pledge__policy-link:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.soul-pledge__actions {
  display: flex;
  gap: var(--aipet-space-3);
  margin-top: var(--aipet-space-4);
  cursor: default;
}

.soul-pledge__policy {
  max-height: 50vh;
  overflow-y: auto;
  padding-right: var(--aipet-space-2);
  font-size: var(--aipet-font-size-sm);
  line-height: var(--aipet-line-height-base);
  color: var(--aipet-color-text-1);
}

.soul-pledge__policy :deep(h1),
.soul-pledge__policy :deep(h2) {
  margin: var(--aipet-space-4) 0 var(--aipet-space-2);
  color: var(--aipet-color-text-1);
}

.soul-pledge__policy :deep(h1) {
  font-size: var(--aipet-font-size-xl);
}

.soul-pledge__policy :deep(h2) {
  font-size: var(--aipet-font-size-lg);
}

.soul-pledge__policy :deep(p),
.soul-pledge__policy :deep(li) {
  margin: 0 0 var(--aipet-space-2);
}

.soul-pledge__policy :deep(table) {
  width: 100%;
  border-collapse: collapse;
  margin: var(--aipet-space-3) 0;
}

.soul-pledge__policy :deep(th),
.soul-pledge__policy :deep(td) {
  padding: var(--aipet-space-2);
  border: 1px solid var(--aipet-color-border);
  text-align: left;
}

.soul-pledge__policy :deep(th) {
  background: var(--aipet-color-surface);
}

.soul-pledge__policy :deep(code) {
  padding: 1px var(--aipet-space-1);
  background: var(--aipet-color-code-bg);
  border-radius: var(--aipet-radius-sm);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
}

.soul-pledge__policy :deep(.onboarding-link-disabled) {
  color: var(--aipet-color-text-3);
  text-decoration: underline dotted;
  cursor: not-allowed;
}

/* a11y：屏幕阅读器可见的 title（视觉隐藏） */
.visually-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
</style>
