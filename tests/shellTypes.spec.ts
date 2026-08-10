import { describe, expect, it } from 'vitest'

import { parseDialogPreviewResult, parseShellContext } from '../src/types/shell'

describe('shell wire contract', () => {
  it('accepts the versioned shell context', () => {
    expect(parseShellContext({
      schemaVersion: 1,
      appVersion: '0.1.0',
      platform: 'windows',
      theme: { schemaVersion: 1, preference: 'system', effective: 'dark' },
      closeBehavior: 'ask',
      integrations: { trayAvailable: true, settingsPersistent: true },
      warnings: []
    }).theme.effective).toBe('dark')
  })

  it('rejects file-system paths or unknown fields disguised as dialog names', () => {
    expect(() => parseDialogPreviewResult({
      schemaVersion: 1,
      kind: 'questionImport',
      cancelled: false,
      selectionCount: 1,
      displayNames: ['C:\\private\\questions.csv']
    })).toThrow('Invalid dialog result')
  })

  it('rejects inconsistent dialog counts and cancellation state', () => {
    expect(() => parseDialogPreviewResult({
      schemaVersion: 1,
      kind: 'paperDocx',
      cancelled: true,
      selectionCount: 1,
      displayNames: ['paper.docx']
    })).toThrow('Invalid dialog result')
  })
})
