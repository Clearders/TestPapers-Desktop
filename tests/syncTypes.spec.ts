import { describe, expect, it } from 'vitest'

import fixtures from '../contracts/sync-client-state.fixtures.json'
import {
  parseSyncStatusChangedEvent,
  parseSyncStatusSnapshot,
  SYNC_STATUS_PRESENTATION
} from '../src/types/sync'

describe('platform-neutral Sync client state', () => {
  it('consumes every fixed cross-platform state and event fixture', () => {
    expect(fixtures.states.map(state => parseSyncStatusSnapshot(state).status)).toEqual([
      'synced', 'pending', 'syncing', 'offline', 'retrying', 'conflict', 'authRequired', 'failed'
    ])
    expect(parseSyncStatusChangedEvent(fixtures.event).state.status).toBe('authRequired')
  })

  it('gives every state an explanation and an actionable next step', () => {
    for (const state of fixtures.states) {
      const presentation = SYNC_STATUS_PRESENTATION[parseSyncStatusSnapshot(state).status]
      expect(presentation.title).not.toBe('')
      expect(presentation.description).not.toBe('')
      expect(presentation.action).not.toBe('')
    }
  })

  it('rejects unknown states, secrets, and malformed entity records', () => {
    const valid = fixtures.states[0]
    expect(() => parseSyncStatusSnapshot({ ...valid, status: 'paused' })).toThrow('Invalid Sync client state')
    expect(() => parseSyncStatusSnapshot({ ...valid, accessToken: 'secret' })).toThrow('Invalid Sync client state')
    expect(() => parseSyncStatusSnapshot({
      ...valid,
      entities: [{ entityType: 'question', entityId: '', status: 'pending' }]
    })).toThrow('Invalid Sync entity state')
  })
})
