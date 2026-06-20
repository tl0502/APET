import { flushPromises, mount } from '@vue/test-utils'
import { describe, expect, test, vi } from 'vitest'
import PersonaSnapshotHistory from '../PersonaSnapshotHistory.vue'
import { listPersonaSnapshots } from '@/services/persona'

vi.mock('@/services/persona', () => ({
  listPersonaSnapshots: vi.fn(),
}))

const stubs = {
  ElButton: {
    template: '<button :disabled="$attrs.disabled" @click="$emit(`click`)"><slot /></button>',
  },
  ElTag: { template: '<span><slot /></span>' },
}

const twoSnapshots = [
  { id: 102, version: '1.0.1', created_at: '2026-06-19T02:00:00Z', is_active: true },
  { id: 101, version: '1.0.0', created_at: '2026-06-19T01:00:00Z', is_active: false },
]

describe('PersonaSnapshotHistory', () => {
  test('fetches by personaId and flags the active snapshot', async () => {
    vi.mocked(listPersonaSnapshots).mockResolvedValueOnce(twoSnapshots)
    const wrapper = mount(PersonaSnapshotHistory, {
      props: { personaId: 'momo', activeSnapshotId: '102' },
      global: { stubs },
    })

    await flushPromises()

    expect(listPersonaSnapshots).toHaveBeenCalledWith('momo')
    expect(wrapper.text()).toContain('v1.0.1')
    expect(wrapper.text()).toContain('v1.0.0')
    expect(wrapper.text()).toContain('当前')

    const buttons = wrapper.findAll('button')
    const activeButton = buttons.find((button) => button.text() === '使用中')
    const restoreButton = buttons.find((button) => button.text() === '恢复')
    expect(activeButton?.attributes('disabled')).toBeDefined()
    expect(restoreButton).toBeTruthy()
  })

  test('emits restore with the snapshot id when 恢复 is clicked', async () => {
    vi.mocked(listPersonaSnapshots).mockResolvedValueOnce(twoSnapshots)
    const wrapper = mount(PersonaSnapshotHistory, {
      props: { personaId: 'momo', activeSnapshotId: '102' },
      global: { stubs },
    })

    await flushPromises()

    const restoreButton = wrapper.findAll('button').find((button) => button.text() === '恢复')
    await restoreButton?.trigger('click')

    expect(wrapper.emitted('restore')).toBeTruthy()
    expect(wrapper.emitted('restore')?.[0]).toEqual([101])
  })

  test('shows an empty state when the persona has no snapshots', async () => {
    vi.mocked(listPersonaSnapshots).mockResolvedValueOnce([])
    const wrapper = mount(PersonaSnapshotHistory, {
      props: { personaId: 'fresh-draft', activeSnapshotId: null },
      global: { stubs },
    })

    await flushPromises()

    expect(wrapper.text()).toContain('还没有已保存的快照')
  })

  test('surfaces an error message when the fetch fails', async () => {
    vi.mocked(listPersonaSnapshots).mockRejectedValueOnce(new Error('db down'))
    const wrapper = mount(PersonaSnapshotHistory, {
      props: { personaId: 'momo', activeSnapshotId: null },
      global: { stubs },
    })

    await flushPromises()

    expect(wrapper.text()).toContain('db down')
  })
})
