import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { describe, expect, it, vi } from 'vitest'

import fixtures from '../contracts/sync-client-state.fixtures.json'
import { createSyncControl } from '../src/application/useSyncControl'
import type { SyncBridge } from '../src/infrastructure/tauri/syncBridge'
import { parseSyncStatusSnapshot, type SyncStatusSnapshot } from '../src/types/sync'

describe('Sync control lifecycle', () => {
  it('subscribes, exposes controls, and releases its event listener', async () => {
    const authenticated = parseSyncStatusSnapshot(fixtures.states[0])
    const paused = { ...authenticated, paused: true, canPause: false, canResume: true, recommendedAction: 'resume' as const }
    let handler: ((state: SyncStatusSnapshot) => void) | undefined
    const unlisten = vi.fn()
    const bridge: SyncBridge = {
      configureSession: vi.fn().mockResolvedValue(authenticated),
      getStatus: vi.fn().mockResolvedValue(authenticated),
      pause: vi.fn().mockResolvedValue(paused),
      resume: vi.fn().mockResolvedValue(authenticated),
      syncNow: vi.fn().mockResolvedValue({ ...authenticated, status: 'syncing', phase: 'pull' }),
      retry: vi.fn().mockResolvedValue(authenticated),
      listConflicts: vi.fn().mockResolvedValue([]),
      resolveConflict: vi.fn().mockResolvedValue(authenticated),
      onStatusChanged: vi.fn(async listener => { handler = listener; return unlisten })
    }
    let control: ReturnType<typeof createSyncControl> | undefined
    const wrapper = mount(defineComponent({
      setup() {
        control = createSyncControl(bridge)
        return () => h('div')
      }
    }))
    await flushPromises()
    expect(control?.state.value?.status).toBe('synced')
    await control?.pause()
    expect(control?.state.value?.paused).toBe(true)
    handler?.(parseSyncStatusSnapshot(fixtures.states[5]))
    expect(control?.presentation.value.title).toBe('Conflict needs review')
    wrapper.unmount()
    expect(unlisten).toHaveBeenCalledOnce()
  })
})
