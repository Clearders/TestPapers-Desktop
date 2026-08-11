import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import {
  assertMutationBase,
  assertRestoreUnlock,
  parseBackupPreflight,
  parseBackupRecoveryKey,
  parseBackupSchedule,
  parseDirectorySelection,
  parseEngineContext,
  parseImportInspection,
  parseJobSummary,
  parseQuestion,
  parseQuestionRevisions,
  parseQuestionSearchPage,
  type BackupPreflight,
  type BackupRecoveryKey,
  type BackupSchedule,
  type DirectorySelection,
  type EngineContext,
  type ImportInspection,
  type JobSummary,
  type LayoutDensity,
  type MutationBase,
  type PaperExportFormat,
  type Question,
  type QuestionInput,
  type QuestionOrder,
  type QuestionRevision,
  type QuestionSearchPage,
  type QuestionSearchRequest,
  type RestoreUnlock
} from '../../types/localEngine'

export const LOCAL_ENGINE_EVENTS = {
  stateChanged: 'testpapers://engine/state-changed',
  jobUpdated: 'testpapers://jobs/updated',
  maintenanceChanged: 'testpapers://workspace/maintenance-changed'
} as const

export interface BackupScheduleInput {
  enabled: boolean
  destinationSelectionId?: string
  intervalMinutes: number
  retentionDays: number
  encryptionMode: 'keychain' | 'none'
  recoveryKeyConfirmed?: boolean
}

export interface LocalEngineBridge {
  getEngineContext(): Promise<EngineContext>
  retryEngineStart(): Promise<EngineContext>
  getJob(id: string): Promise<JobSummary>
  cancelJob(id: string): Promise<JobSummary>
  searchQuestions(request: QuestionSearchRequest): Promise<QuestionSearchPage>
  getQuestion(id: string): Promise<Question>
  createQuestion(input: QuestionInput): Promise<Question>
  updateQuestion(id: string, base: MutationBase, input: QuestionInput): Promise<Question>
  deleteQuestion(id: string, base: MutationBase): Promise<Question>
  restoreQuestion(id: string, base: MutationBase): Promise<Question>
  addQuestionImage(questionId: string, caption?: string): Promise<Question | null>
  listQuestionRevisions(id: string): Promise<QuestionRevision[]>
  revertQuestion(id: string, base: MutationBase, version: number): Promise<Question>
  selectQuestionImport(): Promise<ImportInspection | null>
  commitQuestionImport(importId: string): Promise<JobSummary>
  discardQuestionImport(importId: string): Promise<void>
  generatePaper(input: {
    title: string
    subjects: string[]
    durationMinutes: number
    totalMarks: string
    difficultyCoefficient: number
    questionTypes: Array<{ questionType: string; count: number }>
    requiredTags: string[]
    preferredTags: string[]
    seed: number
  }): Promise<JobSummary>
  exportPaper(input: {
    paperId: string
    format: PaperExportFormat
    includeAnswers: boolean
    questionOrder: QuestionOrder
    layoutDensity: LayoutDensity
  }): Promise<JobSummary>
  getBackupSchedule(): Promise<BackupSchedule>
  prepareBackupEncryption(): Promise<BackupRecoveryKey>
  selectBackupDestination(): Promise<DirectorySelection | null>
  configureBackupSchedule(input: BackupScheduleInput): Promise<BackupSchedule>
  createWorkspaceBackup(input: { encryptionMode: 'passphrase' | 'none'; passphrase?: string }): Promise<JobSummary>
  selectWorkspaceRestore(unlock?: RestoreUnlock): Promise<BackupPreflight | null>
  commitWorkspaceRestore(restoreId: string): Promise<JobSummary>
  discardWorkspaceRestore(restoreId: string): Promise<void>
  selectDataDirectory(): Promise<DirectorySelection | null>
  migrateWorkspaceDataDirectory(selectionId: string): Promise<JobSummary>
  onEngineStateChanged(handler: (context: EngineContext) => void): Promise<UnlistenFn>
  onJobUpdated(handler: (job: JobSummary) => void): Promise<UnlistenFn>
  onMaintenanceChanged(handler: (context: EngineContext) => void): Promise<UnlistenFn>
}

async function listenFor<T>(eventName: string, parser: (value: unknown) => T, handler: (value: T) => void) {
  return listen(eventName, event => handler(parser(event.payload)))
}

async function invoke<T = unknown>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args)
  } catch (cause) {
    if (typeof cause === 'object' && cause !== null && 'message' in cause && typeof cause.message === 'string') {
      throw new Error(cause.message, { cause })
    }
    throw cause
  }
}

export const tauriLocalEngineBridge: LocalEngineBridge = {
  async getEngineContext() {
    return parseEngineContext(await invoke('get_engine_context'))
  },
  async retryEngineStart() {
    return parseEngineContext(await invoke('retry_engine_start'))
  },
  async getJob(id) {
    return parseJobSummary(await invoke('get_job', { id }))
  },
  async cancelJob(id) {
    return parseJobSummary(await invoke('cancel_job', { id }))
  },
  async searchQuestions(request) {
    return parseQuestionSearchPage(await invoke('search_questions', { request }))
  },
  async getQuestion(id) {
    return parseQuestion(await invoke('get_question', { id }))
  },
  async createQuestion(input) {
    return parseQuestion(await invoke('create_question', { input }))
  },
  async updateQuestion(id, base, input) {
    assertMutationBase(base)
    return parseQuestion(await invoke('update_question', { id, base, input }))
  },
  async deleteQuestion(id, base) {
    assertMutationBase(base)
    return parseQuestion(await invoke('delete_question', { id, base }))
  },
  async restoreQuestion(id, base) {
    assertMutationBase(base)
    return parseQuestion(await invoke('restore_question', { id, base }))
  },
  async addQuestionImage(questionId, caption) {
    const value = await invoke('add_question_image', {
      questionId,
      caption: caption?.trim() || null
    })
    return value === null ? null : parseQuestion(value)
  },
  async listQuestionRevisions(id) {
    return parseQuestionRevisions(await invoke('list_question_revisions', { id }))
  },
  async revertQuestion(id, base, version) {
    assertMutationBase(base)
    return parseQuestion(await invoke('revert_question', { id, base, version }))
  },
  async selectQuestionImport() {
    const value = await invoke('select_question_import')
    return value === null ? null : parseImportInspection(value)
  },
  async commitQuestionImport(importId) {
    return parseJobSummary(await invoke('commit_question_import', { importId }))
  },
  async discardQuestionImport(importId) {
    await invoke('discard_question_import', { importId })
  },
  async generatePaper(input) {
    return parseJobSummary(await invoke('generate_paper', { input }))
  },
  async exportPaper(input) {
    return parseJobSummary(await invoke('export_paper', { input }))
  },
  async getBackupSchedule() {
    return parseBackupSchedule(await invoke('get_backup_schedule'))
  },
  async prepareBackupEncryption() {
    return parseBackupRecoveryKey(await invoke('prepare_backup_encryption'))
  },
  async selectBackupDestination() {
    const value = await invoke('select_backup_destination')
    return value === null ? null : parseDirectorySelection(value)
  },
  async configureBackupSchedule(input) {
    return parseBackupSchedule(await invoke('configure_backup_schedule', { input }))
  },
  async createWorkspaceBackup(input) {
    return parseJobSummary(await invoke('create_workspace_backup', { input }))
  },
  async selectWorkspaceRestore(unlock = {}) {
    assertRestoreUnlock(unlock)
    const value = await invoke('select_workspace_restore', { unlock })
    return value === null ? null : parseBackupPreflight(value)
  },
  async commitWorkspaceRestore(restoreId) {
    return parseJobSummary(await invoke('commit_workspace_restore', { restoreId }))
  },
  async discardWorkspaceRestore(restoreId) {
    await invoke('discard_workspace_restore', { restoreId })
  },
  async selectDataDirectory() {
    const value = await invoke('select_data_directory')
    return value === null ? null : parseDirectorySelection(value)
  },
  async migrateWorkspaceDataDirectory(selectionId) {
    return parseJobSummary(await invoke('migrate_workspace_data_directory', { selectionId }))
  },
  onEngineStateChanged(handler) {
    return listenFor(LOCAL_ENGINE_EVENTS.stateChanged, parseEngineContext, handler)
  },
  onJobUpdated(handler) {
    return listenFor(LOCAL_ENGINE_EVENTS.jobUpdated, parseJobSummary, handler)
  },
  onMaintenanceChanged(handler) {
    return listenFor(LOCAL_ENGINE_EVENTS.maintenanceChanged, parseEngineContext, handler)
  }
}
