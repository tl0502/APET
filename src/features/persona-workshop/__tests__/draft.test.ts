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

  test('estimates tokens from source length with a stable rounded value', () => {
    const draft = createPersonaDraft(persona)

    expect(estimateDraftTokens(draft)).toBeGreaterThan(20)
    expect(estimateDraftTokens(draft)).toBeLessThan(300)
  })
})
