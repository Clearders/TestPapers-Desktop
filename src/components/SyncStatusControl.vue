<template>
  <button
    class="status-chip sync-chip"
    :class="`sync-chip--${state?.status ?? 'authRequired'}`"
    type="button"
    aria-haspopup="dialog"
    @click="open = true"
  >
    <span aria-hidden="true" />
    {{ presentation.title }}
    <small v-if="state?.paused">Paused</small>
  </button>

  <Teleport to="body">
    <div v-if="open" class="modal-backdrop" role="presentation" @click.self="open = false">
      <section class="modal sync-modal" role="dialog" aria-modal="true" aria-labelledby="sync-status-title">
        <button class="modal-close" type="button" aria-label="Close sync status" @click="open = false"><AppIcon name="x" /></button>
        <span class="modal-icon"><AppIcon :name="state?.status === 'synced' ? 'check' : 'upload'" /></span>
        <span class="card-kicker">Cloud Sync v1</span>
        <h2 id="sync-status-title">{{ presentation.title }}</h2>
        <p>{{ presentation.description }}</p>
        <p class="sync-next-step"><strong>Next step:</strong> {{ presentation.action }}</p>
        <p v-if="error" class="alert alert--error" role="alert">{{ error }}</p>

        <dl class="sync-counts">
          <div><dt>Pending</dt><dd>{{ state?.pendingCount ?? 0 }}</dd></div>
          <div><dt>Retrying</dt><dd>{{ state?.retryingCount ?? 0 }}</dd></div>
          <div><dt>Conflicts</dt><dd>{{ state?.conflictCount ?? 0 }}</dd></div>
          <div><dt>Failed</dt><dd>{{ state?.failedCount ?? 0 }}</dd></div>
        </dl>

        <div v-if="state?.lastErrorCode" class="sync-error-code">
          <span>Stable error code</span><code>{{ state.lastErrorCode }}</code>
        </div>
        <div v-if="state?.entities.length" class="sync-entities">
          <strong>Affected items</strong>
          <ul>
            <li v-for="entity in state.entities" :key="`${entity.entityType}:${entity.entityId}`">
              <span>{{ entity.entityType }}</span>
              <code>{{ entity.entityId }}</code>
              <em :class="`entity-sync entity-sync--${entity.status}`">{{ entity.status }}</em>
            </li>
          </ul>
        </div>

        <div class="modal-actions sync-actions">
          <button v-if="state?.status === 'authRequired'" class="button button--primary" type="button" @click="$emit('openAccount')">Open account settings</button>
          <button v-if="state?.canResume" class="button button--primary" type="button" :disabled="busy" @click="$emit('resume')">Resume sync</button>
          <button v-if="state?.canRetry" class="button button--primary" type="button" :disabled="busy" @click="$emit('retry')">Retry safely</button>
          <button v-if="state?.canSyncNow" class="button" type="button" :disabled="busy" @click="$emit('syncNow')">Sync now</button>
          <button v-if="state?.canPause" class="button" type="button" :disabled="busy" @click="$emit('pause')">Pause sync</button>
          <button class="button" type="button" @click="open = false">Close</button>
        </div>
        <small class="sync-local-note">Pausing or losing Cloud access never disables local editing.</small>
      </section>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref } from 'vue'

import type { SyncStatusSnapshot } from '../types/sync'
import AppIcon from './AppIcon.vue'

defineProps<{
  state: SyncStatusSnapshot | null
  presentation: { title: string; description: string; action: string }
  busy: boolean
  error: string
}>()
defineEmits<{
  pause: []
  resume: []
  syncNow: []
  retry: []
  openAccount: []
}>()

const open = ref(false)
</script>
