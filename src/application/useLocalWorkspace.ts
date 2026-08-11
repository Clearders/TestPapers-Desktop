import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import {
  tauriLocalEngineBridge,
  type BackupScheduleInput,
  type LocalEngineBridge
} from '../infrastructure/tauri/localEngineBridge'
import type {
  BackupPreflight,
  BackupRecoveryKey,
  BackupSchedule,
  DirectorySelection,
  EngineContext,
  ImportInspection,
  JobSummary,
  PaperExportFormat,
  Question,
  QuestionInput,
  QuestionSearchRequest,
  RestoreUnlock
} from '../types/localEngine'

export type WorkspaceTab = 'questions' | 'papers' | 'backups'

export function createLocalWorkspace(bridge: LocalEngineBridge = tauriLocalEngineBridge) {
  const engine = ref<EngineContext | null>(null)
  const activeTab = ref<WorkspaceTab>('questions')
  const questions = ref<Question[]>([])
  const nextQuestionCursor = ref<string | null>(null)
  const selectedQuestion = ref<Question | null>(null)
  const importInspection = ref<ImportInspection | null>(null)
  const backupSchedule = ref<BackupSchedule | null>(null)
  const backupDestination = ref<DirectorySelection | null>(null)
  const backupRecoveryKey = ref<BackupRecoveryKey | null>(null)
  const restorePreflight = ref<BackupPreflight | null>(null)
  const dataDirectorySelection = ref<DirectorySelection | null>(null)
  const jobs = ref(new Map<string, JobSummary>())
  const query = ref('')
  const busy = ref(false)
  const error = ref('')
  const unlisteners: Array<() => void> = []
  let disposed = false

  const ready = computed(() => engine.value?.state === 'ready' && engine.value.databaseAvailable && !engine.value.maintenanceMode)
  const runningJobs = computed(() => [...jobs.value.values()].filter(job => ['queued', 'running', 'cancelling'].includes(job.state)))

  function rememberJob(job: JobSummary) {
    const next = new Map(jobs.value)
    next.set(job.id, job)
    jobs.value = next
  }

  async function retain(subscription: Promise<() => void>) {
    const unlisten = await subscription
    if (disposed) unlisten()
    else unlisteners.push(unlisten)
  }

  async function run<T>(operation: () => Promise<T>): Promise<T | undefined> {
    busy.value = true
    error.value = ''
    try {
      return await operation()
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
      return undefined
    } finally {
      busy.value = false
    }
  }

  async function initialise() {
    try {
      await Promise.all([
        retain(bridge.onEngineStateChanged(context => {
          engine.value = context
          if (context.state === 'ready' && context.databaseAvailable && !questions.value.length) void refreshQuestions()
        })),
        retain(bridge.onMaintenanceChanged(context => { engine.value = context })),
        retain(bridge.onJobUpdated(rememberJob))
      ])
      engine.value = await bridge.getEngineContext()
      if (ready.value) {
        await Promise.all([refreshQuestions(), loadBackupSchedule()])
      }
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
    }
  }

  async function retryEngine() {
    const context = await run(() => bridge.retryEngineStart())
    if (context) engine.value = context
  }

  async function refreshQuestions(request: QuestionSearchRequest = {}) {
    if (!ready.value) return
    const page = await run(() => bridge.searchQuestions({ query: query.value, limit: 50, ...request, cursor: null }))
    if (page) {
      questions.value = page.items
      nextQuestionCursor.value = page.nextCursor
      if (selectedQuestion.value) {
        selectedQuestion.value = page.items.find(item => item.id === selectedQuestion.value?.id) ?? null
      }
    }
  }

  async function loadMoreQuestions() {
    if (!ready.value || !nextQuestionCursor.value) return
    const page = await run(() => bridge.searchQuestions({ query: query.value, limit: 50, cursor: nextQuestionCursor.value }))
    if (page) {
      questions.value = [...questions.value, ...page.items]
      nextQuestionCursor.value = page.nextCursor
    }
  }

  async function saveQuestion(input: QuestionInput, existing?: Question | null) {
    const saved = existing
      ? await run(() => bridge.updateQuestion(existing.id, {
          baseVersion: existing.version,
          baseContentHash: existing.contentHash
        }, input))
      : await run(() => bridge.createQuestion(input))
    if (saved) {
      selectedQuestion.value = saved
      await refreshQuestions()
    }
    return saved
  }

  async function toggleQuestionDeleted(question: Question) {
    const base = { baseVersion: question.version, baseContentHash: question.contentHash }
    const saved = question.deletedAt === null
      ? await run(() => bridge.deleteQuestion(question.id, base))
      : await run(() => bridge.restoreQuestion(question.id, base))
    if (saved) await refreshQuestions({ includeDeleted: true })
  }

  async function addQuestionImage(questionId: string, caption?: string) {
    const updated = await run(() => bridge.addQuestionImage(questionId, caption))
    if (updated) {
      questions.value = questions.value.map(question => question.id === updated.id ? updated : question)
      if (selectedQuestion.value?.id === updated.id) selectedQuestion.value = updated
    }
    return updated
  }

  async function selectQuestionImport() {
    const inspection = await run(() => bridge.selectQuestionImport())
    if (inspection !== undefined) importInspection.value = inspection
  }

  async function commitQuestionImport() {
    const inspection = importInspection.value
    if (!inspection) return
    const job = await run(() => bridge.commitQuestionImport(inspection.importId))
    if (job) {
      rememberJob(job)
      importInspection.value = null
    }
  }

  async function discardQuestionImport() {
    const inspection = importInspection.value
    if (!inspection) return
    const completed = await run(async () => {
      await bridge.discardQuestionImport(inspection.importId)
      return true
    })
    if (completed) importInspection.value = null
  }

  async function generatePaper(input: Parameters<LocalEngineBridge['generatePaper']>[0]) {
    const job = await run(() => bridge.generatePaper(input))
    if (job) rememberJob(job)
  }

  async function exportPaper(paperId: string, format: PaperExportFormat) {
    const job = await run(() => bridge.exportPaper({
      paperId,
      format,
      includeAnswers: true,
      questionOrder: 'paper',
      layoutDensity: 'auto'
    }))
    if (job) rememberJob(job)
  }

  async function loadBackupSchedule() {
    if (!ready.value) return
    const schedule = await run(() => bridge.getBackupSchedule())
    if (schedule) backupSchedule.value = schedule
  }

  async function configureBackupSchedule(input: BackupScheduleInput) {
    const schedule = await run(() => bridge.configureBackupSchedule(input))
    if (schedule) {
      backupSchedule.value = schedule
      if (input.recoveryKeyConfirmed) dismissBackupRecoveryKey()
    }
  }

  async function prepareBackupEncryption() {
    backupRecoveryKey.value = null
    const prepared = await run(() => bridge.prepareBackupEncryption())
    if (prepared) backupRecoveryKey.value = prepared
    return prepared
  }

  function dismissBackupRecoveryKey() {
    backupRecoveryKey.value = null
  }

  async function selectBackupDestination() {
    const selection = await run(() => bridge.selectBackupDestination())
    if (selection !== undefined) backupDestination.value = selection
  }

  async function createBackup(encrypted: boolean, passphrase?: string) {
    const job = await run(() => bridge.createWorkspaceBackup({
      encryptionMode: encrypted ? 'passphrase' : 'none',
      ...(encrypted ? { passphrase } : {})
    }))
    if (job) rememberJob(job)
  }

  async function selectWorkspaceRestore(unlock: RestoreUnlock = {}) {
    const preflight = await run(() => bridge.selectWorkspaceRestore(unlock))
    if (preflight !== undefined) restorePreflight.value = preflight
    return preflight
  }

  async function commitWorkspaceRestore() {
    const preflight = restorePreflight.value
    if (!preflight || !preflight.compatible) return
    const job = await run(() => bridge.commitWorkspaceRestore(preflight.restoreId))
    if (job) {
      rememberJob(job)
      restorePreflight.value = null
    }
    return job
  }

  async function discardWorkspaceRestore() {
    const preflight = restorePreflight.value
    if (!preflight) return
    const discarded = await run(async () => {
      await bridge.discardWorkspaceRestore(preflight.restoreId)
      return true
    })
    if (discarded) restorePreflight.value = null
  }

  async function selectDataDirectory() {
    const selection = await run(() => bridge.selectDataDirectory())
    if (selection !== undefined) dataDirectorySelection.value = selection
    return selection
  }

  async function migrateWorkspaceDataDirectory() {
    const selection = dataDirectorySelection.value
    if (!selection || !selection.writable) return
    const job = await run(() => bridge.migrateWorkspaceDataDirectory(selection.selectionId))
    if (job) {
      rememberJob(job)
      dataDirectorySelection.value = null
    }
    return job
  }

  async function cancelJob(id: string) {
    const job = await run(() => bridge.cancelJob(id))
    if (job) rememberJob(job)
  }

  function teardown() {
    disposed = true
    backupRecoveryKey.value = null
    const stagedRestore = restorePreflight.value
    restorePreflight.value = null
    if (stagedRestore) void bridge.discardWorkspaceRestore(stagedRestore.restoreId).catch(() => undefined)
    for (const unlisten of unlisteners.splice(0)) unlisten()
  }

  onMounted(() => { void initialise() })
  onBeforeUnmount(teardown)

  return {
    engine,
    ready,
    activeTab,
    questions,
    nextQuestionCursor,
    selectedQuestion,
    importInspection,
    backupSchedule,
    backupDestination,
    backupRecoveryKey,
    restorePreflight,
    dataDirectorySelection,
    jobs,
    runningJobs,
    query,
    busy,
    error,
    retryEngine,
    refreshQuestions,
    loadMoreQuestions,
    saveQuestion,
    toggleQuestionDeleted,
    addQuestionImage,
    selectQuestionImport,
    commitQuestionImport,
    discardQuestionImport,
    generatePaper,
    exportPaper,
    loadBackupSchedule,
    configureBackupSchedule,
    prepareBackupEncryption,
    dismissBackupRecoveryKey,
    selectBackupDestination,
    createBackup,
    selectWorkspaceRestore,
    commitWorkspaceRestore,
    discardWorkspaceRestore,
    selectDataDirectory,
    migrateWorkspaceDataDirectory,
    cancelJob,
    teardown
  }
}
