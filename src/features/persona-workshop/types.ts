export type PersonaWorkshopMode = 'simple' | 'structured' | 'examples' | 'trial' | 'source'

export type PersonaDiagnosticSeverity = 'error' | 'warning' | 'info'

export interface PersonaDiagnostic {
  code: string
  severity: PersonaDiagnosticSeverity
  message: string
}

export interface PersonaExamplePair {
  user: string
  assistant: string
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
