import { describe, expect, it } from 'vitest'
import messageListSource from '../MessageList.vue?raw'
import threadPaneSource from '../ChatThreadPane.vue?raw'

function cssBlock(source: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const match = source.match(new RegExp(`${escaped}\\s*\\{([\\s\\S]*?)\\n\\}`, 'm'))
  if (!match) throw new Error(`Missing CSS block for ${selector}`)
  return match[1]
}

describe('MessageList workspace layout CSS contract', () => {
  it('keeps the thread pane bounded so the message scroller can shrink with workspace', () => {
    const source = threadPaneSource

    const contentSurface = cssBlock(source, '.content-surface')
    expect(contentSurface).toContain('width: 100%;')
    expect(contentSurface).toContain('height: 100%;')
    expect(contentSurface).toContain('overflow: hidden;')

    const scrollSurface = cssBlock(source, '.message-scroll-surface')
    expect(scrollSurface).toContain('width: 100%;')
    expect(scrollSurface).toContain('overflow: hidden;')
  })

  it('makes chat messages and message groups size from their container, not intrinsic text width', () => {
    const source = messageListSource

    const messages = cssBlock(source, '.chat-messages')
    expect(messages).toContain('width: 100%;')
    expect(messages).toContain('height: 100%;')

    const group = cssBlock(source, '.msg-group')
    expect(group).toContain('width: min(100%, 680px);')
  })
})
