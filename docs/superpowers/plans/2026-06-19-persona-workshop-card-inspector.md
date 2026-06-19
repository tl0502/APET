# Persona Workshop Card Inspector Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Persona Workshop three-column layout with a character-card stage and card-triggered right-side Inspector Drawer.

**Architecture:** Keep `PersonaWorkshopPanel.vue` as the orchestration surface. Replace the narrow list pane with `PersonaCardStage.vue`, move preview/diagnostics into `PersonaInspectorDrawer.vue`, and keep `PersonaEditorTabs.vue` as the editor-mode implementation. The Inspector is initially closed, opens only when a role card is clicked, compresses the card stage on desktop, and overlays only inside the Workshop container on small windows.

**Tech Stack:** Vue 3, TypeScript, `<script setup lang="ts">`, Element Plus, Vitest, Vue Test Utils.

---

## File Structure

- Modify: `src/components/persona-workshop/PersonaWorkshopPanel.vue`
  - Owns loading, selected persona id, draft, mode, diagnostics, token estimate, drawer open state, and service calls.
  - Renders toolbar, card stage, and inspector drawer.
  - Uses single-column card stage by default, desktop two-region layout only while inspector is open, and container-scoped overlay at small widths.
- Create: `src/components/persona-workshop/PersonaCardStage.vue`
  - Pure presentational stage for character cards.
  - Props: `personas`, `selectedId`, `loading`.
  - Emits: `select(id)`.
- Create: `src/components/persona-workshop/PersonaInspectorDrawer.vue`
  - Context editor for the selected persona.
  - Props: `open`, `draft`, `mode`, `personaName`, `diagnostics`, `tokenEstimate`.
  - Emits: `close`, `update:mode`, `update:draft`.
- Modify: `src/components/persona-workshop/PersonaEditorTabs.vue`
  - Keep behavior, adjust styling only if needed inside drawer.
- Delete: `src/components/persona-workshop/PersonaListPane.vue`
  - Replaced by `PersonaCardStage.vue`.
- Delete: `src/components/persona-workshop/PersonaPreviewPane.vue`
  - Its diagnostics/token/actions are folded into `PersonaInspectorDrawer.vue`.
- Modify: `src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts`
  - Assert card-stage and inspector behavior instead of three-pane behavior.

---

### Task 1: Update the Workshop Test Contract

**Files:**
- Modify: `src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts`

- [ ] **Step 1: Write the failing test**

Replace the current test body with assertions for the new IA:

```ts
test('loads active persona into card stage and inspector drawer', async () => {
  const wrapper = mount(PersonaWorkshopPanel, {
    props: { isActive: true },
    global: {
      stubs: {
        ElButton: { template: '<button @click="$emit(`click`)"><slot /></button>' },
        ElInput: { template: '<input />' },
        ElSlider: { template: '<input type="range" />' },
        ElTag: { template: '<span><slot /></span>' },
      },
    },
  })

  await flushPromises()

  expect(wrapper.find('[aria-label="角色卡舞台"]').exists()).toBe(true)
  expect(wrapper.find('[aria-label="人格编辑抽屉"]').exists()).toBe(true)
  expect(wrapper.text()).toContain('角色卡')
  expect(wrapper.text()).toContain('默默')
  expect(wrapper.text()).toContain('阿吉')
  expect(wrapper.text()).toContain('塑形')
  expect(wrapper.text()).toContain('结构')
  expect(wrapper.text()).toContain('源码')
  expect(wrapper.text()).toContain('编译诊断')
})
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
pnpm vitest run src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts
```

Expected: FAIL because `角色卡舞台` and `人格编辑抽屉` do not exist yet.

---

### Task 2: Add the Character Card Stage

**Files:**
- Create: `src/components/persona-workshop/PersonaCardStage.vue`
- Delete: `src/components/persona-workshop/PersonaListPane.vue`

- [ ] **Step 1: Implement the card stage component**

Create a focused presentational component that renders a responsive card grid:

```vue
<script setup lang="ts">
import { ElTag } from 'element-plus'
import type { PersonaListItem } from '@/types/persona'

const props = defineProps<{
  personas: PersonaListItem[]
  selectedId: string | null
  loading: boolean
}>()

const emit = defineEmits<{
  select: [id: string]
}>()
</script>
```

Template requirements:

```vue
<section class="persona-card-stage" aria-label="角色卡舞台">
  <p v-if="props.loading" class="persona-card-stage__state">加载中...</p>
  <div v-else class="persona-card-stage__grid">
    <button
      v-for="persona in props.personas"
      :key="persona.id"
      type="button"
      class="persona-card"
      :class="{ 'persona-card--selected': persona.id === props.selectedId }"
      @click="emit('select', persona.id)"
    >
      <span class="persona-card__label">角色卡</span>
      <strong class="persona-card__name">{{ persona.name }}</strong>
      <span class="persona-card__id">{{ persona.id }}</span>
      <span class="persona-card__meta">
        <ElTag size="small">{{ persona.source }}</ElTag>
        <ElTag v-if="persona.is_active" size="small" type="success">active</ElTag>
      </span>
    </button>
  </div>
</section>
```

- [ ] **Step 2: Style the stage**

Use full-width stage composition with stable cards:

```css
.persona-card-stage {
  min-width: 0;
  min-height: 0;
  overflow: auto;
}

.persona-card-stage__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: var(--aipet-space-4);
}

.persona-card {
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  min-height: 220px;
  padding: var(--aipet-space-4);
  border: 1px solid var(--aipet-color-border-faint);
  border-radius: var(--aipet-radius-card);
  background: var(--aipet-color-surface);
  color: inherit;
  text-align: left;
}
```

---

### Task 3: Add the Inspector Drawer

**Files:**
- Create: `src/components/persona-workshop/PersonaInspectorDrawer.vue`
- Delete: `src/components/persona-workshop/PersonaPreviewPane.vue`

- [ ] **Step 1: Implement the drawer component**

Create a drawer that combines current editor modes and diagnostics:

```vue
<script setup lang="ts">
import { ElButton, ElTag } from 'element-plus'
import PersonaEditorTabs from './PersonaEditorTabs.vue'
import type {
  PersonaDiagnostic,
  PersonaSourceDraft,
  PersonaWorkshopMode,
} from '@/features/persona-workshop/types'

const props = defineProps<{
  open: boolean
  draft: PersonaSourceDraft | null
  mode: PersonaWorkshopMode
  personaName: string
  diagnostics: PersonaDiagnostic[]
  tokenEstimate: number
}>()

const emit = defineEmits<{
  close: []
  'update:mode': [mode: PersonaWorkshopMode]
  'update:draft': [draft: PersonaSourceDraft]
}>()
</script>
```

Template requirements:

```vue
<aside v-if="props.open" class="persona-inspector" aria-label="人格编辑抽屉">
  <header class="persona-inspector__header">
    <div>
      <p class="persona-inspector__eyebrow">Inspector</p>
      <h3 class="persona-inspector__title">{{ props.personaName }}</h3>
    </div>
    <ElButton size="small" @click="emit('close')">关闭</ElButton>
  </header>

  <PersonaEditorTabs
    v-if="props.draft"
    :draft="props.draft"
    :mode="props.mode"
    @update:mode="emit('update:mode', $event)"
    @update:draft="emit('update:draft', $event)"
  />
  <p v-else class="persona-inspector__empty">选择一张角色卡开始编辑</p>

  <section class="persona-inspector__diagnostics">
    <div class="persona-inspector__row">
      <span>Token 估算</span>
      <ElTag size="small">{{ props.tokenEstimate }}</ElTag>
    </div>
    <h4>编译诊断</h4>
    <p v-if="props.diagnostics.length === 0">没有阻塞问题</p>
    <ul v-else>
      <li v-for="diagnostic in props.diagnostics" :key="diagnostic.code">
        {{ diagnostic.message }}
      </li>
    </ul>
  </section>
</aside>
```

---

### Task 4: Rewire PersonaWorkshopPanel

**Files:**
- Modify: `src/components/persona-workshop/PersonaWorkshopPanel.vue`

- [ ] **Step 1: Replace imports and add drawer state**

Use the new components:

```ts
import PersonaCardStage from './PersonaCardStage.vue'
import PersonaInspectorDrawer from './PersonaInspectorDrawer.vue'
```

Add:

```ts
const inspectorOpen = shallowRef(false)
```

- [ ] **Step 2: Keep selection behavior and open inspector on card click**

Update `selectPersona`:

```ts
async function selectPersona(id: string) {
  inspectorOpen.value = true
  if (selectedId.value === id) return
  selectedId.value = id
  try {
    const persona = await loadPersona(id)
    draft.value = createPersonaDraft(persona)
    mode.value = 'simple'
    errorMsg.value = null
  } catch (e) {
    errorMsg.value = e instanceof Error ? e.message : String(e)
  }
}
```

- [ ] **Step 3: Replace the three-column grid template**

Render:

```vue
<div class="persona-workshop__layout">
  <main class="persona-workshop__stage">
    <PersonaCardStage
      :personas="personas"
      :selected-id="selectedId"
      :loading="loading"
      @select="selectPersona"
    />
  </main>

  <PersonaInspectorDrawer
    :open="inspectorOpen"
    :draft="draft"
    :mode="mode"
    :persona-name="personaName"
    :diagnostics="diagnostics"
    :token-estimate="tokenEstimate"
    @close="inspectorOpen = false"
    @update:mode="mode = $event"
    @update:draft="draft = $event"
  />
</div>
```

- [ ] **Step 4: Style the layout**

Use a stage-first layout that only allocates drawer width while open:

```css
.persona-workshop__layout {
  display: grid;
  position: relative;
  grid-template-columns: minmax(0, 1fr);
  gap: var(--aipet-space-4);
  min-height: 0;
  flex: 1 1 auto;
}

.persona-workshop__layout--inspector-open {
  grid-template-columns: minmax(0, 1fr) minmax(340px, 380px);
}

.persona-workshop__stage {
  min-width: 0;
  min-height: 0;
}

@media (max-width: 900px) {
  .persona-workshop__layout--inspector-open {
    grid-template-columns: minmax(0, 1fr);
  }

  .persona-workshop__layout--inspector-open :deep(.persona-inspector) {
    position: absolute;
    inset: 0 0 0 auto;
    width: min(380px, 100%);
    z-index: 1;
  }
}
```

---

### Task 5: Verify and Clean Up

**Files:**
- Modify only files touched by Tasks 1-4.

- [ ] **Step 1: Run targeted tests**

Run:

```bash
pnpm vitest run src/features/persona-workshop/__tests__/draft.test.ts src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts
```

Expected: all tests pass.

- [ ] **Step 2: Run typecheck**

Run:

```bash
pnpm typecheck
```

Expected: exits successfully.

- [ ] **Step 3: Run targeted lint**

Run:

```bash
pnpm exec eslint src/features/persona-workshop src/components/persona-workshop src/panels/settings/SettingsPersonaPanel.vue
```

Expected: exits successfully.

- [ ] **Step 4: Check for deleted component references**

Run:

```bash
rg "PersonaListPane|PersonaPreviewPane" src/components/persona-workshop src/panels/settings
```

Expected: no matches.

---

## Self-Review

- Spec coverage: implements the approved character-card stage and fixed right Inspector Drawer. It does not implement persistence, import/export, or save activation because those were already outside this vertical-slice UI refactor.
- Placeholder scan: no `TBD`, `TODO`, or deferred implementation steps.
- Type consistency: all new component contracts use existing `PersonaListItem`, `PersonaSourceDraft`, `PersonaWorkshopMode`, and `PersonaDiagnostic` types.
