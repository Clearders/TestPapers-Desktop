import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import {
  parseCloseRequestedEvent,
  parseCloseResolution,
  parseDialogPreviewResult,
  parseShellContext,
  parseShellEvent,
  parseThemeState,
  type CloseBehavior,
  type CloseDecision,
  type CloseRequestedEvent,
  type CloseResolution,
  type DialogPreviewResult,
  type ExportFormat,
  type ShellContext,
  type ShellEvent,
  type ThemePreference,
  type ThemeState
} from '../../types/shell'

export const SHELL_EVENTS = {
  closeRequested: 'testpapers://shell/close-requested',
  preferencesRequested: 'testpapers://shell/preferences-requested',
  themeChanged: 'testpapers://shell/theme-changed',
  dialogPreviewed: 'testpapers://shell/dialog-previewed'
} as const

export interface ShellBridge {
  getContext(): Promise<ShellContext>
  frontendReady(): Promise<void>
  setThemePreference(preference: ThemePreference): Promise<ShellContext>
  setCloseBehavior(behavior: CloseBehavior): Promise<ShellContext>
  resolveCloseRequest(requestId: number, decision: CloseDecision): Promise<CloseResolution>
  previewQuestionImportDialog(): Promise<DialogPreviewResult>
  previewPaperExportDialog(format: ExportFormat): Promise<DialogPreviewResult>
  onCloseRequested(handler: (payload: CloseRequestedEvent) => void): Promise<UnlistenFn>
  onPreferencesRequested(handler: (payload: ShellEvent) => void): Promise<UnlistenFn>
  onThemeChanged(handler: (payload: ThemeState) => void): Promise<UnlistenFn>
  onDialogPreviewed(handler: (payload: DialogPreviewResult) => void): Promise<UnlistenFn>
}

async function listenFor<T>(
  eventName: string,
  parser: (value: unknown) => T,
  handler: (payload: T) => void
): Promise<UnlistenFn> {
  return listen(eventName, event => handler(parser(event.payload)))
}

export const tauriShellBridge: ShellBridge = {
  async getContext() {
    return parseShellContext(await invoke('get_shell_context'))
  },
  async frontendReady() {
    await invoke('frontend_ready')
  },
  async setThemePreference(preference) {
    return parseShellContext(await invoke('set_theme_preference', { preference }))
  },
  async setCloseBehavior(behavior) {
    return parseShellContext(await invoke('set_close_behavior', { behavior }))
  },
  async resolveCloseRequest(requestId, decision) {
    return parseCloseResolution(await invoke('resolve_close_request', { requestId, decision }))
  },
  async previewQuestionImportDialog() {
    return parseDialogPreviewResult(await invoke('preview_question_import_dialog'))
  },
  async previewPaperExportDialog(format) {
    return parseDialogPreviewResult(await invoke('preview_paper_export_dialog', { format }))
  },
  onCloseRequested(handler) {
    return listenFor(SHELL_EVENTS.closeRequested, parseCloseRequestedEvent, handler)
  },
  onPreferencesRequested(handler) {
    return listenFor(SHELL_EVENTS.preferencesRequested, parseShellEvent, handler)
  },
  onThemeChanged(handler) {
    return listenFor(SHELL_EVENTS.themeChanged, parseThemeState, handler)
  },
  onDialogPreviewed(handler) {
    return listenFor(SHELL_EVENTS.dialogPreviewed, parseDialogPreviewResult, handler)
  }
}
