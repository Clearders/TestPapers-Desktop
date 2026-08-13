import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { parseSyncStatusChangedEvent, parseSyncStatusSnapshot, type SyncStatusSnapshot } from '../../types/sync'

export const SYNC_EVENTS = { statusChanged: 'testpapers://sync/status-changed' } as const

export interface SyncBridge {
  configureSession(input: SyncSessionInput): Promise<SyncStatusSnapshot>
  getStatus(): Promise<SyncStatusSnapshot>
  pause(): Promise<SyncStatusSnapshot>
  resume(): Promise<SyncStatusSnapshot>
  syncNow(): Promise<SyncStatusSnapshot>
  retry(): Promise<SyncStatusSnapshot>
  onStatusChanged(handler: (status: SyncStatusSnapshot) => void): Promise<UnlistenFn>
}

export interface SyncSessionInput {
  baseUrl: string
  accountId: string
  deviceId: string
  accessToken: string
}

async function invokeStatus(command: string) {
  return parseSyncStatusSnapshot(await tauriInvoke(command))
}

export const tauriSyncBridge: SyncBridge = {
  configureSession: async input => parseSyncStatusSnapshot(await tauriInvoke('configure_sync_session', { input })),
  getStatus: () => invokeStatus('get_sync_status'),
  pause: () => invokeStatus('pause_sync'),
  resume: () => invokeStatus('resume_sync'),
  syncNow: () => invokeStatus('sync_now'),
  retry: () => invokeStatus('retry_sync'),
  onStatusChanged(handler) {
    return listen(SYNC_EVENTS.statusChanged, event => handler(parseSyncStatusChangedEvent(event.payload).state))
  }
}
