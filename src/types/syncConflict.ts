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
