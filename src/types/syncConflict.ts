export type SyncConflictEntityType = 'question' | 'paper' | 'draft' | 'attachment' | 'comment' | 'favorite' | 'setting'
export type SyncConflictMutationKind = 'create' | 'update' | 'delete' | 'restore' | 'rename' | 'attach' | 'detach'
export type SyncConflictReason = 'concurrentCreate' | 'divergentContent' | 'tombstoneDivergence' | 'restoreDivergence' | 'renameDivergence'
export type SyncResolutionAction = 'keepLocal' | 'useCloud' | 'saveCopy' | 'manualMerge' | 'restoreVersion' | 'undo'

export interface SyncConflictSnapshot {
  schemaVersion: number
  version: number
  contentHash: string
  mutationKind: SyncConflictMutationKind
  tombstone: boolean
  payload: Record<string, unknown> | null
  deviceId: string
  modifiedAt: string
}

export interface SyncConflictRecord {
  protocolVersion: 1
  conflictId: string
  origin: 'personalSync'
  entityType: SyncConflictEntityType
  entityId: string
  reason: SyncConflictReason
  base: SyncConflictSnapshot | null
  local: SyncConflictSnapshot
  cloud: SyncConflictSnapshot
  detectedAt: string
}

export interface SyncConflictResolutionRecord {
  protocolVersion: 1
  resolutionId: string
  conflictId: string
  operationId: string
  action: SyncResolutionAction
  actorDeviceId: string
  acceptedVersion: number
  acceptedContentHash: string
  result: SyncConflictSnapshot
  newEntityId?: string
  undoesResolutionId?: string
  resolvedAt: string
}

export interface SyncConflictRecoveryRecord {
  conflictId: string
  entityType: SyncConflictEntityType
  entityId: string
  reason: SyncConflictReason
  base: SyncConflictSnapshot | null
  local: SyncConflictSnapshot
  cloud: SyncConflictSnapshot
  state: 'unresolved' | 'resolving' | 'resolved' | 'undone'
  createdAt: number
  updatedAt: number
  resolutions: SyncConflictResolutionRecord[]
}

export type SyncFieldChange = 'unchanged' | 'localOnly' | 'cloudOnly' | 'sameChange' | 'diverged'

export interface SyncFieldDifference {
  field: string
  base: unknown
  local: unknown
  cloud: unknown
  change: SyncFieldChange
}

function equivalent (left: unknown, right: unknown) { return JSON.stringify(left) === JSON.stringify(right) }

export function compareSyncPayloads (
  base: Record<string, unknown> | null,
  local: Record<string, unknown> | null,
  cloud: Record<string, unknown> | null
): SyncFieldDifference[] {
  const keys = [...new Set([...Object.keys(base ?? {}), ...Object.keys(local ?? {}), ...Object.keys(cloud ?? {})])].sort()
  return keys.map(field => {
    const baseValue = base?.[field]
    const localValue = local?.[field]
    const cloudValue = cloud?.[field]
    const localChanged = !equivalent(baseValue, localValue)
    const cloudChanged = !equivalent(baseValue, cloudValue)
    const change: SyncFieldChange = !localChanged && !cloudChanged
      ? 'unchanged'
      : localChanged && !cloudChanged
        ? 'localOnly'
        : !localChanged && cloudChanged
          ? 'cloudOnly'
          : equivalent(localValue, cloudValue) ? 'sameChange' : 'diverged'
    return { field, base: baseValue, local: localValue, cloud: cloudValue, change }
  })
}

export function classifySyncConflict (
  localKind: SyncConflictMutationKind,
  cloudKind: SyncConflictMutationKind,
  localHash: string,
  cloudHash: string
): SyncConflictReason | null {
  if (localHash === cloudHash) return null
  if (localKind === 'create' && cloudKind === 'create') return 'concurrentCreate'
  if (localKind === 'delete' || cloudKind === 'delete') return 'tombstoneDivergence'
  if (localKind === 'restore' || cloudKind === 'restore') return 'restoreDivergence'
  if (localKind === 'rename' || cloudKind === 'rename') return 'renameDivergence'
  return 'divergentContent'
}
