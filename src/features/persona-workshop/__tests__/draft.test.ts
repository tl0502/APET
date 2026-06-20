import { describe, expect, test } from 'vitest'
import type { PersonaSummary } from '@/types/persona'
import {
  applySimplePatch,
  createPersonaDraft,
  estimateDraftTokens,
  formatPersonaExamplePairs,
  getDraftExamplePairs,
  parsePersonaExamplePairs,
  projectDraftToSource,
  validatePersonaDraft,
  withDraftExamplePairs,
} from '../draft'

const persona: PersonaSummary = {
  id: 'momo',
  snapshot_id: 'momo-1.0.0',
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
    "## Don't",
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
  ].join('\n'),
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

    expect(diagnostics.some((d) => d.severity === 'error' && d.code === 'identity.empty')).toBe(
      true,
    )
    expect(diagnostics.some((d) => d.severity === 'error' && d.code === 'rules.do.empty')).toBe(
      true,
    )
  })

  test('validates name and capabilities as frontend errors', () => {
    const draft = createPersonaDraft(persona)
    const broken = {
      ...draft,
      simple: {
        ...draft.simple,
        name: '   ',
      },
      structured: {
        ...draft.structured,
        capabilities: '   ',
      },
    }

    const diagnostics = validatePersonaDraft(broken)

    expect(diagnostics).toContainEqual({
      code: 'name.empty',
      severity: 'error',
      message: '名字不能为空',
    })
    expect(diagnostics).toContainEqual({
      code: 'capabilities.empty',
      severity: 'error',
      message: '能力不能为空',
    })
  })

  test('estimates tokens from source length with a stable rounded value', () => {
    const draft = createPersonaDraft(persona)

    expect(estimateDraftTokens(draft)).toBeGreaterThan(20)
    expect(estimateDraftTokens(draft)).toBeLessThan(300)
  })

  test('parses markdown example pairs as complete user and assistant turns', () => {
    const pairs = parsePersonaExamplePairs(
      [
        '- 用户：今天有点累，什么都不想做',
        '  默默：那就先慢一点。我在旁边，不催你。',
        '',
        '- 用户：我想偷懒',
        '  默默：可以偷一小会儿，但别把自己丢了。',
      ].join('\n'),
    )

    expect(pairs).toEqual([
      {
        user: '今天有点累，什么都不想做',
        assistant: '那就先慢一点。我在旁边，不催你。',
      },
      {
        user: '我想偷懒',
        assistant: '可以偷一小会儿，但别把自己丢了。',
      },
    ])
  })

  test('formats example pairs back to the soul markdown list shape', () => {
    const markdown = formatPersonaExamplePairs(
      [
        {
          user: '今天有点累，什么都不想做',
          assistant: '那就先慢一点。我在旁边，不催你。',
        },
      ],
      '默默',
    )

    expect(markdown).toBe(
      '- 用户：今天有点累，什么都不想做\n  默默：那就先慢一点。我在旁边，不催你。',
    )
  })

  test('skips incomplete example pairs when projecting source', () => {
    const draft = createPersonaDraft(persona)
    const edited = withDraftExamplePairs(draft, [
      { user: '今天有点累', assistant: '我在。' },
      { user: '只有用户', assistant: '' },
    ])

    expect(projectDraftToSource(edited)).toContain('# 例对话')
    expect(projectDraftToSource(edited)).toContain('用户：今天有点累')
    expect(projectDraftToSource(edited)).not.toContain('只有用户')
  })

  test('preserves a newly added blank example pair for the editor', () => {
    const draft = createPersonaDraft(persona)
    const edited = withDraftExamplePairs(draft, [{ user: '', assistant: '' }])

    expect(getDraftExamplePairs(edited)).toEqual([{ user: '', assistant: '' }])
    expect(projectDraftToSource(edited)).not.toContain('# 例对话')
  })

  test('falls back to simple examples when structured examples are empty', () => {
    const draft = createPersonaDraft(persona)
    const withSimpleExamples = {
      ...draft,
      simple: {
        ...draft.simple,
        examples: ['用户：你好\n默默：我在。'],
      },
      structured: {
        ...draft.structured,
        examples: '',
      },
    }

    expect(getDraftExamplePairs(withSimpleExamples)).toEqual([{ user: '你好', assistant: '我在。' }])
  })

  test('validates empty and partial example pairs as warnings', () => {
    const draft = createPersonaDraft(persona)
    const emptyDiagnostics = validatePersonaDraft({
      ...draft,
      simple: { ...draft.simple, examples: [] },
      structured: { ...draft.structured, examples: '' },
    })

    expect(emptyDiagnostics).toContainEqual({
      code: 'examples.empty',
      severity: 'warning',
      message: '建议补充 1-3 条示例对话；没有示例时，AI 只能靠身份与规则判断语气。',
    })

    const partialDiagnostics = validatePersonaDraft(
      withDraftExamplePairs(draft, [{ user: '今天有点累', assistant: '' }]),
    )
    expect(partialDiagnostics.some((d) => d.code === 'examples.partial')).toBe(true)
    expect(partialDiagnostics.some((d) => d.code === 'examples.empty')).toBe(false)
  })
})
