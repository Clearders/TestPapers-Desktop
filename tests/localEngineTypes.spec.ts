import { describe, expect, it } from 'vitest'

import {
  parseBackupPreflight,
  parseBackupRecoveryKey,
  parseBackupSchedule,
  parseEngineContext,
  parseJobSummary,
  parseQuestion,
  parseQuestionSearchPage
} from '../src/types/localEngine'

const ID = '018f8f2a-7c20-7abc-8def-1234567890ab'
const HASH = 'a'.repeat(64)

describe('Local Engine wire contract', () => {
  it('accepts startup and ready contexts without leaking a workspace path', () => {
    expect(parseEngineContext({
      schemaVersion: 1,
      state: 'starting',
      generation: 0,
      workspaceId: null,
      databaseAvailable: false,
      maintenanceMode: false,
      lastError: null
    }).workspaceId).toBeNull()

    const ready = parseEngineContext({
      schemaVersion: 1,
      state: 'ready',
      generation: 1,
      workspaceId: ID,
      databaseAvailable: true,
      maintenanceMode: false,
      lastError: null
    })
    expect(ready.state).toBe('ready')
    expect(ready).not.toHaveProperty('workspacePath')
  })

  it('rejects malformed Engine errors and absolute paths disguised as IDs', () => {
    expect(() => parseEngineContext({
      schemaVersion: 1,
      state: 'degraded',
      generation: 1,
      workspaceId: 'C:\\private\\workspace',
      databaseAvailable: false,
      maintenanceMode: false,
      lastError: {
        schemaVersion: 1,
        code: 'workspace_locked',
        message: 'Locked',
        recoverable: true,
        suggestedAction: 'retry'
      }
    })).toThrow('Invalid context')
  })

  it('validates progress, question pages, and configured backup bounds', () => {
    expect(parseJobSummary({
      schemaVersion: 1,
      id: ID,
      kind: 'import',
      state: 'running',
      completedUnits: 8,
      totalUnits: 10,
      phase: 'Committing questions',
      cancellable: true,
      result: null,
      error: null
    }).completedUnits).toBe(8)

    const question = {
      schemaVersion: 1,
      id: ID,
      ownerId: '018f8f2a-7c20-7abc-8def-1234567890ac',
      replicationScope: 'local_private',
      version: 1,
      contentHash: HASH,
      createdAt: 1,
      updatedAt: 1,
      deletedAt: null,
      type: 'short_answer',
      subjects: ['Mathematics'],
      difficulty: 'medium',
      tags: ['algebra'],
      text: 'Solve $x+1=2$.',
      options: [],
      answer: 'x=1',
      hasLatex: true,
      source: null,
      essayBlankSpace: null,
      scoreWeight: '1',
      images: []
    }
    expect(parseQuestionSearchPage({ schemaVersion: 1, items: [question], nextCursor: null }).items).toHaveLength(1)

    const questionWithImage = {
      ...question,
      images: [{
        attachmentId: '018f8f2a-7c20-7abc-8def-1234567890ad',
        fileName: 'diagram.png',
        mediaType: 'image/png',
        byteSize: 2048,
        caption: null
      }]
    }
    expect(parseQuestion(questionWithImage).images[0]?.fileName).toBe('diagram.png')
    expect(() => parseQuestion({
      ...questionWithImage,
      images: [{ ...questionWithImage.images[0], fileName: 'C:\\private\\diagram.png' }]
    })).toThrow('Invalid question image')

    expect(parseBackupSchedule({
      schemaVersion: 1,
      enabled: false,
      destinationDisplayName: null,
      intervalMinutes: 1440,
      retentionDays: 30,
      encryptionMode: 'keychain',
      lastSuccessfulAt: null,
      nextDueAt: null
    }).intervalMinutes).toBe(1440)

    expect(() => parseBackupSchedule({
      schemaVersion: 1,
      enabled: true,
      destinationDisplayName: 'backups',
      intervalMinutes: 10,
      retentionDays: 30,
      encryptionMode: 'none',
      lastSuccessfulAt: null,
      nextDueAt: null
    })).toThrow('Invalid backup schedule')
  })

  it('validates recovery-key and restore-preflight envelopes without paths', () => {
    expect(parseBackupRecoveryKey({
      schemaVersion: 1,
      keyId: `workspace-${ID}`,
      recoveryKey: 'AGE-SECRET-KEY-1EXAMPLE'
    }).keyId).toBe(`workspace-${ID}`)

    const preflight = parseBackupPreflight({
      schemaVersion: 1,
      restoreId: ID,
      displayName: 'TestPapers-backup.tpbackup',
      workspaceId: '018f8f2a-7c20-7abc-8def-1234567890ac',
      appVersion: '0.1.0',
      schemaVersionFound: 1,
      createdAt: 1_700_000_000_000_000,
      encrypted: true,
      requiresRecoveryKey: false,
      compatible: true,
      warnings: []
    })
    expect(preflight.compatible).toBe(true)
    expect(preflight).not.toHaveProperty('sourcePath')
    expect(() => parseBackupPreflight({ ...preflight, displayName: 'C:\\private\\backup.tpbackup' })).toThrow('Invalid restore preflight')
  })
})
