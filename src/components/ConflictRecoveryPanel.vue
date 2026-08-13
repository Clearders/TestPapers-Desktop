<template>
  <section class="conflict-recovery" aria-labelledby="desktop-conflicts-title">
    <header class="conflict-recovery__header">
      <div><span class="card-kicker">Personal device Sync</span><h2 id="desktop-conflicts-title">Conflict recovery</h2></div>
      <div class="conflict-recovery__actions"><span>{{ unresolvedCount }} needs review</span><button class="button" type="button" :disabled="busy" @click="refresh">Refresh</button></div>
    </header>
    <p class="conflict-origin-note"><strong>Separate from realtime collaboration.</strong> These snapshots came from personal-device Sync; collaborative draft revisions stay in Web.</p>
    <p v-if="error" class="alert alert--error" role="alert">{{ error }}</p>
    <p v-if="notice" class="alert" role="status" aria-live="polite">{{ notice }}</p>
    <p v-if="!conflicts.length && !busy" class="conflict-empty">No preserved conflicts need review.</p>

    <div v-else class="conflict-layout">
      <nav class="conflict-list" aria-label="Preserved Sync conflicts">
        <button v-for="item in conflicts" :key="item.conflictId" type="button" :class="{ active: selected?.conflictId === item.conflictId }" @click="select(item)">
          <strong>{{ label(item.entityType) }}</strong><span>{{ item.entityId.slice(0, 12) }} · {{ label(item.state) }}</span>
        </button>
      </nav>

      <article v-if="selected" class="conflict-detail" :aria-labelledby="`conflict-${selected.conflictId}`">
        <div class="conflict-meta"><div><span class="card-kicker">{{ label(selected.reason) }}</span><h3 :id="`conflict-${selected.conflictId}`">{{ selected.entityType }} · {{ selected.entityId }}</h3></div><span>{{ formatTime(selected.updatedAt) }}</span></div>
        <div class="snapshot-devices"><span>Local · {{ selected.local.deviceId }}</span><span>Cloud · {{ selected.cloud.deviceId }}</span></div>

        <div class="desktop-comparison" role="table" aria-label="Local baseline Cloud comparison">
          <div class="desktop-comparison__row desktop-comparison__head" role="row"><span>Field</span><span>Baseline</span><span>Local</span><span>Cloud</span></div>
          <div v-for="difference in differences" :key="difference.field" class="desktop-comparison__row" :class="`desktop-comparison__row--${difference.change}`" role="row">
            <strong>{{ difference.field }}<small>{{ label(difference.change) }}</small></strong><code>{{ display(difference.base) }}</code><code>{{ display(difference.local) }}</code><code>{{ display(difference.cloud) }}</code>
          </div>
        </div>

        <div class="conflict-choice-grid">
          <button class="button" type="button" :disabled="busy || selected.state !== 'unresolved'" @click="resolve('keepLocal')">Keep Local</button>
          <button class="button" type="button" :disabled="busy || selected.state !== 'unresolved'" @click="resolve('useCloud')">Use Cloud</button>
          <button class="button" type="button" :disabled="busy || selected.state !== 'unresolved'" @click="resolve('saveCopy')">Save a copy</button>
          <button v-if="selected.base" class="button" type="button" :disabled="busy || selected.state !== 'unresolved'" @click="resolve('restoreVersion', { version: selected.base.version })">Restore baseline v{{ selected.base.version }}</button>
        </div>

        <div v-if="richMerge" class="desktop-manual-merge">
          <label :for="`merge-${selected.conflictId}`"><strong>Manual merge JSON</strong><span>Drafts survive app restarts; submitted decisions persist in SQLite before transport.</span></label>
          <textarea :id="`merge-${selected.conflictId}`" v-model="manualPayload" rows="8" spellcheck="false" />
          <button class="button button--primary" type="button" :disabled="busy || selected.state !== 'unresolved'" @click="submitManual">Accept manual merge</button>
        </div>

        <section class="conflict-audit" aria-labelledby="conflict-audit-title">
          <h4 id="conflict-audit-title">Resolution history</h4>
          <p v-if="!selected.resolutions.length">No accepted resolution yet.</p>
          <ol v-else><li v-for="resolution in selected.resolutions" :key="resolution.resolutionId"><strong>{{ label(resolution.action) }} → v{{ resolution.acceptedVersion }}</strong><span>{{ resolution.actorDeviceId }} · {{ formatIso(resolution.resolvedAt) }}</span></li></ol>
          <button v-if="undoCandidate" class="button" type="button" :disabled="busy || selected.state !== 'resolved'" @click="resolve('undo', { undoesResolutionId: undoCandidate.resolutionId })">Undo latest resolution</button>
        </section>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'

import { tauriSyncBridge, type SyncBridge } from '../infrastructure/tauri/syncBridge'
import { compareSyncPayloads, type SyncConflictRecoveryRecord, type SyncResolutionAction } from '../types/syncConflict'

const props = withDefaults(defineProps<{ bridge?: SyncBridge; conflictCount?: number }>(), { bridge: () => tauriSyncBridge, conflictCount: 0 })
const conflicts = ref<SyncConflictRecoveryRecord[]>([])
const selected = ref<SyncConflictRecoveryRecord | null>(null)
const manualPayload = ref('{}')
const busy = ref(false)
const error = ref('')
const notice = ref('')
const draftPrefix = 'testpapers.sync-conflict-draft.'
let restoringDraft = false
const unresolvedCount = computed(() => conflicts.value.filter(item => ['unresolved', 'resolving'].includes(item.state)).length)
const richMerge = computed(() => Boolean(selected.value && ['question', 'paper', 'draft'].includes(selected.value.entityType)))
const differences = computed(() => selected.value ? compareSyncPayloads(selected.value.base?.payload ?? null, selected.value.local.payload, selected.value.cloud.payload) : [])
const undoCandidate = computed(() => selected.value?.resolutions.at(-1)?.action === 'undo' ? null : selected.value?.resolutions.at(-1) ?? null)

onMounted(refresh)
watch(() => props.conflictCount, refresh)
watch(manualPayload, value => {
  if (!restoringDraft && selected.value?.state === 'unresolved') globalThis.localStorage.setItem(`${draftPrefix}${selected.value.conflictId}`, value)
}, { flush: 'sync' })

async function refresh () {
  busy.value = true; error.value = ''
  try {
    conflicts.value = await props.bridge.listConflicts()
    const currentId = selected.value?.conflictId
    select(conflicts.value.find(item => item.conflictId === currentId) ?? conflicts.value[0] ?? null)
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) } finally { busy.value = false }
}

function select (item: SyncConflictRecoveryRecord | null) {
  selected.value = item
  restoringDraft = true
  manualPayload.value = item
    ? globalThis.localStorage.getItem(`${draftPrefix}${item.conflictId}`) ?? JSON.stringify(item.local.payload ?? {}, null, 2)
    : '{}'
  restoringDraft = false
}

async function resolve (action: SyncResolutionAction, options: { version?: number; undoesResolutionId?: string; payload?: Record<string, unknown> } = {}) {
  if (!selected.value || !globalThis.confirm(`Queue “${label(action)}” for this conflict?`)) return
  const conflictId = selected.value.conflictId
  const request: Record<string, unknown> = {
    protocolVersion: 1, operationId: globalThis.crypto.randomUUID(), action,
    currentVersion: selected.value.cloud.version, currentContentHash: selected.value.cloud.contentHash
  }
  if (action === 'saveCopy') request.newEntityId = globalThis.crypto.randomUUID()
  if (action === 'manualMerge') request.payload = options.payload
  if (action === 'restoreVersion') request.payload = { version: options.version }
  if (action === 'undo') request.undoesResolutionId = options.undoesResolutionId
  busy.value = true; error.value = ''; notice.value = ''
  try {
    await props.bridge.resolveConflict(conflictId, request)
    globalThis.localStorage.removeItem(`${draftPrefix}${conflictId}`)
    notice.value = 'Decision saved to the persistent queue. Sync will retry safely until Cloud accepts it.'
    await refresh()
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) } finally { busy.value = false }
}

async function submitManual () {
  try {
    const value = JSON.parse(manualPayload.value) as unknown
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Manual merge must be a JSON object.')
    await resolve('manualMerge', { payload: value as Record<string, unknown> })
  } catch (cause) { error.value = cause instanceof Error ? cause.message : String(cause) }
}

function display (value: unknown) { return value === undefined ? '—' : typeof value === 'string' ? value : JSON.stringify(value, null, 2) }
function label (value: string) { return value.replace(/([a-z])([A-Z])/g, '$1 $2').replace(/^./, letter => letter.toUpperCase()) }
function formatTime (value: number) { return new Date(value / 1000).toLocaleString() }
function formatIso (value: string) { return new Date(value).toLocaleString() }
</script>
