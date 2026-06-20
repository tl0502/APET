import type { PersonaSummary } from '@/types/persona'
import type {
  PersonaDiagnostic,
  PersonaExamplePair,
  PersonaSimpleDraft,
  PersonaSourceDraft,
  PersonaStructuredDraft,
} from './types'

const SECTION_LABELS = ['身份', '性格', '能力', '行为规则', '离线模板', '反应配置', '例对话']
export const MAX_PERSONA_EXAMPLES = 5

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

function stripSpeakerPrefix(line: string): { speaker: string; text: string } | null {
  const normalized = line.trim().replace(/^- /, '').trim()
  const match = normalized.match(/^([^：:]{1,24})[：:]\s*(.*)$/)
  if (!match) return null
  return { speaker: match[1].trim(), text: match[2].trim() }
}

function normalizeExampleText(text: string): string {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .join('\n')
}

function completePairs(pairs: PersonaExamplePair[]): PersonaExamplePair[] {
  return pairs
    .map((pair) => ({
      user: normalizeExampleText(pair.user),
      assistant: normalizeExampleText(pair.assistant),
    }))
    .filter((pair) => pair.user.length > 0 && pair.assistant.length > 0)
    .slice(0, MAX_PERSONA_EXAMPLES)
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
    examples: parsePersonaExamplePairs(structured.examples)
      .map((pair) => `用户：${pair.user}\n${persona.name}：${pair.assistant}`)
      .slice(0, MAX_PERSONA_EXAMPLES),
  }
}

export function nextPersonaId(existingIds: string[], baseId = 'user-persona'): string {
  const used = new Set(existingIds.map((id) => id.trim()).filter(Boolean))
  const base = baseId.trim() || 'user-persona'
  if (!used.has(base)) return base

  for (let index = 2; ; index += 1) {
    const candidate = `${base}-${index}`
    if (!used.has(candidate)) return candidate
  }
}

export function createBlankPersonaDraft(existingIds: string[] = []): PersonaSourceDraft {
  const name = '新人格'
  const personaId = nextPersonaId(existingIds, 'user-persona')
  return {
    personaId,
    version: '1.0.0',
    source: 'user',
    simple: {
      name,
      tagline: '由你塑造的桌面伙伴',
      relationshipStyle: 'companion',
      warmth: 3,
      playfulness: 2,
      formality: 2,
      proactivity: 3,
      brevity: 4,
      speechLength: 'short',
      initiative: 'sometimes',
      dislikes: ['声称拥有未授权系统能力'],
      examples: [],
    },
    structured: {
      identity: `你叫${name}，是一个由用户塑造的桌面伙伴。`,
      personality: '- 温和\n- 可靠',
      capabilities: '- 陪用户聊天\n- 帮用户整理想法',
      rulesDo: ['用第二人称回应', '先理解用户意图再给建议'],
      rulesDont: ['不要声称拥有未授予的系统权限'],
      offlineTemplates: '',
      reactions: '',
      examples: '',
    },
    sourceText: '',
    preservedUnknownText: '',
  }
}

function replaceFirstNameReference(text: string, oldName: string, nextName: string): string {
  const trimmedOldName = oldName.trim()
  if (!trimmedOldName) return text
  return text.replace(trimmedOldName, nextName)
}

export function duplicatePersonaDraft(
  draft: PersonaSourceDraft,
  existingIds: string[] = [],
): PersonaSourceDraft {
  const nextName = `${draft.simple.name.trim() || '人格'} 副本`
  return {
    ...draft,
    personaId: nextPersonaId(existingIds, `${draft.personaId}-copy`),
    version: '1.0.0',
    source: 'user',
    simple: {
      ...draft.simple,
      name: nextName,
    },
    structured: {
      ...draft.structured,
      identity: replaceFirstNameReference(
        draft.structured.identity,
        draft.simple.name,
        nextName,
      ),
    },
    sourceText: '',
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

export function parsePersonaExamplePairs(markdown: string): PersonaExamplePair[] {
  const pairs: PersonaExamplePair[] = []
  let current: PersonaExamplePair | null = null
  let target: 'user' | 'assistant' | null = null

  for (const rawLine of markdown.split(/\r?\n/)) {
    const line = rawLine.trimEnd()
    if (!line.trim()) continue

    const parsed = stripSpeakerPrefix(line)
    if (parsed?.speaker === '用户') {
      if (current) pairs.push(current)
      current = { user: parsed.text, assistant: '' }
      target = 'user'
      continue
    }

    if (parsed && current) {
      current.assistant = parsed.text
      target = 'assistant'
      continue
    }

    if (current && target) {
      current[target] = `${current[target]}\n${line.trim()}`
    }
  }

  if (current) pairs.push(current)
  return pairs.slice(0, MAX_PERSONA_EXAMPLES)
}

export function formatPersonaExamplePairs(
  pairs: PersonaExamplePair[],
  personaName: string,
  options: { includeIncomplete?: boolean } = {},
): string {
  const assistantName = personaName.trim() || '助手'
  const selectedPairs = options.includeIncomplete
    ? pairs
        .map((pair) => ({
          user: normalizeExampleText(pair.user),
          assistant: normalizeExampleText(pair.assistant),
        }))
        .slice(0, MAX_PERSONA_EXAMPLES)
    : completePairs(pairs)

  return selectedPairs
    .map((pair) => {
      const userLines = pair.user.split(/\r?\n/).filter(Boolean)
      const assistantLines = pair.assistant.split(/\r?\n/).filter(Boolean)
      const user =
        options.includeIncomplete && userLines.length === 0
          ? '- 用户：'
          : userLines
              .map((line, index) => (index === 0 ? `- 用户：${line}` : `  ${line}`))
              .join('\n')
      const assistant = assistantLines
        .map((line, index) => (index === 0 ? `  ${assistantName}：${line}` : `  ${line}`))
        .join('\n')
      return [user, assistant].filter(Boolean).join('\n')
    })
    .filter(Boolean)
    .join('\n\n')
}

export function getDraftExamplePairs(draft: PersonaSourceDraft): PersonaExamplePair[] {
  const structuredPairs = parsePersonaExamplePairs(draft.structured.examples)
  if (completePairs(structuredPairs).length > 0 || structuredPairs.length > 0) {
    return structuredPairs
  }
  return draft.simple.examples.flatMap((example) => parsePersonaExamplePairs(example))
}

export function withDraftExamplePairs(
  draft: PersonaSourceDraft,
  pairs: PersonaExamplePair[],
): PersonaSourceDraft {
  const limitedPairs = pairs.slice(0, MAX_PERSONA_EXAMPLES)
  const structuredExamples = formatPersonaExamplePairs(limitedPairs, draft.simple.name, {
    includeIncomplete: true,
  })
  return {
    ...draft,
    simple: {
      ...draft.simple,
      examples: completePairs(limitedPairs).map(
        (pair) => `用户：${pair.user}\n${draft.simple.name.trim() || '助手'}：${pair.assistant}`,
      ),
    },
    structured: {
      ...draft.structured,
      examples: structuredExamples,
    },
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
  const examples = formatPersonaExamplePairs(getDraftExamplePairs(draft), draft.simple.name)
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

  if (examples.trim()) {
    parts.push('# 例对话', examples)
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

  if (!draft.simple.name.trim()) {
    diagnostics.push({ code: 'name.empty', severity: 'error', message: '名字不能为空' })
  }
  if (!draft.structured.identity.trim()) {
    diagnostics.push({ code: 'identity.empty', severity: 'error', message: '身份不能为空' })
  }
  if (!draft.structured.personality.trim()) {
    diagnostics.push({ code: 'personality.empty', severity: 'error', message: '性格不能为空' })
  }
  if (!draft.structured.capabilities.trim()) {
    diagnostics.push({ code: 'capabilities.empty', severity: 'error', message: '能力不能为空' })
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

  const pairs = getDraftExamplePairs(draft)
  const complete = completePairs(pairs)
  if (pairs.length === 0) {
    diagnostics.push({
      code: 'examples.empty',
      severity: 'warning',
      message: '建议补充 1-3 条示例对话；没有示例时，AI 只能靠身份与规则判断语气。',
    })
  }
  if (pairs.some((pair) => pair.user.trim() === '' || pair.assistant.trim() === '')) {
    diagnostics.push({
      code: 'examples.partial',
      severity: 'warning',
      message: '存在未写完整的示例对话，保存时会跳过不完整样本',
    })
  }

  return diagnostics
}
