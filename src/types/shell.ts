export const SHELL_SCHEMA_VERSION = 1 as const

export type ThemePreference = 'system' | 'light' | 'dark'
export type EffectiveTheme = 'light' | 'dark'
export type CloseBehavior = 'ask' | 'quit' | 'tray'
export type CloseDecision = 'quit' | 'tray' | 'cancel'
export type DesktopPlatform = 'windows' | 'linux' | 'macos'
export type ExportFormat = 'docx' | 'tex'
export type DialogPreviewKind = 'questionImport' | 'paperDocx' | 'paperTex'

export interface ThemeState {
  schemaVersion: typeof SHELL_SCHEMA_VERSION
  preference: ThemePreference
  effective: EffectiveTheme
}

export interface IntegrationStatus {
  trayAvailable: boolean
  settingsPersistent: boolean
}

export interface ShellContext {
  schemaVersion: typeof SHELL_SCHEMA_VERSION
  appVersion: string
  platform: DesktopPlatform
  theme: ThemeState
  closeBehavior: CloseBehavior
  integrations: IntegrationStatus
  warnings: string[]
}

export interface CloseRequestedEvent {
  schemaVersion: typeof SHELL_SCHEMA_VERSION
  requestId: number
}

export interface ShellEvent {
  schemaVersion: typeof SHELL_SCHEMA_VERSION
}

export interface CloseResolution {
  schemaVersion: typeof SHELL_SCHEMA_VERSION
  outcome: 'cancelled' | 'hiding' | 'exiting'
}

export interface DialogPreviewResult {
  schemaVersion: typeof SHELL_SCHEMA_VERSION
  kind: DialogPreviewKind
  cancelled: boolean
  selectionCount: number
  displayNames: string[]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isOneOf<T extends string>(value: unknown, choices: readonly T[]): value is T {
  return typeof value === 'string' && choices.includes(value as T)
}

function hasSchemaVersion(value: Record<string, unknown>): boolean {
  return value.schemaVersion === SHELL_SCHEMA_VERSION
}

export function parseThemeState(value: unknown): ThemeState {
  if (
    !isRecord(value) ||
    !hasSchemaVersion(value) ||
    !isOneOf(value.preference, ['system', 'light', 'dark']) ||
    !isOneOf(value.effective, ['light', 'dark'])
  ) {
    throw new Error('Invalid theme state received from the Desktop shell')
  }
  return value as unknown as ThemeState
}

export function parseShellContext(value: unknown): ShellContext {
  if (
    !isRecord(value) ||
    !hasSchemaVersion(value) ||
    typeof value.appVersion !== 'string' ||
    !isOneOf(value.platform, ['windows', 'linux', 'macos']) ||
    !isOneOf(value.closeBehavior, ['ask', 'quit', 'tray']) ||
    !isRecord(value.integrations) ||
    typeof value.integrations.trayAvailable !== 'boolean' ||
    typeof value.integrations.settingsPersistent !== 'boolean' ||
    !Array.isArray(value.warnings) ||
    !value.warnings.every(item => typeof item === 'string')
  ) {
    throw new Error('Invalid context received from the Desktop shell')
  }
  parseThemeState(value.theme)
  return value as unknown as ShellContext
}

export function parseCloseRequestedEvent(value: unknown): CloseRequestedEvent {
  if (!isRecord(value) || !hasSchemaVersion(value) || !Number.isSafeInteger(value.requestId) || Number(value.requestId) < 1) {
    throw new Error('Invalid close request received from the Desktop shell')
  }
  return value as unknown as CloseRequestedEvent
}

export function parseShellEvent(value: unknown): ShellEvent {
  if (!isRecord(value) || !hasSchemaVersion(value)) {
    throw new Error('Invalid event received from the Desktop shell')
  }
  return value as unknown as ShellEvent
}

export function parseCloseResolution(value: unknown): CloseResolution {
  if (!isRecord(value) || !hasSchemaVersion(value) || !isOneOf(value.outcome, ['cancelled', 'hiding', 'exiting'])) {
    throw new Error('Invalid close resolution received from the Desktop shell')
  }
  return value as unknown as CloseResolution
}

export function parseDialogPreviewResult(value: unknown): DialogPreviewResult {
  if (
    !isRecord(value) ||
    !hasSchemaVersion(value) ||
    !isOneOf(value.kind, ['questionImport', 'paperDocx', 'paperTex']) ||
    typeof value.cancelled !== 'boolean' ||
    !Number.isSafeInteger(value.selectionCount) ||
    Number(value.selectionCount) < 0 ||
    !Array.isArray(value.displayNames) ||
    !value.displayNames.every(item => typeof item === 'string' && item.length > 0 && !/[\\/]/.test(item)) ||
    value.selectionCount !== value.displayNames.length ||
    value.cancelled !== (value.selectionCount === 0)
  ) {
    throw new Error('Invalid dialog result received from the Desktop shell')
  }
  return value as unknown as DialogPreviewResult
}
