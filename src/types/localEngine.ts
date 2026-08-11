export const LOCAL_SCHEMA_VERSION = 1 as const

export type EngineState = 'starting' | 'ready' | 'recovering' | 'degraded' | 'stopping'
export type SuggestedAction = 'retry' | 'restore' | 'chooseDirectory' | 'restartApp' | 'contactSupport'
export type JobKind = 'import' | 'generation' | 'export' | 'backup' | 'restore' | 'dataDirectoryMigration'
export type JobState = 'queued' | 'running' | 'cancelling' | 'completed' | 'failed' | 'cancelled'
export type QuestionType = 'single_choice' | 'multiple_choice' | 'true_false' | 'blank' | 'short_answer' | 'essay'
export type QuestionDifficulty = 'easy' | 'medium' | 'hard'
export type ReplicationScope = 'local_private' | 'cloud_synced' | 'collaborative_shared'
export type PaperStatus = 'draft' | 'published'
export type QuestionOrder = 'paper' | 'categorized'
export type LayoutDensity = 'auto' | 'normal' | 'compact' | 'dense'
export type PaperExportFormat = 'docx' | 'tex' | 'pdf'

export interface EngineError {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  code: string
  message: string
  recoverable: boolean
  suggestedAction: SuggestedAction
}

export interface EngineContext {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  state: EngineState
  generation: number
  workspaceId: string | null
  databaseAvailable: boolean
  maintenanceMode: boolean
  lastError: EngineError | null
}

export interface JobSummary {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  id: string
  kind: JobKind
  state: JobState
  completedUnits: number
  totalUnits: number | null
  phase: string
  cancellable: boolean
  result: Record<string, unknown> | null
  error: EngineError | null
}

export interface MutationBase {
  baseVersion: number
  baseContentHash: string
}

export interface QuestionImage {
  attachmentId: string
  fileName: string
  mediaType: string
  byteSize: number
  caption: string | null
}

export interface EssayBlankSpace {
  lines: number
  lineHeight: number
}

export interface Question {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  id: string
  ownerId: string
  replicationScope: ReplicationScope
  version: number
  contentHash: string
  createdAt: number
  updatedAt: number
  deletedAt: number | null
  type: QuestionType
  subjects: string[]
  difficulty: QuestionDifficulty
  tags: string[]
  text: string
  options: string[]
  answer: string | string[]
  hasLatex: boolean
  source: string | null
  essayBlankSpace: EssayBlankSpace | null
  scoreWeight: string
  images: QuestionImage[]
}

export interface QuestionInput {
  type: QuestionType
  subjects: string[]
  difficulty: QuestionDifficulty
  tags: string[]
  text: string
  options?: string[]
  answer: string | string[]
  hasLatex?: boolean
  source?: string | null
  essayBlankSpace?: EssayBlankSpace | null
  scoreWeight?: string
}

export interface QuestionSearchRequest {
  query?: string
  subjects?: string[]
  tags?: string[]
  types?: QuestionType[]
  difficulties?: QuestionDifficulty[]
  includeDeleted?: boolean
  cursor?: string | null
  limit?: number
}

export interface QuestionSearchPage {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  items: Question[]
  nextCursor: string | null
}

export interface QuestionRevision {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  entityId: string
  version: number
  contentHash: string
  action: 'create' | 'update' | 'delete' | 'restore' | 'revert'
  acceptedAt: number
  snapshot: Question
}

export interface ImportInspection {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  importId: string
  displayName: string
  validRows: number
  invalidRows: number
  errors: Array<{ rowNumber: number; messages: string[] }>
}

export interface PaperItem {
  id: string
  questionId: string | null
  order: number
  marks: string | null
  questionSnapshot: QuestionInput
}

export interface Paper {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  id: string
  ownerId: string
  replicationScope: ReplicationScope
  version: number
  contentHash: string
  createdAt: number
  updatedAt: number
  deletedAt: number | null
  title: string
  subject: string
  durationMinutes: number
  totalMarks: string
  status: PaperStatus
  items: PaperItem[]
}

export interface BackupSchedule {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  enabled: boolean
  destinationDisplayName: string | null
  intervalMinutes: number
  retentionDays: number
  encryptionMode: 'keychain' | 'none'
  lastSuccessfulAt: number | null
  nextDueAt: number | null
}

export interface DirectorySelection {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  selectionId: string
  displayName: string
  writable: boolean
  availableBytes: number | null
}

export interface BackupPreflight {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  restoreId: string
  displayName: string
  workspaceId: string
  appVersion: string
  schemaVersionFound: number
  createdAt: number
  encrypted: boolean
  requiresRecoveryKey: boolean
  compatible: boolean
  warnings: string[]
}

export interface BackupRecoveryKey {
  schemaVersion: typeof LOCAL_SCHEMA_VERSION
  keyId: string
  recoveryKey: string
}

export interface RestoreUnlock {
  passphrase?: string
  recoveryKey?: string
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function isOneOf<T extends string>(value: unknown, choices: readonly T[]): value is T {
  return typeof value === 'string' && choices.includes(value as T)
}

function isUuid(value: unknown) {
  return typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value)
}

function parseEngineError(value: unknown): EngineError | null {
  if (value === null || value === undefined) return null
  if (
    !isRecord(value) ||
    value.schemaVersion !== LOCAL_SCHEMA_VERSION ||
    typeof value.code !== 'string' ||
    typeof value.message !== 'string' ||
    typeof value.recoverable !== 'boolean' ||
    !isOneOf(value.suggestedAction, ['retry', 'restore', 'chooseDirectory', 'restartApp', 'contactSupport'])
  ) throw new Error('Invalid Local Engine error')
  return value as unknown as EngineError
}

export function parseEngineContext(value: unknown): EngineContext {
  if (
    !isRecord(value) ||
    value.schemaVersion !== LOCAL_SCHEMA_VERSION ||
    !isOneOf(value.state, ['starting', 'ready', 'recovering', 'degraded', 'stopping']) ||
    !Number.isSafeInteger(value.generation) || Number(value.generation) < 0 ||
    !(value.workspaceId === null || isUuid(value.workspaceId)) ||
    typeof value.databaseAvailable !== 'boolean' ||
    typeof value.maintenanceMode !== 'boolean'
  ) throw new Error('Invalid context received from the Local Engine')
  parseEngineError(value.lastError)
  return value as unknown as EngineContext
}

export function parseJobSummary(value: unknown): JobSummary {
  if (
    !isRecord(value) ||
    value.schemaVersion !== LOCAL_SCHEMA_VERSION ||
    !isUuid(value.id) ||
    !isOneOf(value.kind, ['import', 'generation', 'export', 'backup', 'restore', 'dataDirectoryMigration']) ||
    !isOneOf(value.state, ['queued', 'running', 'cancelling', 'completed', 'failed', 'cancelled']) ||
    !Number.isSafeInteger(value.completedUnits) || Number(value.completedUnits) < 0 ||
    !(value.totalUnits === null || (Number.isSafeInteger(value.totalUnits) && Number(value.totalUnits) >= 0)) ||
    typeof value.phase !== 'string' ||
    typeof value.cancellable !== 'boolean' ||
    !(value.result === null || isRecord(value.result))
  ) throw new Error('Invalid Local Engine job')
  parseEngineError(value.error)
  return value as unknown as JobSummary
}

function parseQuestionInput(value: unknown): QuestionInput {
  if (
    !isRecord(value) ||
    !isOneOf(value.type, ['single_choice', 'multiple_choice', 'true_false', 'blank', 'short_answer', 'essay']) ||
    !Array.isArray(value.subjects) || !value.subjects.every(item => typeof item === 'string') ||
    !isOneOf(value.difficulty, ['easy', 'medium', 'hard']) ||
    !Array.isArray(value.tags) || !value.tags.every(item => typeof item === 'string') ||
    typeof value.text !== 'string' ||
    !(typeof value.answer === 'string' || (Array.isArray(value.answer) && value.answer.every(item => typeof item === 'string')))
  ) throw new Error('Invalid question received from the Local Engine')
  return value as unknown as QuestionInput
}

export function parseQuestion(value: unknown): Question {
  if (
    !isRecord(value) ||
    value.schemaVersion !== LOCAL_SCHEMA_VERSION ||
    !isUuid(value.id) || !isUuid(value.ownerId) ||
    !isOneOf(value.replicationScope, ['local_private', 'cloud_synced', 'collaborative_shared']) ||
    !Number.isSafeInteger(value.version) || Number(value.version) < 1 ||
    typeof value.contentHash !== 'string' || !/^[0-9a-f]{64}$/.test(value.contentHash) ||
    !Number.isSafeInteger(value.createdAt) || !Number.isSafeInteger(value.updatedAt) ||
    !(value.deletedAt === null || Number.isSafeInteger(value.deletedAt))
  ) throw new Error('Invalid question received from the Local Engine')
  parseQuestionInput(value)
  if (
    !Array.isArray(value.options) || !value.options.every(item => typeof item === 'string') ||
    typeof value.hasLatex !== 'boolean' ||
    !(value.source === null || typeof value.source === 'string') ||
    typeof value.scoreWeight !== 'string' ||
    !Array.isArray(value.images)
  ) throw new Error('Invalid question received from the Local Engine')
  for (const image of value.images) {
    if (
      !isRecord(image) || !isUuid(image.attachmentId) ||
      typeof image.fileName !== 'string' || !image.fileName || /[\\/]/.test(image.fileName) ||
      !isOneOf(image.mediaType, ['image/png', 'image/jpeg', 'image/gif', 'image/webp']) ||
      !Number.isSafeInteger(image.byteSize) || Number(image.byteSize) < 0 || Number(image.byteSize) > 30 * 1024 * 1024 ||
      !(image.caption === null || typeof image.caption === 'string')
    ) throw new Error('Invalid question image received from the Local Engine')
  }
  return value as unknown as Question
}

export function parseQuestionSearchPage(value: unknown): QuestionSearchPage {
  if (
    !isRecord(value) || value.schemaVersion !== LOCAL_SCHEMA_VERSION ||
    !Array.isArray(value.items) ||
    !(value.nextCursor === null || typeof value.nextCursor === 'string')
  ) throw new Error('Invalid question page received from the Local Engine')
  value.items.forEach(parseQuestion)
  return value as unknown as QuestionSearchPage
}

export function parseQuestionRevisions(value: unknown): QuestionRevision[] {
  if (!Array.isArray(value)) throw new Error('Invalid revision list received from the Local Engine')
  return value.map(item => {
    if (
      !isRecord(item) || item.schemaVersion !== LOCAL_SCHEMA_VERSION || !isUuid(item.entityId) ||
      !Number.isSafeInteger(item.version) || !isOneOf(item.action, ['create', 'update', 'delete', 'restore', 'revert']) ||
      !Number.isSafeInteger(item.acceptedAt) || typeof item.contentHash !== 'string'
    ) throw new Error('Invalid question revision received from the Local Engine')
    parseQuestion(item.snapshot)
    return item as unknown as QuestionRevision
  })
}

export function parseImportInspection(value: unknown): ImportInspection {
  if (
    !isRecord(value) || value.schemaVersion !== LOCAL_SCHEMA_VERSION || !isUuid(value.importId) ||
    typeof value.displayName !== 'string' || /[\\/]/.test(value.displayName) ||
    !Number.isSafeInteger(value.validRows) || !Number.isSafeInteger(value.invalidRows) ||
    !Array.isArray(value.errors)
  ) throw new Error('Invalid import inspection received from the Local Engine')
  return value as unknown as ImportInspection
}

export function parseBackupSchedule(value: unknown): BackupSchedule {
  if (
    !isRecord(value) || value.schemaVersion !== LOCAL_SCHEMA_VERSION || typeof value.enabled !== 'boolean' ||
    !(value.destinationDisplayName === null || (typeof value.destinationDisplayName === 'string' && !/[\\/]/.test(value.destinationDisplayName))) ||
    !Number.isSafeInteger(value.intervalMinutes) || Number(value.intervalMinutes) < 60 || Number(value.intervalMinutes) > 43200 ||
    !Number.isSafeInteger(value.retentionDays) || Number(value.retentionDays) < 1 || Number(value.retentionDays) > 3650 ||
    !isOneOf(value.encryptionMode, ['keychain', 'none'])
  ) throw new Error('Invalid backup schedule received from the Local Engine')
  return value as unknown as BackupSchedule
}

export function parseDirectorySelection(value: unknown): DirectorySelection {
  if (
    !isRecord(value) || value.schemaVersion !== LOCAL_SCHEMA_VERSION || !isUuid(value.selectionId) ||
    typeof value.displayName !== 'string' || !value.displayName || /[\\/]/.test(value.displayName) ||
    typeof value.writable !== 'boolean' ||
    !(value.availableBytes === null || (Number.isSafeInteger(value.availableBytes) && Number(value.availableBytes) >= 0))
  ) throw new Error('Invalid directory selection received from the Local Engine')
  return value as unknown as DirectorySelection
}

export function parseBackupPreflight(value: unknown): BackupPreflight {
  if (
    !isRecord(value) || value.schemaVersion !== LOCAL_SCHEMA_VERSION || !isUuid(value.restoreId) ||
    typeof value.displayName !== 'string' || !value.displayName || /[\\/]/.test(value.displayName) ||
    !isUuid(value.workspaceId) || typeof value.appVersion !== 'string' || !value.appVersion ||
    !Number.isSafeInteger(value.schemaVersionFound) || Number(value.schemaVersionFound) < 0 ||
    !Number.isSafeInteger(value.createdAt) || typeof value.encrypted !== 'boolean' ||
    typeof value.requiresRecoveryKey !== 'boolean' || typeof value.compatible !== 'boolean' ||
    !Array.isArray(value.warnings) || !value.warnings.every(warning => typeof warning === 'string')
  ) throw new Error('Invalid restore preflight received from the Local Engine')
  return value as unknown as BackupPreflight
}

export function parseBackupRecoveryKey(value: unknown): BackupRecoveryKey {
  if (
    !isRecord(value) || value.schemaVersion !== LOCAL_SCHEMA_VERSION ||
    typeof value.keyId !== 'string' || !/^workspace-[0-9a-f-]{36}$/.test(value.keyId) ||
    typeof value.recoveryKey !== 'string' || !value.recoveryKey.trim() || value.recoveryKey.length > 4096
  ) throw new Error('Invalid backup recovery key received from the Local Engine')
  return value as unknown as BackupRecoveryKey
}

export function assertRestoreUnlock(value: RestoreUnlock) {
  const passphrase = value.passphrase ?? ''
  const recoveryKey = value.recoveryKey ?? ''
  if (typeof passphrase !== 'string' || typeof recoveryKey !== 'string' || passphrase.length > 4096 || recoveryKey.length > 4096) {
    throw new Error('Restore credentials are invalid')
  }
  if (passphrase.length > 0 && recoveryKey.trim().length > 0) {
    throw new Error('Use either a restore passphrase or a recovery key, not both')
  }
}

export function assertMutationBase(value: MutationBase) {
  if (!Number.isSafeInteger(value.baseVersion) || value.baseVersion < 1 || !/^[0-9a-f]{64}$/.test(value.baseContentHash)) {
    throw new Error('A valid base version and content hash are required')
  }
}
