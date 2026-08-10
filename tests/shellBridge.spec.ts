import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { tauriShellBridge } from '../src/infrastructure/tauri/shellBridge'

afterEach(() => clearMocks())

describe('typed Tauri shell bridge', () => {
  it('uses the allowlisted theme command and validates its result', async () => {
    const commands = vi.fn((command: string, args?: unknown) => {
      expect(command).toBe('set_theme_preference')
      expect(args).toEqual({ preference: 'dark' })
      return {
        schemaVersion: 1,
        appVersion: '0.1.0',
        platform: 'linux',
        theme: { schemaVersion: 1, preference: 'dark', effective: 'dark' },
        closeBehavior: 'ask',
        integrations: { trayAvailable: true, settingsPersistent: true },
        warnings: []
      }
    })
    mockIPC(commands)
    await expect(tauriShellBridge.setThemePreference('dark')).resolves.toMatchObject({ theme: { effective: 'dark' } })
    expect(commands).toHaveBeenCalledOnce()
  })

  it('sends a close request ID and decision with the exact camelCase arguments', async () => {
    const commands = vi.fn((command: string, args?: unknown) => {
      expect(command).toBe('resolve_close_request')
      expect(args).toEqual({ requestId: 14, decision: 'tray' })
      return { schemaVersion: 1, outcome: 'hiding' }
    })
    mockIPC(commands)
    await expect(tauriShellBridge.resolveCloseRequest(14, 'tray')).resolves.toEqual({
      schemaVersion: 1,
      outcome: 'hiding'
    })
  })
})
