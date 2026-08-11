import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { tauriLocalEngineBridge } from '../src/infrastructure/tauri/localEngineBridge'

const ID = '018f8f2a-7c20-7abc-8def-1234567890ab'
const HASH = 'b'.repeat(64)

afterEach(() => clearMocks())

describe('typed Local Engine bridge', () => {
  it('uses a command-specific search contract', async () => {
    const commands = vi.fn((command: string, args?: unknown) => {
      expect(command).toBe('search_questions')
      expect(args).toEqual({ request: { query: 'algebra', limit: 50 } })
      return { schemaVersion: 1, items: [], nextCursor: null }
    })
    mockIPC(commands)
    await expect(tauriLocalEngineBridge.searchQuestions({ query: 'algebra', limit: 50 })).resolves.toEqual({
      schemaVersion: 1,
      items: [],
      nextCursor: null
    })
  })

  it('requires optimistic concurrency metadata for updates', async () => {
    const commands = vi.fn(() => ({
      schemaVersion: 1,
      id: ID,
      ownerId: '018f8f2a-7c20-7abc-8def-1234567890ac',
      replicationScope: 'local_private',
      version: 2,
      contentHash: HASH,
      createdAt: 1,
      updatedAt: 2,
      deletedAt: null,
      type: 'short_answer',
      subjects: ['Math'],
      difficulty: 'easy',
      tags: [],
      text: 'Updated',
      options: [],
      answer: 'Answer',
      hasLatex: false,
      source: null,
      essayBlankSpace: null,
      scoreWeight: '1',
      images: []
    }))
    mockIPC(commands)
    await expect(tauriLocalEngineBridge.updateQuestion(ID, {
      baseVersion: 1,
      baseContentHash: HASH
    }, {
      type: 'short_answer', subjects: ['Math'], difficulty: 'easy', tags: [], text: 'Updated', answer: 'Answer'
    })).resolves.toMatchObject({ version: 2 })
    expect(commands).toHaveBeenCalledWith('update_question', expect.objectContaining({
      id: ID,
      base: { baseVersion: 1, baseContentHash: HASH }
    }))
  })

  it('adds a question image through the native picker and returns metadata only', async () => {
    const commands = vi.fn((command: string, args?: unknown) => {
      expect(command).toBe('add_question_image')
      expect(args).toEqual({ questionId: ID, caption: 'Coordinate plane' })
      return {
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
        subjects: ['Math'],
        difficulty: 'easy',
        tags: [],
        text: 'Read the graph.',
        options: [],
        answer: 'Answer',
        hasLatex: false,
        source: null,
        essayBlankSpace: null,
        scoreWeight: '1',
        images: [{
          attachmentId: '018f8f2a-7c20-7abc-8def-1234567890ad',
          fileName: 'graph.webp',
          mediaType: 'image/webp',
          byteSize: 1024,
          caption: 'Coordinate plane'
        }]
      }
    })
    mockIPC(commands)
    const question = await tauriLocalEngineBridge.addQuestionImage(ID, ' Coordinate plane ')
    expect(question?.images[0]).toEqual(expect.objectContaining({ fileName: 'graph.webp', byteSize: 1024 }))
    expect(question?.images[0]).not.toHaveProperty('path')
  })

  it('uses the CLE-28 encryption, restore, and data-directory command envelopes', async () => {
    const job = {
      schemaVersion: 1,
      id: ID,
      kind: 'restore',
      state: 'queued',
      completedUnits: 0,
      totalUnits: null,
      phase: 'queued',
      cancellable: true,
      result: null,
      error: null
    }
    const preflight = {
      schemaVersion: 1,
      restoreId: ID,
      displayName: 'workspace.tpbackup.age',
      workspaceId: '018f8f2a-7c20-7abc-8def-1234567890ac',
      appVersion: '0.1.0',
      schemaVersionFound: 1,
      createdAt: 1_700_000_000_000_000,
      encrypted: true,
      requiresRecoveryKey: false,
      compatible: true,
      warnings: []
    }
    const directory = {
      schemaVersion: 1,
      selectionId: '018f8f2a-7c20-7abc-8def-1234567890ad',
      displayName: 'TestPapers Workspace',
      writable: true,
      availableBytes: 1_000_000
    }
    const commands = vi.fn((command: string) => {
      switch (command) {
        case 'prepare_backup_encryption':
          return { schemaVersion: 1, keyId: `workspace-${ID}`, recoveryKey: 'AGE-SECRET-KEY-1EXAMPLE' }
        case 'select_workspace_restore': return preflight
        case 'commit_workspace_restore': return job
        case 'discard_workspace_restore': return undefined
        case 'select_data_directory': return directory
        case 'migrate_workspace_data_directory': return { ...job, kind: 'dataDirectoryMigration' }
        default: throw new Error(`Unexpected command: ${command}`)
      }
    })
    mockIPC(commands)

    await expect(tauriLocalEngineBridge.prepareBackupEncryption()).resolves.toMatchObject({ keyId: `workspace-${ID}` })
    await expect(tauriLocalEngineBridge.selectWorkspaceRestore({ recoveryKey: 'AGE-SECRET-KEY-1EXAMPLE' })).resolves.toEqual(preflight)
    await expect(tauriLocalEngineBridge.commitWorkspaceRestore(ID)).resolves.toMatchObject({ kind: 'restore' })
    await expect(tauriLocalEngineBridge.discardWorkspaceRestore(ID)).resolves.toBeUndefined()
    await expect(tauriLocalEngineBridge.selectDataDirectory()).resolves.toEqual(directory)
    await expect(tauriLocalEngineBridge.migrateWorkspaceDataDirectory(directory.selectionId)).resolves.toMatchObject({ kind: 'dataDirectoryMigration' })

    expect(commands).toHaveBeenCalledWith('select_workspace_restore', { unlock: { recoveryKey: 'AGE-SECRET-KEY-1EXAMPLE' } })
    expect(commands).toHaveBeenCalledWith('commit_workspace_restore', { restoreId: ID })
    expect(commands).toHaveBeenCalledWith('discard_workspace_restore', { restoreId: ID })
    expect(commands).toHaveBeenCalledWith('migrate_workspace_data_directory', { selectionId: directory.selectionId })
  })
})
