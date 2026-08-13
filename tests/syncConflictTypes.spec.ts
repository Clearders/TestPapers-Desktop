import { describe, expect, it } from 'vitest'

import fixtures from '../contracts/sync-v1.fixtures.json'
import {
  classifySyncConflict,
  type SyncConflictMutationKind,
  type SyncConflictReason,
  type SyncResolutionAction
} from '../src/types/syncConflict'

describe('Sync v1 conflict semantics', () => {
  it('classifies every language-neutral fixture identically', () => {
    for (const testCase of fixtures.conflictCases) {
      const localHash = 'a'.repeat(64)
      const cloudHash = testCase.sameHash ? localHash : 'b'.repeat(64)
      expect(classifySyncConflict(
        testCase.localKind as SyncConflictMutationKind,
        testCase.cloudKind as SyncConflictMutationKind,
        localHash,
        cloudHash
      )).toBe(testCase.reason)
    }
  })

  it('keeps personal sync separate from realtime collaboration and pins every action', () => {
    expect(fixtures.conflictCases.every(testCase => ('origin' in testCase ? testCase.origin : 'personalSync') === 'personalSync')).toBe(true)
    expect(fixtures.resolutionCases.map(testCase => testCase.action as SyncResolutionAction)).toEqual([
      'keepLocal', 'useCloud', 'saveCopy', 'manualMerge', 'restoreVersion', 'undo'
    ])
    expect(new Set(fixtures.conflictCases.map(testCase => testCase.reason).filter(Boolean) as SyncConflictReason[])).toEqual(new Set([
      'concurrentCreate', 'divergentContent', 'tombstoneDivergence', 'restoreDivergence', 'renameDivergence'
    ]))
  })
})
