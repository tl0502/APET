<script setup lang="ts">
// PersonaPickerView：Onboarding Step 2 — 从 3 内置人格里选一个（flows §1.2 + ADR-009）。
//
// 关键设计：
// 1. 显示顺序固定 [momo, joker, coach]（flows §1.2 字面），不跟 active 排序漂
//    —— 重新走 onboarding（reconsent / 改 granted=0）时，"产品默认"位置不变
// 2. 默认选中固定为 momo（产品锚定的默认人格）；不跟 active 漂
//    —— 上次选了 joker、删 consent 重新走时,默认仍是默默
// 3. "跳过(用默认)" = 强制 activate momo（保证回归产品默认）。如果当前 active 已是 momo，
//    activate 仍调用一次（IPC 幂等，开销可接受），换得逻辑统一
// 4. "用这个" = 选中 != 当前 active 时调 activate；否则跳过 IPC 直接 emit done
// 5. listPersonas 失败时 toast warn + 仍允许"跳过"通过流程（不卡死 onboarding）
// 6. tagline 文案 M1 hardcode by id；M2 工坊接入时可改 frontmatter 字段或抓 .soul.md 段

import { onMounted, ref } from 'vue'
import { ElButton } from 'element-plus'
import { activatePersona, listPersonas } from '@/services/persona'
import type { PersonaListItem } from '@/types/persona'
import { useToast } from '@/composables/useToast'

const emit = defineEmits<{ done: [] }>()
const toast = useToast()

const personas = ref<PersonaListItem[]>([])
const loading = ref(true)
const selectedId = ref<string | null>(null)
const submitting = ref(false)

/** 产品默认人格 id（flows §1.2 / ADR-008 灵魂宣誓页叙述者）。 */
const DEFAULT_PERSONA_ID = 'momo'

/** 显示顺序 = flows §1.2 字面（"默默 / 阿吉 / 教官"）。3 内置 id 后端 const 化,不会漂。 */
const DISPLAY_ORDER = ['momo', 'joker', 'coach'] as const

/**
 * 卡片副文案：用 persona-design.md §5 关键词浓缩成一行 hook。
 * 这里 hardcode by id 而不是抓 raw markdown 区段，是为了：
 * 1. picker UI 文案需要"挑选感"（短、口语、对比），跟 .soul.md 给 LLM 的"人格描述"语义不同
 * 2. raw markdown 后端不返（list 不带 raw_markdown，按需 persona_load(id) 拉）
 * 3. 3 内置 id 后端 BUILTIN_SEEDS 常量化，不会漂；用户人格 fallback 空字符串即可
 */
const TAGLINE_BY_ID: Record<string, string> = {
  momo: '慵懒,但靠谱。安静地陪着你。',
  joker: '损友兼气氛组,嘴上不饶人,心里软。',
  coach: '克制、专业、不废话。推你完成。',
}

function sortByDisplayOrder(items: PersonaListItem[]): PersonaListItem[] {
  const indexOf = (id: string) => {
    const i = DISPLAY_ORDER.indexOf(id as (typeof DISPLAY_ORDER)[number])
    return i === -1 ? Number.MAX_SAFE_INTEGER : i
  }
  return [...items].sort((a, b) => {
    const ai = indexOf(a.id)
    const bi = indexOf(b.id)
    if (ai !== bi) return ai - bi
    // 不在 DISPLAY_ORDER 里的（未来用户自建人格）按 id ASC 兜底
    return a.id.localeCompare(b.id)
  })
}

onMounted(async () => {
  try {
    const list = await listPersonas()
    personas.value = sortByDisplayOrder(list)
    // 默认选中固定为产品默认人格；缺失时 fallback 第一张（防极端 list 空 / momo 缺失）
    selectedId.value =
      personas.value.find((p) => p.id === DEFAULT_PERSONA_ID)?.id ??
      personas.value[0]?.id ??
      null
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    console.error('[PersonaPickerView] listPersonas failed:', e)
    toast.warn(`人格列表加载失败：${msg}（可跳过沿用默认）`, { duration: 5000 })
  } finally {
    loading.value = false
  }
})

function onSelect(id: string) {
  if (submitting.value) return
  selectedId.value = id
}

async function onConfirm() {
  if (submitting.value || !selectedId.value) return
  const current = personas.value.find((p) => p.is_active)
  // 选中 = 当前 active：跳过 IPC 直接 emit done（少一次往返）
  if (current?.id === selectedId.value) {
    emit('done')
    return
  }
  submitting.value = true
  try {
    await activatePersona(selectedId.value)
    emit('done')
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    console.error('[PersonaPickerView] activatePersona failed:', e)
    toast.error(`激活失败：${msg}`, { duration: 5000 })
    submitting.value = false
  }
}

async function onSkip() {
  if (submitting.value) return
  const current = personas.value.find((p) => p.is_active)
  // 跳过 = 强制回到产品默认人格（默默）。如果当前已经是 momo，IPC 仍跑一次，幂等无副作用，
  // 换"逻辑统一"：用户看到"跳过(用默认)"就一定能确定 active=momo，不受历史 active 干扰。
  // list 加载失败导致 personas 空时，DEFAULT_PERSONA_ID 兜底；activate 失败 toast + 不切步
  // （比"激活失败但 onboarding 继续"对用户更诚实）。
  if (current?.id === DEFAULT_PERSONA_ID) {
    emit('done')
    return
  }
  submitting.value = true
  try {
    await activatePersona(DEFAULT_PERSONA_ID)
    emit('done')
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e)
    console.error('[PersonaPickerView] skip-to-default activate failed:', e)
    toast.error(`回退到默认失败：${msg}`, { duration: 5000 })
    submitting.value = false
  }
}
</script>

<template>
  <section
    class="persona-picker"
    role="dialog"
    aria-modal="true"
    aria-labelledby="persona-picker-title"
  >
    <h1 id="persona-picker-title" class="persona-picker__title">
      和我一起的,会是一个什么样的伴?
    </h1>
    <p class="persona-picker__hint">选了不喜欢,以后还能换。或者跳过,用我(默默)。</p>

    <p v-if="loading" class="persona-picker__loading">加载中...</p>
    <ul
      v-else
      class="persona-picker__list"
      role="radiogroup"
      aria-label="选择内置人格"
    >
      <li
        v-for="p in personas"
        :key="p.id"
        :class="['persona-card', { 'persona-card--selected': p.id === selectedId }]"
        role="radio"
        :aria-checked="p.id === selectedId"
        tabindex="0"
        @click="onSelect(p.id)"
        @keydown.enter.prevent="onSelect(p.id)"
        @keydown.space.prevent="onSelect(p.id)"
      >
        <div class="persona-card__head">
          <span class="persona-card__name">{{ p.name }}</span>
          <span class="persona-card__id">{{ p.id }}</span>
        </div>
        <div class="persona-card__tagline">{{ TAGLINE_BY_ID[p.id] ?? '' }}</div>
      </li>
    </ul>

    <div class="persona-picker__actions" role="group" aria-label="操作">
      <ElButton :disabled="submitting" @click="onSkip">跳过(用默认)</ElButton>
      <ElButton
        type="primary"
        :disabled="submitting || !selectedId"
        :loading="submitting"
        @click="onConfirm"
      >
        用这个
      </ElButton>
    </div>
  </section>
</template>

<style scoped>
.persona-picker {
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

.persona-picker__title {
  margin: 0 0 var(--aipet-space-2);
  font-size: var(--aipet-font-size-xl);
  font-weight: 600;
  color: var(--aipet-color-text-1);
  text-align: center;
}

.persona-picker__hint {
  margin: 0 0 var(--aipet-space-5);
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
  text-align: center;
}

.persona-picker__loading {
  margin: var(--aipet-space-6) 0;
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-3);
  text-align: center;
}

.persona-picker__list {
  display: flex;
  flex-direction: column;
  gap: var(--aipet-space-3);
  margin: 0 0 var(--aipet-space-6);
  padding: 0;
  list-style: none;
}

.persona-card {
  padding: var(--aipet-space-4) var(--aipet-space-5);
  border: 1px solid var(--aipet-color-border);
  border-radius: var(--aipet-radius-base);
  background: var(--aipet-color-surface);
  cursor: pointer;
  transition:
    border-color var(--aipet-duration-fast) var(--aipet-ease-standard),
    background var(--aipet-duration-fast) var(--aipet-ease-standard);
}

.persona-card:hover,
.persona-card:focus-visible {
  border-color: var(--aipet-color-primary);
  outline: none;
}

.persona-card--selected {
  border-color: var(--aipet-color-primary);
  background: color-mix(in srgb, var(--aipet-color-primary) 12%, var(--aipet-color-surface));
}

.persona-card__head {
  display: flex;
  align-items: baseline;
  gap: var(--aipet-space-2);
  margin-bottom: var(--aipet-space-1);
}

.persona-card__name {
  font-size: var(--aipet-font-size-lg);
  font-weight: 600;
  color: var(--aipet-color-text-1);
}

.persona-card__id {
  font-family: var(--aipet-font-family-mono);
  font-size: var(--aipet-font-size-xs);
  color: var(--aipet-color-text-3);
}

.persona-card__tagline {
  font-size: var(--aipet-font-size-sm);
  color: var(--aipet-color-text-2);
  line-height: var(--aipet-line-height-base);
}

.persona-picker__actions {
  display: flex;
  gap: var(--aipet-space-3);
  justify-content: center;
  margin-top: auto;
}
</style>
