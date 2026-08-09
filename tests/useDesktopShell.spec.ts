import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { describe, expect, it, vi } from 'vitest'

import { createDesktopShell } from '../src/application/useDesktopShell'
import type { ShellBridge } from '../src/infrastructure/tauri/shellBridge'
import type { CloseRequestedEvent, ThemeState } from '../src/types/shell'

function createBridge() {
  let closeHandler: ((event: CloseRequestedEvent) => void) | undefined
  let themeHandler: ((event: ThemeState) => void) | undefined
  const unlisten = vi.fn()
  const context = {
    schemaVersion: 1 as const,
    appVersion: '0.1.0',
    platform: 'windows' as const,
    theme: { schemaVersion: 1 as const, preference: 'system' as const, effective: 'light' as const },
    closeBehavior: 'ask' as const,
    integrations: { trayAvailable: true, settingsPersistent: true },
    warnings: []
  }
  const bridge: ShellBridge = {
    getContext: vi.fn().mockResolvedValue(context),
    frontendReady: vi.fn().mockResolvedValue(undefined),
    setThemePreference: vi.fn().mockResolvedValue(context),
    setCloseBehavior: vi.fn().mockResolvedValue(context),
    resolveCloseRequest: vi.fn().mockResolvedValue({ schemaVersion: 1, outcome: 'cancelled' }),
    previewQuestionImportDialog: vi.fn(),
    previewPaperExportDialog: vi.fn(),
    onCloseRequested: vi.fn(async handler => { closeHandler = handler; return unlisten }),
    onPreferencesRequested: vi.fn(async () => unlisten),
    onThemeChanged: vi.fn(async handler => { themeHandler = handler; return unlisten }),
    onDialogPreviewed: vi.fn(async () => unlisten)
  }
  return {
    bridge,
    context,
    unlisten,
    emitClose: () => closeHandler?.({ schemaVersion: 1, requestId: 7 }),
    emitDarkTheme: () => themeHandler?.({ schemaVersion: 1, preference: 'system', effective: 'dark' })
  }
}

describe('Desktop shell lifecycle', () => {
  it('subscribes before showing the window and releases listeners on unmount', async () => {
    const fake = createBridge()
    let shell: ReturnType<typeof createDesktopShell> | undefined
    const wrapper = mount(defineComponent({
      setup() {
        shell = createDesktopShell(fake.bridge)
        return () => h('div')
      }
    }))
    await flushPromises()
    expect(fake.bridge.frontendReady).toHaveBeenCalledOnce()
    expect(shell?.context.value).toEqual(fake.context)
    fake.emitClose()
    expect(shell?.closeRequest.value?.requestId).toBe(7)
    await shell?.resolveClose('cancel')
    expect(fake.bridge.resolveCloseRequest).toHaveBeenCalledWith(7, 'cancel')
    expect(shell?.closeRequest.value).toBeNull()
    fake.emitDarkTheme()
    expect(shell?.effectiveTheme.value).toBe('dark')
    expect(document.documentElement.dataset.theme).toBe('dark')
    window.dispatchEvent(new Event('beforeunload'))
    expect(fake.unlisten).toHaveBeenCalledTimes(4)
    wrapper.unmount()
    expect(fake.unlisten).toHaveBeenCalledTimes(4)
  })
})
