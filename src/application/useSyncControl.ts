import { computed, onBeforeUnmount, onMounted, ref } from 'vue'

import { tauriSyncBridge, type SyncBridge } from '../infrastructure/tauri/syncBridge'
import { SYNC_STATUS_PRESENTATION, type SyncStatusSnapshot } from '../types/sync'

export function createSyncControl(bridge: SyncBridge = tauriSyncBridge) {
  const state = ref<SyncStatusSnapshot | null>(null)
  const busy = ref(false)
  const error = ref('')
  let disposed = false
  let unlisten: (() => void) | undefined

  const presentation = computed(() => SYNC_STATUS_PRESENTATION[state.value?.status ?? 'authRequired'])

  async function run(operation: () => Promise<SyncStatusSnapshot>) {
    busy.value = true
    error.value = ''
    try {
      state.value = await operation()
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
    } finally {
      busy.value = false
    }
  }

  async function initialise() {
    try {
      unlisten = await bridge.onStatusChanged(status => { state.value = status })
      if (disposed) unlisten()
      else state.value = await bridge.getStatus()
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause)
    }
  }

  function teardown() {
    disposed = true
    unlisten?.()
    unlisten = undefined
  }

  onMounted(() => { void initialise() })
  onBeforeUnmount(teardown)

  return {
    state,
    busy,
    error,
    presentation,
    pause: () => run(() => bridge.pause()),
    resume: () => run(() => bridge.resume()),
    syncNow: () => run(() => bridge.syncNow()),
    retry: () => run(() => bridge.retry()),
    teardown
  }
}
