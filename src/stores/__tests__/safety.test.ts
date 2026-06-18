import { setActivePinia, createPinia } from 'pinia'
import { beforeEach, describe, expect, it, vi, type Mock } from 'vitest'

import { useSafetyStore } from '../safety'

vi.mock('@/services/safety', () => ({
  SAFETY_SCOPES: ['prefixInjection', 'userInput', 'streamToken', 'finalOutput'],
  getSafetyPolicy: vi.fn(),
  setSafetyScope: vi.fn(),
}))

import { getSafetyPolicy, setSafetyScope } from '@/services/safety'

const mockGetSafetyPolicy = getSafetyPolicy as unknown as Mock
const mockSetSafetyScope = setSafetyScope as unknown as Mock

beforeEach(() => {
  setActivePinia(createPinia())
  vi.clearAllMocks()
  mockGetSafetyPolicy.mockResolvedValue({
    prefixInjection: false,
    userInput: false,
    streamToken: false,
    finalOutput: false,
  })
  mockSetSafetyScope.mockResolvedValue(undefined)
})

describe('safety store', () => {
  it('loads all four scopes from backend snapshot', async () => {
    mockGetSafetyPolicy.mockResolvedValueOnce({
      prefixInjection: true,
      userInput: false,
      streamToken: true,
      finalOutput: false,
    })

    const store = useSafetyStore()
    await store.load()

    expect(store.loaded).toBe(true)
    expect(store.scopes.prefixInjection).toBe(true)
    expect(store.scopes.userInput).toBe(false)
    expect(store.scopes.streamToken).toBe(true)
    expect(store.scopes.finalOutput).toBe(false)
  })

  it('optimistically toggles a scope and persists it', async () => {
    const store = useSafetyStore()
    await store.load()

    await store.setScope('finalOutput', true)

    expect(store.scopes.finalOutput).toBe(true)
    expect(store.savingScopes.finalOutput).toBe(false)
    expect(mockSetSafetyScope).toHaveBeenCalledWith('finalOutput', true)
  })

  it('rolls back optimistic toggle when persistence fails', async () => {
    mockSetSafetyScope.mockRejectedValueOnce(new Error('db failed'))

    const store = useSafetyStore()
    await store.load()

    await expect(store.setScope('streamToken', true)).rejects.toThrow('db failed')
    expect(store.scopes.streamToken).toBe(false)
    expect(store.savingScopes.streamToken).toBe(false)
  })

  it('tracks saving state per scope', async () => {
    let resolveSet: (() => void) | undefined
    mockSetSafetyScope.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveSet = resolve
      }),
    )

    const store = useSafetyStore()
    await store.load()

    const pending = store.setScope('prefixInjection', true)

    expect(store.savingScopes.prefixInjection).toBe(true)
    expect(store.savingScopes.finalOutput).toBe(false)

    resolveSet?.()
    await pending

    expect(store.savingScopes.prefixInjection).toBe(false)
  })
})
