import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, it, vi } from 'vitest'

import fixtures from '../contracts/sync-client-state.fixtures.json'
import { tauriSyncBridge } from '../src/infrastructure/tauri/syncBridge'

afterEach(() => clearMocks())

describe('typed Sync control bridge', () => {
  it('hands a native authentication session to the allowlisted boundary without persisting it', async () => {
    const input = {
      baseUrl: 'https://api.testpapers.dev',
      accountId: '018f8f2a-7c20-7abc-8def-1234567890ab',
      deviceId: '018f8f2a-7c20-7abc-8def-1234567890ac',
      accessToken: 'header.payload.signature'
    }
    const commands = vi.fn(() => fixtures.states[0])
    mockIPC(commands)
    await expect(tauriSyncBridge.configureSession(input)).resolves.toMatchObject({ status: 'synced' })
    expect(commands).toHaveBeenCalledWith('configure_sync_session', { input })
  })
  it.each([
    ['getStatus', 'get_sync_status'],
    ['pause', 'pause_sync'],
    ['resume', 'resume_sync'],
    ['syncNow', 'sync_now'],
    ['retry', 'retry_sync']
  ] as const)('maps %s to its allowlisted native command', async (method, command) => {
    const commands = vi.fn(() => fixtures.states[0])
    mockIPC(commands)
    await expect(tauriSyncBridge[method]()).resolves.toMatchObject({ status: 'synced' })
    expect(commands).toHaveBeenCalledWith(command, {})
  })

  it('rejects an unversioned native response', async () => {
    mockIPC(() => ({ status: 'synced' }))
    await expect(tauriSyncBridge.getStatus()).rejects.toThrow('Invalid Sync client state')
  })
})
