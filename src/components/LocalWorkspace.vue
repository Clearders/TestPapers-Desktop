<template>
  <section class="workspace" aria-labelledby="workspace-title">
    <div class="workspace-heading">
      <div>
        <span class="eyebrow">Local-first workspace · CLE-24–28</span>
        <h1 id="workspace-title">Author, generate, and protect papers offline.</h1>
      </div>
      <div class="engine-pill" :class="`engine-pill--${engine?.state ?? 'starting'}`" aria-live="polite">
        <span aria-hidden="true" />
        <div>
          <strong>{{ engineLabel }}</strong>
          <small v-if="engine?.workspaceId">Workspace {{ engine.workspaceId.slice(0, 8) }}</small>
          <small v-else>Preparing local workspace</small>
        </div>
      </div>
    </div>

    <div v-if="engine?.lastError" class="alert alert--error" role="alert">
      <strong>{{ engine.lastError.code }}</strong> — {{ engine.lastError.message }}
      <button v-if="engine.lastError.recoverable" class="button" type="button" :disabled="busy" @click="retryEngine">Retry Local Engine</button>
    </div>
    <p v-if="error" class="alert alert--error" role="alert">{{ error }}</p>

    <div v-if="!ready" class="workspace-unavailable">
      <AppIcon name="settings" />
      <h2>{{ engine?.maintenanceMode ? 'Workspace maintenance in progress' : 'Local Engine is not ready' }}</h2>
      <p>Editing remains locked until the local database is healthy. No Cloud service is required.</p>
    </div>

    <template v-else>
      <nav class="workspace-tabs" aria-label="Workspace sections">
        <button type="button" :class="{ active: activeTab === 'questions' }" @click="activeTab = 'questions'">Question bank</button>
        <button type="button" :class="{ active: activeTab === 'papers' }" @click="activeTab = 'papers'">Paper builder</button>
        <button type="button" :class="{ active: activeTab === 'backups' }" @click="activeTab = 'backups'">Backup & restore</button>
      </nav>

      <section v-if="activeTab === 'questions'" class="workspace-panel" aria-labelledby="questions-title">
        <div class="panel-toolbar">
          <div>
            <span class="card-kicker">CLE-26</span>
            <h2 id="questions-title">Offline question bank</h2>
          </div>
          <div class="toolbar-actions">
            <input v-model.trim="query" type="search" placeholder="Search text, subjects, or tags" @keyup.enter="refreshQuestions()">
            <button class="button" type="button" :disabled="busy" @click="refreshQuestions()">Search</button>
            <button class="button" type="button" :disabled="busy" @click="selectQuestionImport"><AppIcon name="upload" /> Import</button>
            <button class="button button--primary" type="button" @click="startNewQuestion">New question</button>
          </div>
        </div>

        <div class="question-layout">
          <div class="question-list" aria-live="polite">
            <button
              v-for="question in questions"
              :key="question.id"
              class="question-row"
              :class="{ active: selectedQuestion?.id === question.id, deleted: question.deletedAt !== null }"
              type="button"
              @click="selectQuestion(question)"
            >
              <span><strong>{{ typeLabel(question.type) }}</strong> · {{ question.difficulty }}</span>
              <p>{{ question.text }}</p>
              <small>v{{ question.version }} · {{ question.subjects.join(', ') }}</small>
            </button>
            <p v-if="!questions.length" class="empty-state">No questions match this view.</p>
            <button v-if="nextQuestionCursor" class="button load-more" type="button" :disabled="busy" @click="loadMoreQuestions">Load more</button>
          </div>

          <form class="question-editor" @submit.prevent="submitQuestion">
            <div class="editor-title">
              <div>
                <span class="card-kicker">{{ editingQuestion ? `Revision ${editingQuestion.version}` : 'New local question' }}</span>
                <h3>{{ editingQuestion ? 'Edit question' : 'Create question' }}</h3>
              </div>
              <button v-if="editingQuestion" class="button" type="button" :disabled="busy" @click="toggleQuestionDeleted(editingQuestion)">
                {{ editingQuestion.deletedAt === null ? 'Delete' : 'Restore' }}
              </button>
            </div>
            <div class="form-grid">
              <label>Type
                <select v-model="questionForm.type">
                  <option value="single_choice">Single choice</option>
                  <option value="multiple_choice">Multiple choice</option>
                  <option value="true_false">True / false</option>
                  <option value="blank">Fill blank</option>
                  <option value="short_answer">Short answer</option>
                  <option value="essay">Essay</option>
                </select>
              </label>
              <label>Difficulty
                <select v-model="questionForm.difficulty"><option>easy</option><option>medium</option><option>hard</option></select>
              </label>
            </div>
            <label>Subjects <input v-model="questionForm.subjects" required placeholder="Mathematics; Algebra"></label>
            <label>Tags <input v-model="questionForm.tags" placeholder="linear; practice"></label>
            <label>Question <textarea v-model="questionForm.text" required rows="5" placeholder="LaTeX such as $x+1=2$ is supported." /></label>
            <label v-if="isOptionType">Options <textarea v-model="questionForm.options" rows="3" placeholder="One option per line" /></label>
            <label>Answer <textarea v-model="questionForm.answer" required rows="3" /></label>
            <label>Source <input v-model="questionForm.source"></label>
            <section v-if="editingQuestion" class="job-card" aria-labelledby="question-images-title">
              <div class="editor-title">
                <div>
                  <span class="card-kicker">Native attachments</span>
                  <h3 id="question-images-title">Question images</h3>
                </div>
                <small>{{ editingQuestion.images.length }} attached</small>
              </div>
              <ul v-if="editingQuestion.images.length">
                <li v-for="image in editingQuestion.images" :key="image.attachmentId">
                  <strong>{{ image.fileName }}</strong>
                  <small>{{ image.mediaType }} · {{ formatBytes(image.byteSize) }}<template v-if="image.caption"> · {{ image.caption }}</template></small>
                </li>
              </ul>
              <p v-else class="empty-state">No images attached.</p>
              <template v-if="editingQuestion.deletedAt === null">
                <label>Optional caption <input v-model="questionImageCaption" maxlength="500" autocomplete="off"></label>
                <button class="button" type="button" :disabled="busy" @click="addSelectedQuestionImage">Choose image…</button>
              </template>
              <small v-else>Restore this question before adding images.</small>
            </section>
            <div class="editor-actions">
              <button class="button" type="button" @click="startNewQuestion">Clear</button>
              <button class="button button--primary" type="submit" :disabled="busy || !questionForm.text.trim()">{{ editingQuestion ? 'Save new revision' : 'Create question' }}</button>
            </div>
          </form>
        </div>
      </section>

      <section v-else-if="activeTab === 'papers'" class="workspace-panel" aria-labelledby="papers-title">
        <div class="panel-toolbar">
          <div><span class="card-kicker">CLE-27</span><h2 id="papers-title">Professional paper generation</h2></div>
        </div>
        <form class="paper-form" @submit.prevent="submitGeneration">
          <div class="form-grid form-grid--three">
            <label>Title <input v-model="paperForm.title" required></label>
            <label>Subjects <input v-model="paperForm.subjects" required placeholder="Mathematics"></label>
            <label>Duration <input v-model.number="paperForm.durationMinutes" type="number" min="1" required></label>
            <label>Total marks <input v-model="paperForm.totalMarks" inputmode="decimal" required></label>
            <label>Question count <input v-model.number="paperForm.count" type="number" min="1" max="200" required></label>
            <label>Difficulty coefficient <input v-model.number="paperForm.difficultyCoefficient" type="number" min="0" max="1" step="0.05"></label>
          </div>
          <button class="button button--primary" type="submit" :disabled="busy">Generate locally</button>
        </form>
        <div class="job-grid">
          <article v-for="job in paperJobs" :key="job.id" class="job-card">
            <span class="card-kicker">{{ job.kind }}</span>
            <h3>{{ job.phase || job.state }}</h3>
            <progress :value="job.completedUnits" :max="job.totalUnits ?? Math.max(job.completedUnits, 1)" />
            <small>{{ job.state }} · {{ job.completedUnits }}{{ job.totalUnits === null ? '' : `/${job.totalUnits}` }}</small>
            <div class="editor-actions">
              <button v-if="job.cancellable" class="button" type="button" @click="cancelJob(job.id)">Cancel</button>
              <template v-if="job.state === 'completed' && typeof job.result?.paperId === 'string'">
                <button v-for="format in exportFormats" :key="format" class="button" type="button" @click="exportPaper(String(job.result?.paperId), format)">{{ format.toUpperCase() }}</button>
              </template>
            </div>
          </article>
          <p v-if="!paperJobs.length" class="empty-state">Generated papers and exports will appear here.</p>
        </div>
      </section>

      <section v-else class="workspace-panel" aria-labelledby="backups-title">
        <div class="panel-toolbar">
          <div><span class="card-kicker">CLE-28</span><h2 id="backups-title">Backup & restore</h2></div>
          <button class="button button--primary" type="button" :disabled="busy" @click="manualBackup">Create portable backup</button>
        </div>
        <form class="backup-form" @submit.prevent="saveBackupSchedule">
          <label class="toggle-row"><input v-model="backupForm.enabled" type="checkbox"> Enable scheduled backups</label>
          <div class="destination-row">
            <div><strong>Destination</strong><span>{{ backupDestination?.displayName ?? backupSchedule?.destinationDisplayName ?? 'Not selected' }}</span></div>
            <button class="button" type="button" @click="selectBackupDestination">Choose folder</button>
          </div>
          <div class="form-grid form-grid--three">
            <label>Interval, minutes <input v-model.number="backupForm.intervalMinutes" type="number" min="60" max="43200"></label>
            <label>Retention, days <input v-model.number="backupForm.retentionDays" type="number" min="1" max="3650"></label>
            <label>Encryption
              <select v-model="backupForm.encryptionMode"><option value="keychain">OS keychain</option><option value="none">None</option></select>
            </label>
          </div>
          <div v-if="backupForm.encryptionMode === 'keychain'" class="destination-row">
            <div>
              <strong>Recovery key</strong>
              <span>{{ backupForm.recoveryKeyConfirmed ? 'Saved and confirmed for this configuration' : 'Generate and save this before enabling encrypted backups' }}</span>
            </div>
            <button class="button" type="button" :disabled="busy" @click="prepareBackupEncryption">Show recovery key</button>
          </div>
          <label v-if="backupForm.encryptionMode === 'keychain'" class="toggle-row"><input v-model="backupForm.recoveryKeyConfirmed" type="checkbox"> I saved the recovery key shown during key setup.</label>
          <button
            class="button button--primary"
            type="submit"
            :disabled="busy || (backupForm.enabled && !backupDestination && !backupSchedule?.destinationDisplayName) || (backupForm.enabled && backupForm.encryptionMode === 'keychain' && !backupForm.recoveryKeyConfirmed)"
          >
            Save schedule
          </button>
        </form>

        <section class="backup-form" aria-labelledby="restore-workspace-title">
          <div class="editor-title">
            <div><span class="card-kicker">Restore</span><h3 id="restore-workspace-title">Restore a workspace backup</h3></div>
          </div>
          <p>Selecting a backup performs a read-only compatibility check before any workspace data is replaced.</p>
          <div class="form-grid form-grid--three">
            <label>Unlock method
              <select v-model="restoreForm.unlockMethod">
                <option value="keychain">OS keychain / unencrypted</option>
                <option value="passphrase">Portable-backup passphrase</option>
                <option value="recoveryKey">Exported recovery key</option>
              </select>
            </label>
            <label v-if="restoreForm.unlockMethod === 'passphrase'">Passphrase
              <input v-model="restoreForm.passphrase" type="password" autocomplete="off" maxlength="4096">
            </label>
            <label v-if="restoreForm.unlockMethod === 'recoveryKey'">Recovery key
              <input v-model="restoreForm.recoveryKey" type="password" autocomplete="off" maxlength="4096">
            </label>
          </div>
          <div class="editor-actions">
            <button class="button" type="button" :disabled="busy || Boolean(restorePreflight)" @click="chooseWorkspaceRestore">Select backup…</button>
          </div>
        </section>

        <section class="backup-form" aria-labelledby="data-directory-title">
          <div class="editor-title">
            <div><span class="card-kicker">Storage</span><h3 id="data-directory-title">Workspace data directory</h3></div>
          </div>
          <p>The destination is checked for space and write access. The original workspace remains available after migration.</p>
          <div class="destination-row">
            <div>
              <strong>{{ dataDirectorySelection?.displayName ?? 'No destination selected' }}</strong>
              <span v-if="dataDirectorySelection">{{ dataDirectorySelection.writable ? 'Writable' : 'Read only' }} · {{ formatAvailableBytes(dataDirectorySelection.availableBytes) }}</span>
              <span v-else>Choose a parent folder for the new workspace directory.</span>
            </div>
            <button class="button" type="button" :disabled="busy" @click="selectDataDirectory">Choose folder</button>
          </div>
          <div class="editor-actions">
            <button class="button button--primary" type="button" :disabled="busy || !dataDirectorySelection?.writable" @click="migrateWorkspaceDataDirectory">Move workspace data</button>
          </div>
        </section>

        <div class="job-grid">
          <article v-for="job in backupJobs" :key="job.id" class="job-card">
            <span class="card-kicker">{{ job.kind }}</span><h3>{{ job.phase || job.state }}</h3>
            <small>{{ job.error?.message ?? job.state }}</small>
            <button v-if="job.cancellable" class="button" type="button" @click="cancelJob(job.id)">Cancel</button>
          </article>
        </div>
      </section>
    </template>

    <div v-if="importInspection" class="modal-backdrop" role="presentation">
      <section class="modal import-modal" role="dialog" aria-modal="true" aria-labelledby="import-title">
        <button class="modal-close" type="button" aria-label="Close import preview" @click="discardQuestionImport"><AppIcon name="x" /></button>
        <span class="modal-icon"><AppIcon name="upload" /></span>
        <h2 id="import-title">Review {{ importInspection.displayName }}</h2>
        <p><strong>{{ importInspection.validRows }}</strong> valid rows and <strong>{{ importInspection.invalidRows }}</strong> invalid rows.</p>
        <ul class="import-errors"><li v-for="item in importInspection.errors" :key="item.rowNumber">Row {{ item.rowNumber }}: {{ item.messages.join('; ') }}</li></ul>
        <p>All valid rows will be committed in one transaction. Invalid rows will remain uncommitted.</p>
        <div class="modal-actions">
          <button class="button" type="button" @click="discardQuestionImport">Cancel</button>
          <button class="button button--primary" type="button" :disabled="!importInspection.validRows" @click="commitQuestionImport">Import valid rows</button>
        </div>
      </section>
    </div>

    <div v-if="backupRecoveryKey" class="modal-backdrop" role="presentation">
      <section class="modal import-modal" role="dialog" aria-modal="true" aria-labelledby="recovery-key-title" aria-describedby="recovery-key-description">
        <span class="modal-icon"><AppIcon name="settings" /></span>
        <h2 id="recovery-key-title">Save this recovery key now</h2>
        <p id="recovery-key-description">This is the only frontend display. It is removed from memory when this dialog closes and is never written to browser storage.</p>
        <label>Key ID <input :value="backupRecoveryKey.keyId" readonly></label>
        <label>Recovery key
          <textarea :value="backupRecoveryKey.recoveryKey" readonly rows="5" spellcheck="false" autocomplete="off" @focus="($event.target as HTMLTextAreaElement).select()" />
        </label>
        <div class="modal-actions">
          <button class="button" type="button" @click="dismissBackupRecoveryKey">Close without confirming</button>
          <button class="button button--primary" type="button" @click="confirmBackupRecoveryKey">I saved this key</button>
        </div>
      </section>
    </div>

    <div v-if="restorePreflight" class="modal-backdrop" role="presentation">
      <section class="modal import-modal" role="dialog" aria-modal="true" aria-labelledby="restore-preflight-title">
        <span class="modal-icon"><AppIcon name="upload" /></span>
        <h2 id="restore-preflight-title">Review restore: {{ restorePreflight.displayName }}</h2>
        <p>Workspace {{ restorePreflight.workspaceId.slice(0, 8) }} · schema {{ restorePreflight.schemaVersionFound }} · app {{ restorePreflight.appVersion }}</p>
        <p>{{ restorePreflight.encrypted ? 'Encrypted backup' : 'Unencrypted backup' }}{{ restorePreflight.requiresRecoveryKey ? ' · recovery key required' : '' }}</p>
        <ul v-if="restorePreflight.warnings.length" class="import-errors">
          <li v-for="warning in restorePreflight.warnings" :key="warning">{{ warning }}</li>
        </ul>
        <p v-if="!restorePreflight.compatible" class="alert alert--error" role="alert">This backup is not compatible with the current app and cannot be restored.</p>
        <p v-else>Restoring enters maintenance mode and atomically replaces the active workspace after validation.</p>
        <div class="modal-actions">
          <button class="button" type="button" :disabled="busy" @click="discardWorkspaceRestore">Cancel</button>
          <button class="button button--primary" type="button" :disabled="busy || !restorePreflight.compatible" @click="commitWorkspaceRestore">Restore workspace</button>
        </div>
      </section>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue'

import { createLocalWorkspace } from '../application/useLocalWorkspace'
import type { PaperExportFormat, Question, QuestionInput, QuestionType } from '../types/localEngine'
import AppIcon from './AppIcon.vue'

const {
  engine, ready, activeTab, questions, nextQuestionCursor, selectedQuestion, importInspection,
  backupSchedule, backupDestination, backupRecoveryKey, restorePreflight, dataDirectorySelection,
  jobs, query, busy, error, retryEngine, refreshQuestions,
  loadMoreQuestions, saveQuestion, toggleQuestionDeleted, addQuestionImage, selectQuestionImport,
  commitQuestionImport, discardQuestionImport, generatePaper, exportPaper,
  configureBackupSchedule, prepareBackupEncryption, dismissBackupRecoveryKey,
  selectBackupDestination, createBackup, selectWorkspaceRestore, commitWorkspaceRestore,
  discardWorkspaceRestore, selectDataDirectory, migrateWorkspaceDataDirectory, cancelJob
} = createLocalWorkspace()

const questionForm = reactive({
  type: 'single_choice' as QuestionType,
  difficulty: 'medium' as const,
  subjects: '', tags: '', text: '', options: '', answer: '', source: ''
})
const editingQuestion = computed(() => selectedQuestion.value)
const isOptionType = computed(() => ['single_choice', 'multiple_choice', 'true_false'].includes(questionForm.type))
const questionImageCaption = ref('')
const engineLabel = computed(() => ({
  starting: 'Starting Local Engine', ready: 'Local Engine ready', recovering: 'Recovering Local Engine',
  degraded: 'Local Engine needs attention', stopping: 'Stopping Local Engine'
})[engine.value?.state ?? 'starting'])

const paperForm = reactive({ title: '', subjects: '', durationMinutes: 60, totalMarks: '100', count: 10, difficultyCoefficient: 0.5 })
const exportFormats: PaperExportFormat[] = ['docx', 'tex', 'pdf']
const paperJobs = computed(() => [...jobs.value.values()].filter(job => ['generation', 'export'].includes(job.kind)))
const backupJobs = computed(() => [...jobs.value.values()].filter(job => ['backup', 'restore', 'dataDirectoryMigration'].includes(job.kind)))
const backupForm = reactive({ enabled: false, intervalMinutes: 1440, retentionDays: 30, encryptionMode: 'keychain' as 'keychain' | 'none', recoveryKeyConfirmed: false })
const restoreForm = reactive({
  unlockMethod: 'keychain' as 'keychain' | 'passphrase' | 'recoveryKey',
  passphrase: '',
  recoveryKey: ''
})

watch(backupSchedule, value => {
  if (!value) return
  Object.assign(backupForm, {
    enabled: value.enabled,
    intervalMinutes: value.intervalMinutes,
    retentionDays: value.retentionDays,
    encryptionMode: value.encryptionMode
  })
}, { immediate: true })

function splitList(value: string) { return [...new Set(value.split(/[;,|\n]/).map(item => item.trim()).filter(Boolean))] }
function typeLabel(value: QuestionType) { return value.replaceAll('_', ' ') }
function formatBytes(value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / (1024 * 1024)).toFixed(1)} MB`
}
function formatAvailableBytes(value: number | null) { return value === null ? 'Available space unknown' : `${formatBytes(value)} available` }

function selectQuestion(question: Question) {
  selectedQuestion.value = question
  questionImageCaption.value = ''
  Object.assign(questionForm, {
    type: question.type,
    difficulty: question.difficulty,
    subjects: question.subjects.join('; '), tags: question.tags.join('; '), text: question.text,
    options: question.options.join('\n'), answer: Array.isArray(question.answer) ? question.answer.join('\n') : question.answer,
    source: question.source ?? ''
  })
}

function startNewQuestion() {
  selectedQuestion.value = null
  questionImageCaption.value = ''
  Object.assign(questionForm, { type: 'single_choice', difficulty: 'medium', subjects: '', tags: '', text: '', options: '', answer: '', source: '' })
}

async function submitQuestion() {
  const input: QuestionInput = {
    type: questionForm.type,
    difficulty: questionForm.difficulty,
    subjects: splitList(questionForm.subjects), tags: splitList(questionForm.tags).map(tag => tag.toLowerCase()),
    text: questionForm.text.trim(), options: isOptionType.value ? splitList(questionForm.options) : undefined,
    answer: questionForm.type === 'multiple_choice' ? splitList(questionForm.answer) : questionForm.answer.trim(),
    source: questionForm.source.trim() || null,
    hasLatex: /\$\$[^$]+\$\$|\$[^$]+\$/.test(`${questionForm.text} ${questionForm.answer}`), scoreWeight: '1'
  }
  await saveQuestion(input, editingQuestion.value)
}

async function addSelectedQuestionImage() {
  const question = editingQuestion.value
  if (!question || question.deletedAt !== null) return
  const attached = await addQuestionImage(question.id, questionImageCaption.value.trim() || undefined)
  if (attached) questionImageCaption.value = ''
}

async function submitGeneration() {
  await generatePaper({
    title: paperForm.title.trim(), subjects: splitList(paperForm.subjects), durationMinutes: paperForm.durationMinutes,
    totalMarks: paperForm.totalMarks, difficultyCoefficient: paperForm.difficultyCoefficient,
    questionTypes: [{ questionType: 'single_choice', count: paperForm.count }], requiredTags: [], preferredTags: [], seed: Date.now()
  })
}

async function saveBackupSchedule() {
  await configureBackupSchedule({
    enabled: backupForm.enabled,
    ...(backupDestination.value ? { destinationSelectionId: backupDestination.value.selectionId } : {}),
    intervalMinutes: backupForm.intervalMinutes, retentionDays: backupForm.retentionDays,
    encryptionMode: backupForm.encryptionMode, recoveryKeyConfirmed: backupForm.recoveryKeyConfirmed
  })
}

function confirmBackupRecoveryKey() {
  backupForm.recoveryKeyConfirmed = true
  dismissBackupRecoveryKey()
}

async function chooseWorkspaceRestore() {
  const unlock = restoreForm.unlockMethod === 'passphrase'
    ? { passphrase: restoreForm.passphrase }
    : restoreForm.unlockMethod === 'recoveryKey'
      ? { recoveryKey: restoreForm.recoveryKey }
      : {}
  try {
    await selectWorkspaceRestore(unlock)
  } finally {
    restoreForm.passphrase = ''
    restoreForm.recoveryKey = ''
  }
}

async function manualBackup() {
  const passphrase = globalThis.prompt('Enter a portable backup passphrase. Leave blank for an unencrypted backup.') ?? ''
  await createBackup(Boolean(passphrase), passphrase || undefined)
}
</script>
