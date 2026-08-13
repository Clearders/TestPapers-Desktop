export const SYNC_CLIENT_STATE_SCHEMA_VERSION = 1 as const
export const SYNC_PROTOCOL_VERSION = 1 as const

export type SyncStatus = 'synced' | 'pending' | 'syncing' | 'offline' | 'retrying' | 'conflict' | 'authRequired' | 'failed'
export type SyncPhase = 'idle' | 'pull' | 'apply' | 'ack' | 'push' | 'settle'
export type SyncRecommendedAction = 'none' | 'wait' | 'resume' | 'syncNow' | 'retry' | 'signIn' | 'resolveConflict' | 'reviewFailure'
export type SyncEntityType = 'question' | 'paper' | 'draft' | 'attachment' | 'comment' | 'favorite' | 'setting'

export interface SyncEntityStatus {
  entityType: SyncEntityType
  entityId: string
  status: SyncStatus
}

export interface SyncStatusSnapshot {
  schemaVersion: typeof SYNC_CLIENT_STATE_SCHEMA_VERSION
  protocolVersion: typeof SYNC_PROTOCOL_VERSION
  accountId: string | null
  deviceId: string | null
  status: SyncStatus
  paused: boolean
  phase: SyncPhase
  pendingCount: number
  retryingCount: number
  conflictCount: number
  failedCount: number
  lastCompletedAt: number | null
  lastErrorCode: string | null
  recommendedAction: SyncRecommendedAction
  canPause: boolean
  canResume: boolean
  canSyncNow: boolean
  canRetry: boolean
  entities: SyncEntityStatus[]
}

export interface SyncStatusChangedEvent {
  schemaVersion: typeof SYNC_CLIENT_STATE_SCHEMA_VERSION
  type: 'sync.statusChanged'
  occurredAt: number
  state: SyncStatusSnapshot
}

export const SYNC_STATUS_PRESENTATION: Record<SyncStatus, { title: string; description: string; action: string }> = {
  synced: { title: 'Synced', description: 'All accepted changes are safely stored on this device and in Cloud.', action: 'You can sync again or pause automatic sync.' },
  pending: { title: 'Changes waiting', description: 'Your edits are saved locally and waiting to be sent.', action: 'Choose Sync now, or keep editing while offline.' },
  syncing: { title: 'Syncing', description: 'Desktop is applying and sending changes in the background.', action: 'Keep editing; the current cycle does not lock the workspace.' },
  offline: { title: 'Offline', description: 'Cloud is unavailable. Your local edits remain safe on this device.', action: 'Reconnect and retry, or pause sync until later.' },
  retrying: { title: 'Retry scheduled', description: 'A retryable request is waiting for its safe backoff window.', action: 'Wait for automatic retry or choose Retry now.' },
  conflict: { title: 'Conflict needs review', description: 'Local and Cloud snapshots were both preserved; neither version was overwritten.', action: 'Review the affected items before choosing a version.' },
  authRequired: { title: 'Sign in required', description: 'Sync is stopped because no valid native session is available.', action: 'Open Account settings to sign in. Local editing remains available.' },
  failed: { title: 'Sync needs attention', description: 'At least one operation could not be safely accepted.', action: 'Review the stable error code, then retry without changing the queued payload.' }
}

const STATUSES: readonly SyncStatus[] = ['synced', 'pending', 'syncing', 'offline', 'retrying', 'conflict', 'authRequired', 'failed']
const PHASES: readonly SyncPhase[] = ['idle', 'pull', 'apply', 'ack', 'push', 'settle']
const ACTIONS: readonly SyncRecommendedAction[] = ['none', 'wait', 'resume', 'syncNow', 'retry', 'signIn', 'resolveConflict', 'reviewFailure']
const ENTITY_TYPES: readonly SyncEntityType[] = ['question', 'paper', 'draft', 'attachment', 'comment', 'favorite', 'setting']
const SNAPSHOT_KEYS = [
  'schemaVersion', 'protocolVersion', 'accountId', 'deviceId', 'status', 'paused', 'phase',
  'pendingCount', 'retryingCount', 'conflictCount', 'failedCount', 'lastCompletedAt',
  'lastErrorCode', 'recommendedAction', 'canPause', 'canResume', 'canSyncNow', 'canRetry', 'entities'
] as const

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function oneOf<T extends string>(value: unknown, choices: readonly T[]): value is T {
  return typeof value === 'string' && choices.includes(value as T)
}

function uuidOrNull(value: unknown) {
  return value === null || (typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(value))
}

function count(value: unknown) {
  return Number.isSafeInteger(value) && Number(value) >= 0
}

function hasOnlyKeys(value: Record<string, unknown>, keys: readonly string[]) {
  return Object.keys(value).length === keys.length && Object.keys(value).every(key => keys.some(candidate => candidate === key))
}

export function parseSyncStatusSnapshot(value: unknown): SyncStatusSnapshot {
  if (
    !record(value) || !hasOnlyKeys(value, SNAPSHOT_KEYS) || value.schemaVersion !== SYNC_CLIENT_STATE_SCHEMA_VERSION || value.protocolVersion !== SYNC_PROTOCOL_VERSION ||
    !uuidOrNull(value.accountId) || !uuidOrNull(value.deviceId) || !oneOf(value.status, STATUSES) ||
    typeof value.paused !== 'boolean' || !oneOf(value.phase, PHASES) ||
    !count(value.pendingCount) || !count(value.retryingCount) || !count(value.conflictCount) || !count(value.failedCount) ||
    !(value.lastCompletedAt === null || count(value.lastCompletedAt)) ||
    !(value.lastErrorCode === null || (typeof value.lastErrorCode === 'string' && /^[A-Z0-9_]+$/.test(value.lastErrorCode))) ||
    !oneOf(value.recommendedAction, ACTIONS) || typeof value.canPause !== 'boolean' || typeof value.canResume !== 'boolean' ||
    typeof value.canSyncNow !== 'boolean' || typeof value.canRetry !== 'boolean' || !Array.isArray(value.entities)
  ) throw new Error('Invalid Sync client state')
  for (const entity of value.entities) {
    if (!record(entity) || !hasOnlyKeys(entity, ['entityType', 'entityId', 'status']) || !oneOf(entity.entityType, ENTITY_TYPES) || typeof entity.entityId !== 'string' || !entity.entityId || !oneOf(entity.status, STATUSES)) {
      throw new Error('Invalid Sync entity state')
    }
  }
  return value as unknown as SyncStatusSnapshot
}

export function parseSyncStatusChangedEvent(value: unknown): SyncStatusChangedEvent {
  if (!record(value) || !hasOnlyKeys(value, ['schemaVersion', 'type', 'occurredAt', 'state']) || value.schemaVersion !== SYNC_CLIENT_STATE_SCHEMA_VERSION || value.type !== 'sync.statusChanged' || !count(value.occurredAt)) {
    throw new Error('Invalid Sync status event')
  }
  parseSyncStatusSnapshot(value.state)
  return value as unknown as SyncStatusChangedEvent
}
