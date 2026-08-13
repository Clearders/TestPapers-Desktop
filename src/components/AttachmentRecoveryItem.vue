<template>
  <li class="attachment-item" :class="`attachment-item--${status ?? 'local'}`">
    <div class="attachment-copy">
      <strong>{{ image.fileName }}</strong>
      <small>{{ image.mediaType }} · {{ formatBytes(image.byteSize) }}<template v-if="image.caption"> · {{ image.caption }}</template></small>
    </div>
    <div v-if="recovery" class="attachment-placeholder" role="status" aria-live="polite">
      <span aria-hidden="true">{{ recovery.icon }}</span>
      <div>
        <strong>{{ recovery.title }}</strong>
        <small>{{ recovery.reason }} The question content remains available.</small>
      </div>
      <button v-if="recovery.retryable" class="button" type="button" @click="$emit('retry')">Retry attachment</button>
    </div>
    <em v-else class="entity-sync entity-sync--synced">Available locally</em>
  </li>
</template>

<script setup lang="ts">
import { computed } from 'vue'

import type { QuestionImage } from '../types/localEngine'
import type { SyncStatus } from '../types/sync'

const props = defineProps<{
  image: QuestionImage
  status?: SyncStatus
  errorCode?: string | null
}>()

defineEmits<{ retry: [] }>()

const recovery = computed(() => {
  if (!props.status || props.status === 'synced') return null
  const error = props.errorCode ? ` (${props.errorCode})` : ''
  if (props.status === 'failed' || props.status === 'conflict') {
    return { icon: '!', title: 'Attachment needs attention', reason: `Cloud could not safely accept these bytes${error}.`, retryable: true }
  }
  if (props.status === 'offline' || props.status === 'authRequired') {
    return { icon: '↓', title: 'Attachment unavailable on this device', reason: 'The file will be fetched after Cloud access returns.', retryable: true }
  }
  if (props.status === 'retrying') {
    return { icon: '↻', title: 'Attachment retry scheduled', reason: `The verified transfer will resume without restarting${error}.`, retryable: true }
  }
  return { icon: '…', title: 'Attachment transfer pending', reason: 'Metadata is safe while verified bytes finish syncing.', retryable: false }
})

function formatBytes (value: number) {
  if (value < 1024) return `${value} B`
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`
  return `${(value / (1024 * 1024)).toFixed(1)} MB`
}
</script>

<style scoped>
.attachment-item { display: grid; gap: 8px; padding: 10px 0; border-bottom: 1px solid var(--color-border); }
.attachment-item:last-child { border-bottom: 0; }
.attachment-copy { display: grid; gap: 2px; }
.attachment-copy small, .attachment-placeholder small { color: var(--color-muted); }
.attachment-placeholder { display: grid; grid-template-columns: auto 1fr auto; gap: 10px; align-items: center; padding: 10px; border: 1px dashed var(--color-warning); border-radius: var(--radius); background: color-mix(in srgb, var(--color-warning) 7%, transparent); }
.attachment-placeholder > span { display: grid; place-items: center; width: 28px; height: 28px; border-radius: 50%; background: color-mix(in srgb, var(--color-warning) 18%, transparent); font-weight: 900; }
.attachment-placeholder > div { display: grid; gap: 2px; }
@media (max-width: 560px) { .attachment-placeholder { grid-template-columns: auto 1fr; } .attachment-placeholder .button { grid-column: 1 / -1; } }
</style>
