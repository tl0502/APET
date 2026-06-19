/// <reference types="node" />

import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, it } from 'vitest'
import workspaceAppSource from '../WorkspaceApp.vue?raw'

const buttonsCssPath = path.resolve(process.cwd(), 'src/styles/buttons.css')

describe('WorkspaceApp chrome buttons', () => {
  it('uses stable icon elements instead of visible text glyphs for window controls', () => {
    const source = workspaceAppSource

    expect(source).toContain('aipet-chrome-icon aipet-chrome-icon--min')
    expect(source).toContain('aipet-chrome-icon aipet-chrome-icon--max')
    expect(source).toContain('aipet-chrome-icon aipet-chrome-icon--close')
    expect(source).not.toContain('>─</button>')
    expect(source).not.toContain('>□</button>')
    expect(source).not.toContain('>✕</button>')
  })

  it('defines chrome control icons in CSS so font fallback cannot shift their baselines', () => {
    const buttonsCss = readFileSync(buttonsCssPath, 'utf8')

    expect(buttonsCss).toContain('.aipet-chrome-icon--min::before')
    expect(buttonsCss).toContain('.aipet-chrome-icon--max::before')
    expect(buttonsCss).toContain('.aipet-chrome-icon--close::before')
  })
})
