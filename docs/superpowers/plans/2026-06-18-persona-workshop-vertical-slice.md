# Persona Workshop Vertical Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the placeholder Persona settings panel with a usable Persona Workshop front-end vertical slice: persona list, Simple / Structured / Source editing tabs, compile-style diagnostics, token estimate, and local preview.

**Architecture:** Keep this slice front-end only and non-destructive. Existing `persona_list` / `persona_get_active` IPC remains the data source; draft editing is local until the back-end SoulRuntimeProfile / PersonaSnapshot implementation lands. Pure draft projection and validation logic lives in `src/features/persona-workshop/`, while Vue SFCs stay focused on UI and explicit props/emits.

**Tech Stack:** Vue 3 Composition API with `<script setup lang="ts">`, Pinia-free local component state for this slice, Element Plus controls, Vitest for pure logic and component smoke tests.

---

## Component Map

- `src/features/persona-workshop/types.ts`: shared draft, diagnostics, mode, and section types.
- `src/features/persona-workshop/draft.ts`: pure helpers for creating draft state from a `PersonaSummary`, projecting to source text, applying Simple edits, diagnostics, and token estimates.
- `src/features/persona-workshop/__tests__/draft.test.ts`: TDD coverage for draft projection, validation, unknown source preservation, and budget estimate.
- `src/components/persona-workshop/PersonaWorkshopPanel.vue`: feature container; loads persona list / active persona, owns draft state, wires child components.
- `src/components/persona-workshop/PersonaListPane.vue`: left rail list and actions; props in, events up.
- `src/components/persona-workshop/PersonaEditorTabs.vue`: middle editor; owns tab UI, delegates mode-specific fields inline for this slice.
- `src/components/persona-workshop/PersonaPreviewPane.vue`: right rail diagnostics, token estimate, source preview, and disabled future actions.
- `src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts`: component smoke test with mocked persona service.
- `src/panels/settings/SettingsPersonaPanel.vue`: thin wrapper that hosts `PersonaWorkshopPanel` and keeps `VrmAvatarExporter` active-state behavior.

## Task 1: Pure Draft Model

**Files:**
- Create: `src/features/persona-workshop/types.ts`
- Create: `src/features/persona-workshop/draft.ts`
- Test: `src/features/persona-workshop/__tests__/draft.test.ts`

- [ ] **Step 1: Write failing draft tests**

Create `src/features/persona-workshop/__tests__/draft.test.ts`:

```ts
import { describe, expect, test } from 'vitest'
import type { PersonaSummary } from '@/types/persona'
import {
  applySimplePatch,
  createPersonaDraft,
  estimateDraftTokens,
  projectDraftToSource,
  validatePersonaDraft,
} from '../draft'

const persona: PersonaSummary = {
  id: 'momo',
  name: '默默',
  version: '1.0.0',
  source: 'builtin',
  raw_markdown: [
    '# 身份',
    '你叫**默默**。',
    '',
    '# 性格',
    '- 慵懒,但靠谱',
    '',
    '# 能力',
    '- 安静地陪伴',
    '',
    '# 行为规则',
    '## Do',
    '- 用第二人称',
    '## Don\\'t',
    '- 不要空洞鼓励',
    '',
    '# 离线模板',
    '## 问候 / Greeting',
    '- 诶,你回来了。',
    '',
    '# 反应配置',
    '```yaml',
    'click.head:',
    '  template: 嗯?',
    '```',
  ].join('\\n'),
}

describe('persona workshop draft helpers', () => {
  test('creates a simple draft from an existing persona summary', () => {
    const draft = createPersonaDraft(persona)

    expect(draft.personaId).toBe('momo')
    expect(draft.simple.name).toBe('默默')
    expect(draft.structured.identity).toContain('你叫**默默**')
    expect(draft.structured.personality).toContain('慵懒')
    expect(draft.sourceText).toContain('# 反应配置')
  })

  test('applies simple edits without deleting structured-only source text', () => {
    const draft = createPersonaDraft(persona)
    const edited = applySimplePatch(draft, {
      name: '小默',
      tagline: '安静但可靠',
      warmth: 5,
    })

    expect(edited.simple.name).toBe('小默')
    expect(edited.simple.warmth).toBe(5)
    expect(edited.structured.reactions).toContain('click.head')
    expect(projectDraftToSource(edited)).toContain('# 反应配置')
  })

  test('validates required identity and behavior rules', () => {
    const draft = createPersonaDraft(persona)
    const broken = {
      ...draft,
      structured: {
        ...draft.structured,
        identity: '',
        rulesDo: [],
      },
    }

    const diagnostics = validatePersonaDraft(broken)

    expect(diagnostics.some((d) => d.severity === 'error' && d.code === 'identity.empty')).toBe(true)
    expect(diagnostics.some((d) => d.severity === 'error' && d.code === 'rules.do.empty')).toBe(true)
  })

  test('estimates tokens from source length with a stable rounded value', () => {
    const draft = createPersonaDraft(persona)

    expect(estimateDraftTokens(draft)).toBeGreaterThan(20)
    expect(estimateDraftTokens(draft)).toBeLessThan(300)
  })
})
```

- [ ] **Step 2: Run the test and verify RED**

Run: `pnpm vitest run src/features/persona-workshop/__tests__/draft.test.ts`

Expected: FAIL because `src/features/persona-workshop/draft.ts` does not exist.

- [ ] **Step 3: Add model types**

Create `src/features/persona-workshop/types.ts`:

```ts
export type PersonaWorkshopMode = 'simple' | 'structured' | 'source'

export type PersonaDiagnosticSeverity = 'error' | 'warning' | 'info'

export interface PersonaDiagnostic {
  code: string
  severity: PersonaDiagnosticSeverity
  message: string
}

export interface PersonaSimpleDraft {
  name: string
  tagline: string
  relationshipStyle: 'companion' | 'buddy' | 'coach' | 'custom'
  warmth: number
  playfulness: number
  formality: number
  proactivity: number
  brevity: number
  speechLength: 'short' | 'normal' | 'detailed'
  initiative: 'quiet' | 'sometimes' | 'often'
  dislikes: string[]
  examples: string[]
}

export interface PersonaStructuredDraft {
  identity: string
  personality: string
  capabilities: string
  rulesDo: string[]
  rulesDont: string[]
  offlineTemplates: string
  reactions: string
  examples: string
}

export interface PersonaSourceDraft {
  personaId: string
  version: string
  source: string
  simple: PersonaSimpleDraft
  structured: PersonaStructuredDraft
  sourceText: string
  preservedUnknownText: string
}
```

- [ ] **Step 4: Add minimal draft implementation**

Create `src/features/persona-workshop/draft.ts`:

```ts
import type { PersonaSummary } from '@/types/persona'
import type {
  PersonaDiagnostic,
  PersonaSimpleDraft,
  PersonaSourceDraft,
  PersonaStructuredDraft,
} from './types'

const SECTION_LABELS = ['身份', '性格', '能力', '行为规则', '离线模板', '反应配置', '例对话']

function extractSection(markdown: string, label: string): string {
  const lines = markdown.split(/\r?\n/)
  const start = lines.findIndex((line) => line.trim().startsWith(`# ${label}`))
  if (start === -1) return ''
  const body: string[] = []
  for (let i = start + 1; i < lines.length; i++) {
    const line = lines[i]
    if (line.startsWith('# ') && !line.startsWith('## ')) break
    body.push(line)
  }
  return body.join('\n').trim()
}

function extractListItems(markdown: string): string[] {
  return markdown
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter((line) => line.startsWith('- '))
    .map((line) => line.slice(2).trim())
    .filter(Boolean)
}

function splitRules(rules: string): { rulesDo: string[]; rulesDont: string[] } {
  const doIndex = rules.indexOf('## Do')
  const dontIndex = rules.indexOf("## Don't")
  const doText = doIndex === -1 ? '' : rules.slice(doIndex, dontIndex === -1 ? undefined : dontIndex)
  const dontText = dontIndex === -1 ? '' : rules.slice(dontIndex)
  return {
    rulesDo: extractListItems(doText),
    rulesDont: extractListItems(dontText),
  }
}

function buildSimpleDraft(persona: PersonaSummary, structured: PersonaStructuredDraft): PersonaSimpleDraft {
  return {
    name: persona.name,
    tagline: structured.identity.split(/\r?\n/).find(Boolean)?.replace(/\*\*/g, '') ?? '',
    relationshipStyle: 'companion',
    warmth: 3,
    playfulness: 3,
    formality: 2,
    proactivity: 3,
    brevity: 4,
    speechLength: 'short',
    initiative: 'sometimes',
    dislikes: structured.rulesDont.slice(0, 3),
    examples: extractListItems(structured.examples).slice(0, 3),
  }
}

export function createPersonaDraft(persona: PersonaSummary): PersonaSourceDraft {
  const rules = splitRules(extractSection(persona.raw_markdown, '行为规则'))
  const structured: PersonaStructuredDraft = {
    identity: extractSection(persona.raw_markdown, '身份'),
    personality: extractSection(persona.raw_markdown, '性格'),
    capabilities: extractSection(persona.raw_markdown, '能力'),
    rulesDo: rules.rulesDo,
    rulesDont: rules.rulesDont,
    offlineTemplates: extractSection(persona.raw_markdown, '离线模板'),
    reactions: extractSection(persona.raw_markdown, '反应配置'),
    examples: extractSection(persona.raw_markdown, '例对话'),
  }
  return {
    personaId: persona.id,
    version: persona.version,
    source: persona.source,
    simple: buildSimpleDraft(persona, structured),
    structured,
    sourceText: persona.raw_markdown,
    preservedUnknownText: collectUnknownTopLevelSections(persona.raw_markdown),
  }
}

function collectUnknownTopLevelSections(markdown: string): string {
  const chunks: string[] = []
  const lines = markdown.split(/\r?\n/)
  let current: string[] = []
  let keep = false
  for (const line of lines) {
    if (line.startsWith('# ')) {
      if (keep && current.length > 0) chunks.push(current.join('\n').trim())
      current = [line]
      keep = !SECTION_LABELS.some((label) => line.trim().startsWith(`# ${label}`))
      continue
    }
    if (keep) current.push(line)
  }
  if (keep && current.length > 0) chunks.push(current.join('\n').trim())
  return chunks.filter(Boolean).join('\n\n')
}

export function applySimplePatch(
  draft: PersonaSourceDraft,
  patch: Partial<PersonaSimpleDraft>,
): PersonaSourceDraft {
  return {
    ...draft,
    simple: {
      ...draft.simple,
      ...patch,
    },
  }
}

export function projectDraftToSource(draft: PersonaSourceDraft): string {
  const parts = [
    '# 身份',
    draft.structured.identity,
    '# 性格',
    draft.structured.personality,
    '# 能力',
    draft.structured.capabilities,
    '# 行为规则',
    '## Do',
    draft.structured.rulesDo.map((item) => `- ${item}`).join('\n'),
    "## Don't",
    draft.structured.rulesDont.map((item) => `- ${item}`).join('\n'),
    '# 离线模板',
    draft.structured.offlineTemplates,
    '# 反应配置',
    draft.structured.reactions,
  ]
  if (draft.structured.examples.trim()) {
    parts.push('# 例对话', draft.structured.examples)
  }
  if (draft.preservedUnknownText.trim()) {
    parts.push(draft.preservedUnknownText)
  }
  return parts.map((part) => part.trim()).filter(Boolean).join('\n\n')
}

export function validatePersonaDraft(draft: PersonaSourceDraft): PersonaDiagnostic[] {
  const diagnostics: PersonaDiagnostic[] = []
  if (!draft.structured.identity.trim()) {
    diagnostics.push({ code: 'identity.empty', severity: 'error', message: '身份不能为空' })
  }
  if (!draft.structured.personality.trim()) {
    diagnostics.push({ code: 'personality.empty', severity: 'error', message: '性格不能为空' })
  }
  if (draft.structured.rulesDo.length === 0) {
    diagnostics.push({ code: 'rules.do.empty', severity: 'error', message: '至少需要 1 条 Do 规则' })
  }
  if (draft.structured.rulesDont.length === 0) {
    diagnostics.push({ code: 'rules.dont.empty', severity: 'warning', message: "建议至少写 1 条 Don't 规则" })
  }
  if (estimateDraftTokens(draft) > 1200) {
    diagnostics.push({ code: 'budget.high', severity: 'warning', message: '人格定义偏长，会挤压聊天历史' })
  }
  return diagnostics
}

export function estimateDraftTokens(draft: PersonaSourceDraft): number {
  return Math.ceil(projectDraftToSource(draft).length / 3)
}
```

- [ ] **Step 5: Run the draft tests and verify GREEN**

Run: `pnpm vitest run src/features/persona-workshop/__tests__/draft.test.ts`

Expected: PASS.

## Task 2: Persona Workshop UI Shell

**Files:**
- Create: `src/components/persona-workshop/PersonaListPane.vue`
- Create: `src/components/persona-workshop/PersonaEditorTabs.vue`
- Create: `src/components/persona-workshop/PersonaPreviewPane.vue`
- Create: `src/components/persona-workshop/PersonaWorkshopPanel.vue`
- Test: `src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts`
- Modify: `src/panels/settings/SettingsPersonaPanel.vue`

- [ ] **Step 1: Write failing component smoke test**

Create `src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts`:

```ts
import { mount, flushPromises } from '@vue/test-utils'
import { describe, expect, test, vi } from 'vitest'
import PersonaWorkshopPanel from '../PersonaWorkshopPanel.vue'

vi.mock('@/services/persona', () => ({
  getActivePersona: vi.fn(async () => ({
    id: 'momo',
    name: '默默',
    version: '1.0.0',
    source: 'builtin',
    raw_markdown: '# 身份\n你叫默默。\n\n# 性格\n- 慵懒\n\n# 能力\n- 陪伴\n\n# 行为规则\n## Do\n- 用第二人称\n## Don\\'t\n- 不空洞鼓励',
  })),
  listPersonas: vi.fn(async () => [
    { id: 'momo', name: '默默', version: '1.0.0', source: 'builtin', is_active: true },
    { id: 'joker', name: '阿吉', version: '1.0.0', source: 'builtin', is_active: false },
  ]),
}))

describe('PersonaWorkshopPanel', () => {
  test('loads active persona and renders three editor modes', async () => {
    const wrapper = mount(PersonaWorkshopPanel, {
      props: { isActive: true },
      global: {
        stubs: {
          ElButton: { template: '<button><slot /></button>' },
          ElInput: { template: '<input />' },
          ElSlider: { template: '<input type="range" />' },
          ElTag: { template: '<span><slot /></span>' },
          ElSegmented: { template: '<div><slot /></div>' },
        },
      },
    })

    await flushPromises()

    expect(wrapper.text()).toContain('Persona Workshop')
    expect(wrapper.text()).toContain('默默')
    expect(wrapper.text()).toContain('塑形')
    expect(wrapper.text()).toContain('结构')
    expect(wrapper.text()).toContain('源码')
    expect(wrapper.text()).toContain('编译诊断')
  })
})
```

- [ ] **Step 2: Run component test and verify RED**

Run: `pnpm vitest run src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts`

Expected: FAIL because the component does not exist.

- [ ] **Step 3: Implement SFCs with props down / events up**

Implement the four components following the component map. Use `shallowRef` for primitive local state, `computed` for diagnostics and token estimate, and class selectors in scoped CSS.

Minimum visible UI:

- Left: persona list with active marker.
- Middle: segmented mode control with labels `塑形`, `结构`, `源码`.
- Right: `编译诊断`, token estimate, disabled buttons `试聊` and `保存快照`.

- [ ] **Step 4: Replace SettingsPersonaPanel placeholder**

Modify `src/panels/settings/SettingsPersonaPanel.vue` so it imports and renders `PersonaWorkshopPanel`, keeps `props.isActive`, and removes the disabled “打开人格工坊” placeholder copy.

- [ ] **Step 5: Run component test and verify GREEN**

Run: `pnpm vitest run src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts`

Expected: PASS.

## Task 3: Workspace Verification

**Files:**
- Existing only.

- [ ] **Step 1: Run focused frontend tests**

Run:

```bash
pnpm vitest run src/features/persona-workshop/__tests__/draft.test.ts src/components/persona-workshop/__tests__/PersonaWorkshopPanel.test.ts
```

Expected: both test files PASS.

- [ ] **Step 2: Run full frontend tests**

Run: `pnpm test`

Expected: 293 existing tests plus new tests PASS.

- [ ] **Step 3: Run typecheck**

Run: `pnpm typecheck`

Expected: PASS.

- [ ] **Step 4: Commit vertical slice**

Review the file list:

```bash
git status --short
```

Commit only files from this plan and the approved design spec, not unrelated existing changes:

```bash
git add docs/superpowers/specs/2026-06-18-persona-workshop-design.md \
  docs/superpowers/plans/2026-06-18-persona-workshop-vertical-slice.md \
  src/features/persona-workshop \
  src/components/persona-workshop \
  src/panels/settings/SettingsPersonaPanel.vue
git commit -m "feat: add persona workshop vertical slice"
```

## Self-Review

Spec coverage:

- Three-layer model: covered by `PersonaEditorTabs.vue`.
- Source-format undecided: preserved because helpers use `PersonaSourceDraft` and Source mode labels source text as legacy-compatible, not final.
- Runtime boundary: not implemented in this slice; intentionally deferred because back-end `SoulRuntimeProfile` / `PersonaSnapshot` needs a separate plan.
- Persona list / local draft / diagnostics / token estimate: covered.

Placeholder scan:

- No `TBD` / `TODO` instructions. Back-end compile and snapshot work is explicitly out of scope for this vertical slice.

Type consistency:

- `PersonaSourceDraft`, `PersonaSimpleDraft`, `PersonaStructuredDraft`, and `PersonaDiagnostic` are defined once in `types.ts` and consumed by helpers/components.

