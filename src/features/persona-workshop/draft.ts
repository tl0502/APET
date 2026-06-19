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

function buildSimpleDraft(
  persona: PersonaSummary,
  structured: PersonaStructuredDraft,
): PersonaSimpleDraft {
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

  return parts
    .map((part) => part.trim())
    .filter(Boolean)
    .join('\n\n')
}

export function estimateDraftTokens(draft: PersonaSourceDraft): number {
  return Math.ceil(projectDraftToSource(draft).length / 3)
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
    diagnostics.push({
      code: 'rules.dont.empty',
      severity: 'warning',
      message: "建议至少写 1 条 Don't 规则",
    })
  }
  if (estimateDraftTokens(draft) > 1200) {
    diagnostics.push({
      code: 'budget.high',
      severity: 'warning',
      message: '人格定义偏长，会挤压聊天历史',
    })
  }

  return diagnostics
}
