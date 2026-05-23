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
// #21 step router 改造：grant consent 成功后 emit('done')，由父 OnboardingApp 切到下一 step
// （之前是直接调 invoke('onboarding_complete') 切窗 + emit step-done，那时只有 1 步）。
// 切窗 IPC 的调用时机挪到 OnboardingApp 的最后一步完成时。
//
// dev mode 入口：浏览器直访 http://localhost:1420/onboarding.html
//   IPC 调用（grantConsentSafe / window.close）在浏览器无 Tauri
//   上下文会抛 → 全部 try/catch + console.warn，不阻断界面验收。
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { ElButton } from 'element-plus'
import { getCurrentWindow } from '@tauri-apps/api/window'
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

const emit = defineEmits<{ done: [] }>()

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

  // grantConsentSafe 内部先 fetch CURRENT_CONSENT_VERSION 再 grant（防硬编码 stale）
  // 失败 → DB 没写 → 复位 agreeing 让用户能重试
  try {
    await grantConsentSafe('soul_pledge')
  } catch (e) {
    console.error('[SoulPledgeView] grant failed:', e)
    toast.error(`同意写入失败：${e instanceof Error ? e.message : String(e)}`, { duration: 5000 })
    agreeing.value = false
    return
  }

  // grant 成功 → 通知父级 OnboardingApp 切到 Step 2（PersonaPicker）。
  // 切窗 IPC（onboarding_complete）由 OnboardingApp 在最后一步完成时调用，
  // 不在本视图，避免本视图同时负责"step 1 业务"+"整个 onboarding 切窗"两种角色。
  // 不复位 agreeing：父级切走后本组件会被销毁，复位反而短暂闪烁。
  emit('done')
}

function onShowPolicy() {
  if (!buttonsEnabled.value) return
  showPolicy.value = true
}

// 弹窗每次打开重置滚动到顶：EP ElDialog 默认 destroy-on-close=false，DOM 持久化，
// 上次滚动位置会残留。watch false→true 时 nextTick 后把 scrollable 容器 scrollTop 归零。
const policyScrollRef = ref<HTMLDivElement | null>(null)

watch(showPolicy, async (open) => {
  if (!open) return
  // nextTick 等 ElDialog v-show=true 切到 DOM 可见；再加一次 RAF 等浏览器布局应用，
  // 否则 scrollTop=0 可能在 transition 完成前被 EP 覆盖（实测过 EP 弹窗 transition 期间
  // scrollTop 偶发被 reset 到上次值的 bug）。
  await nextTick()
  await new Promise((r) => requestAnimationFrame(r))
  if (policyScrollRef.value) policyScrollRef.value.scrollTop = 0
})

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
    :class="{ 'soul-pledge--played': !isPlaying }"
    role="dialog"
    aria-modal="true"
    aria-labelledby="soul-pledge-title"
    @click="onStageClick"
  >
    <h1 id="soul-pledge-title" class="visually-hidden">灵魂宣誓 — momo 的承诺</h1>

    <div class="soul-pledge__avatar">
      <PetCanvas
        :draggable="false"
        :size="{ width: 200, height: 200 }"
        :enable-reaction="false"
        @loaded="onCanvasLoaded"
        @error="onCanvasError"
      />
    </div>

    <p class="soul-pledge__signature" aria-hidden="true">
      <span class="soul-pledge__signature-line"></span>
      <span class="soul-pledge__signature-text">
        默默
        <span class="soul-pledge__signature-dot">·</span>
        <span class="soul-pledge__signature-id">momo</span>
      </span>
      <span class="soul-pledge__signature-line"></span>
    </p>

    <article class="soul-pledge__letter" aria-live="polite">
      <p
        v-for="(p, i) in paragraphs"
        :key="i"
        class="soul-pledge__paragraph"
        :class="{
          'is-visible': i < visibleCount,
          'soul-pledge__paragraph--lead': i === 0,
        }"
        :aria-hidden="i >= visibleCount"
      >
        {{ p }}
      </p>
      <div v-show="!isPlaying" class="soul-pledge__rule">
        <button
          type="button"
          class="soul-pledge__policy-link"
          :disabled="!buttonsEnabled"
          @click.stop="onShowPolicy"
        >
          看看完整数据策略
        </button>
      </div>
    </article>

    <footer class="soul-pledge__footer">
      <div class="soul-pledge__actions" role="group" aria-label="操作">
        <ElButton
          ref="policyBtnRef"
          :disabled="!buttonsEnabled"
          @click="onShowPolicy"
        >
          再看一眼条款
        </ElButton>
        <ElButton text :disabled="!buttonsEnabled" @click="onExit">退出</ElButton>
        <ElButton
          type="primary"
          :disabled="!buttonsEnabled"
          :loading="agreeing"
          @click="onAgree"
        >
          我懂了,一起开始
        </ElButton>
      </div>
      <p class="soul-pledge__kbd-hints" aria-hidden="true">
        按 <kbd class="soul-pledge__kbd">ESC</kbd> 随时退出
      </p>
    </footer>

    <StandardDialog
      v-model="showPolicy"
      title="完整数据策略 v1.0"
      width="90%"
    >
      <!-- dataPolicyHtml 来自构建期嵌入的本地 data_policy_v1.md（?raw + marked），非用户输入；
           外链已替换为 .onboarding-link-disabled span，无 XSS 风险 -->
      <!-- eslint-disable-next-line vue/no-v-html -->
      <div ref="policyScrollRef" class="soul-pledge__policy" v-html="dataPolicyHtml"></div>
      <template #footer>
        <ElButton type="primary" @click="showPolicy = false">我读完了</ElButton>
      </template>
    </StandardDialog>
  </section>
</template>

<style scoped>
/* ============ 舞台容器 ============
 * isolation: isolate → 创建独立 stacking context,让 ::before 光晕 z-index:-1
 * 只在 .soul-pledge 内"下沉",不会跑到根 stacking context 的全窗背景之下消失。
 * overflow: hidden → 光晕 720×520 椭圆边缘超出窗口时被裁掉,不溢出 scrollbar。
 */
.soul-pledge {
  position: relative;
  isolation: isolate;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  align-items: center;
  width: 100%;
  height: 100%;
  padding: var(--aipet-space-3) var(--aipet-space-6) var(--aipet-space-4);
  background: var(--aipet-color-bg);
  box-sizing: border-box;
  user-select: none;
  /* 文案播放期间 stage 整体可点击跳过 → cursor:pointer 提示；
     播完后（.soul-pledge--played）skip 已无效，cursor 恢复 default 避免误导 */
  cursor: pointer;
}

.soul-pledge--played {
  cursor: default;
}

/* ============ 角色光晕 ============
 * radial-gradient 椭圆,中心稍偏上（28%）对准 avatar 中心,primary 10% → 透明。
 * 单点光源,不是页面级渐变,符合"角色光斑"语义（tokens.css §"无渐变"原则的例外）。
 * z-index: -1 + 父 isolation → 在 .soul-pledge 内最底层,不挡内容。
 */
.soul-pledge::before {
  content: '';
  position: absolute;
  top: 0;
  left: 50%;
  width: 720px;
  height: 520px;
  transform: translateX(-50%);
  background: radial-gradient(
    ellipse 50% 60% at center 28%,
    color-mix(in srgb, var(--aipet-color-primary) 10%, transparent),
    transparent 70%
  );
  pointer-events: none;
  z-index: -1;
  opacity: 0;
  animation: soul-pledge-glow-in var(--aipet-duration-slow) var(--aipet-ease-standard) 200ms forwards;
}

@keyframes soul-pledge-glow-in {
  to {
    opacity: 1;
  }
}

/* ============ avatar ============ */
.soul-pledge__avatar {
  /* PetCanvas 320×320 居中显示在顶部 */
  flex: 0 0 auto;
  margin-bottom: var(--aipet-space-2);
}

/* ============ 角色署名 ============
 * "── 默默 · momo ──"：左右 1px hairline + 中间 mono 字 + 字距开间。
 * letter-spacing 0.16em 营造"戏剧海报副标题"的呼吸感。
 */
.soul-pledge__signature {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--aipet-space-3);
  margin: 0 0 var(--aipet-space-2);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  opacity: 0;
  animation: soul-pledge-sig-in var(--aipet-duration-slow) var(--aipet-ease-standard) 400ms forwards;
}

@keyframes soul-pledge-sig-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

.soul-pledge__signature-line {
  width: 32px;
  height: 1px;
  background: var(--aipet-color-border);
}

.soul-pledge__signature-text {
  display: inline-flex;
  align-items: baseline;
  gap: var(--aipet-space-2);
  letter-spacing: 0.16em;
}

.soul-pledge__signature-dot {
  color: var(--aipet-color-border-strong);
}

.soul-pledge__signature-id {
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  letter-spacing: 0.08em;
  color: var(--aipet-color-text-3);
}

/* ============ 信笺卡片 ============
 * 用 surface + radius-card + shadow-sm + border-faint 形成"信纸"质感。
 * 与 .soul-pledge bg 同色（亮色都是 #ffffff），靠 shadow + border 浮起 → 符合
 * tokens.css 注释"L0/L2 同色,靠 shadow + border 分层"（Bear/Linear/MacOS Big Sur 通行）。
 * 暗色下 surface=#2a2a2a 与 bg=#171717 自然有 +11 灰度差,纯色阶梯生效。
 *
 * cursor: default → 卡片内 cursor 不是 pointer（暗示阅读区）；点击仍冒泡到 section
 * 触发 skipToEnd（stage 整体可 skip,设计意图见 script line 96-103 注释）。
 */
.soul-pledge__letter {
  /* flex: 1 1 0 + min-height: 0 让 letter 严格按 flex 容器分配的空间裁剪自己,
     内容超过时 overflow-y: auto 优雅滚动。640 紧凑窗下接受 scroll fallback,
     大多数显示器（缩放 100%）能装下不显 scrollbar。 */
  flex: 1 1 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  justify-content: flex-start;
  width: 100%;
  max-width: 400px;
  padding: var(--aipet-space-2) var(--aipet-space-3);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-card);
  background: var(--aipet-color-surface);
  box-shadow: var(--aipet-shadow-sm);
  cursor: default;
  overflow-y: auto;
  /* 极轻 scrollbar:仅在 hover 时显形,不抢仪式感 */
  scrollbar-width: thin;
  scrollbar-color: transparent transparent;
}

.soul-pledge__letter:hover {
  scrollbar-color: var(--aipet-color-border) transparent;
}

.soul-pledge__letter::-webkit-scrollbar {
  width: 4px;
}

.soul-pledge__letter::-webkit-scrollbar-thumb {
  background: transparent;
  border-radius: 2px;
}

.soul-pledge__letter:hover::-webkit-scrollbar-thumb {
  background: var(--aipet-color-border);
}

/* ============ 段落渐进出现 ============
 * translateY 12px（原 8）+ duration-slow（原 base），"涌现"感更明显。
 * line-height: loose（1.6），阅读区呼吸感拉开。
 */
.soul-pledge__paragraph {
  margin: 0 0 var(--aipet-space-2);
  font-size: var(--aipet-font-size-base);
  line-height: 1.5;
  color: var(--aipet-color-text-1);
  text-align: center;
  opacity: 0;
  transform: translateY(12px);
  transition:
    opacity var(--aipet-duration-slow) var(--aipet-ease-standard),
    transform var(--aipet-duration-slow) var(--aipet-ease-standard);
}

.soul-pledge__paragraph:last-of-type {
  margin-bottom: 0;
}

.soul-pledge__paragraph.is-visible {
  opacity: 1;
  transform: translateY(0);
}

/* ============ 首段 drop cap ============
 * 中文 ::first-letter 选中第一个字符（如"诶"），放大 + 紫色 + 下沉。
 * 注意：text-align: center 会让 drop cap 居中,需要切回 left 才能形成 drop 效果。
 */
.soul-pledge__paragraph--lead {
  text-align: left;
}

.soul-pledge__paragraph--lead::first-letter {
  float: left;
  margin-right: var(--aipet-space-2);
  padding-top: 2px;
  font-size: 2em;
  font-weight: 600;
  line-height: 0.95;
  color: var(--aipet-color-primary);
}

/* ============ 分隔器（hr-with-label）============
 * "── 看看完整数据策略 ──"：左右伸出 hairline + 中间可点击 link。
 * flex 1 占据剩余 → line 自动延伸到卡片左右边。
 * ChatGPT empty state / Linear divider 通行做法。
 */
.soul-pledge__rule {
  display: flex;
  align-items: center;
  justify-content: center;
  margin-top: var(--aipet-space-3);
}

.soul-pledge__rule::before,
.soul-pledge__rule::after {
  content: '';
  flex: 1 1 auto;
  height: 1px;
  background: var(--aipet-color-border);
  opacity: 0;
  animation: soul-pledge-rule-in var(--aipet-duration-slow) var(--aipet-ease-standard) 100ms forwards;
}

@keyframes soul-pledge-rule-in {
  to {
    opacity: 1;
  }
}

.soul-pledge__policy-link {
  flex: 0 0 auto;
  padding: var(--aipet-space-1) var(--aipet-space-3);
  margin: 0 var(--aipet-space-2);
  border: none;
  background: transparent;
  color: var(--aipet-color-text-3);
  font: inherit;
  font-size: var(--aipet-font-size-sm);
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

/* ============ footer ============
 * 用 border-top hairline 与上方信笺卡片软性切分,而不是裸悬底部。
 * cursor: default → 按钮区不参与 stage skip 视觉提示。
 */
.soul-pledge__footer {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--aipet-space-2);
  width: 100%;
  max-width: 440px;
  margin-top: var(--aipet-space-3);
  padding-top: var(--aipet-space-3);
  border-top: 1px solid var(--aipet-color-border-faint);
  cursor: default;
}

.soul-pledge__actions {
  display: flex;
  gap: var(--aipet-space-3);
  align-items: center;
}

/* ============ 键盘提示 ============
 * Linear/Telegram 桌面端通行：底部 mono 提示一两个关键快捷键。
 * 只提 ESC（默认 focus 在"再看一眼条款",Enter 行为取决于 focus,
 * 不要误导用户以为 Enter = 同意,避免破坏 script 中"focus 默认非危险按钮"的 UX 意图）。
 */
.soul-pledge__kbd-hints {
  margin: 0;
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
  letter-spacing: 0.04em;
}

.soul-pledge__kbd {
  display: inline-block;
  min-width: 28px;
  padding: 1px 6px;
  margin: 0 2px;
  border: 1px solid var(--aipet-color-border);
  border-bottom-width: 2px;
  border-radius: var(--aipet-radius-sm);
  background: var(--aipet-color-surface-soft);
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-2);
  line-height: 1.2;
  text-align: center;
}

/* ============ policy 弹窗内容 ============ */
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
  background: var(--aipet-color-surface-soft);
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
