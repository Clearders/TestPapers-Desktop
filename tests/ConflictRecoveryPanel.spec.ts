import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import fixtures from '../contracts/sync-client-state.fixtures.json'
import ConflictRecoveryPanel from '../src/components/ConflictRecoveryPanel.vue'
import type { SyncBridge } from '../src/infrastructure/tauri/syncBridge'
import type { SyncConflictRecoveryRecord } from '../src/types/syncConflict'
import { parseSyncStatusSnapshot } from '../src/types/sync'

const conflict: SyncConflictRecoveryRecord = {
  conflictId: 'conflict-1', entityType: 'draft', entityId: 'draft-1', reason: 'divergentContent',
  state: 'unresolved', createdAt: 1_700_000_000_000_000, updatedAt: 1_700_000_001_000_000,
  base: { schemaVersion: 1, version: 1, contentHash: 'base', mutationKind: 'update', tombstone: false, payload: { title: 'Base', body: 'Same' }, deviceId: 'device-a', modifiedAt: '2026-08-01T00:00:00Z' },
  local: { schemaVersion: 1, version: 1, contentHash: 'local', mutationKind: 'update', tombstone: false, payload: { title: 'Local', body: 'Same' }, deviceId: 'desktop', modifiedAt: '2026-08-02T00:00:00Z' },
  cloud: { schemaVersion: 1, version: 2, contentHash: 'cloud', mutationKind: 'update', tombstone: false, payload: { title: 'Cloud', body: 'Same' }, deviceId: 'web', modifiedAt: '2026-08-03T00:00:00Z' },
  resolutions: []
}

function bridge(): SyncBridge {
  const state = parseSyncStatusSnapshot(fixtures.states[0])
  return {
    configureSession: vi.fn().mockResolvedValue(state), getStatus: vi.fn().mockResolvedValue(state),
    pause: vi.fn().mockResolvedValue(state), resume: vi.fn().mockResolvedValue(state),
    syncNow: vi.fn().mockResolvedValue(state), retry: vi.fn().mockResolvedValue(state),
    listConflicts: vi.fn().mockResolvedValue([conflict]), resolveConflict: vi.fn().mockResolvedValue(state),
    onStatusChanged: vi.fn(async () => () => {})
  }
}

beforeEach(() => {
  const values = new Map<string, string>()
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    removeItem: (key: string) => values.delete(key),
    clear: () => values.clear(),
    key: (index: number) => [...values.keys()][index] ?? null,
    get length() { return values.size }
  } satisfies Storage)
  localStorage.clear()
  vi.stubGlobal('confirm', vi.fn(() => true))
})

describe('Desktop conflict recovery', () => {
  it('distinguishes personal Sync and renders a three-way comparison', async () => {
    const wrapper = mount(ConflictRecoveryPanel, { props: { bridge: bridge() } })
    await flushPromises()
    expect(wrapper.text()).toContain('Separate from realtime collaboration')
    expect(wrapper.text()).toContain('Baseline')
    expect(wrapper.text()).toContain('Local')
    expect(wrapper.text()).toContain('Cloud')
    expect(wrapper.text()).toContain('Diverged')
  })

  it('restores an unfinished manual merge after remount and queues it durably', async () => {
    const firstBridge = bridge()
    const first = mount(ConflictRecoveryPanel, { props: { bridge: firstBridge } })
    await flushPromises()
    await first.get('textarea').setValue('{"title":"Recovered merge"}')
    first.unmount()

    const secondBridge = bridge()
    const second = mount(ConflictRecoveryPanel, { props: { bridge: secondBridge } })
    await flushPromises()
    expect((second.get('textarea').element as HTMLTextAreaElement).value).toContain('Recovered merge')
    await second.get('.button--primary').trigger('click')
    await flushPromises()
    expect(secondBridge.resolveConflict).toHaveBeenCalledWith('conflict-1', expect.objectContaining({
      action: 'manualMerge', payload: { title: 'Recovered merge' }, currentVersion: 2, currentContentHash: 'cloud'
    }))
    expect(localStorage.getItem('testpapers.sync-conflict-draft.conflict-1')).toBeNull()
  })
})
