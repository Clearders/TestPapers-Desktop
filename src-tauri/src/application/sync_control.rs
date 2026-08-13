use std::{
    sync::{Arc, Mutex, MutexGuard},
    thread,
};

use serde::Serialize;
use serde_json::Value;

use crate::{
    local_data::{LocalDataStore, SyncConflictRecoveryRecord, SyncControlData, SyncRuntimePhase},
    sync::{CloudSyncTransport, SyncRunReport, SyncWorker, SyncWorkerError, TransportErrorKind},
};

const SYNC_CLIENT_STATE_SCHEMA_VERSION: u8 = 1;
const SYNC_PROTOCOL_VERSION: u8 = 1;

type SyncRunner = Arc<dyn Fn() -> Result<SyncRunReport, SyncWorkerError> + Send + Sync>;
type StatusListener = Arc<dyn Fn(SyncStatusSnapshot) + Send + Sync>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SyncStatus {
    Synced,
    Pending,
    Syncing,
    Offline,
    Retrying,
    Conflict,
    AuthRequired,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SyncRecommendedAction {
    None,
    Wait,
    Resume,
    SyncNow,
    Retry,
    SignIn,
    ResolveConflict,
    ReviewFailure,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncEntityStatus {
    pub(crate) entity_type: String,
    pub(crate) entity_id: String,
    pub(crate) status: SyncStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncStatusSnapshot {
    pub(crate) schema_version: u8,
    pub(crate) protocol_version: u8,
    pub(crate) account_id: Option<String>,
    pub(crate) device_id: Option<String>,
    pub(crate) status: SyncStatus,
    pub(crate) paused: bool,
    pub(crate) phase: SyncRuntimePhase,
    pub(crate) pending_count: u32,
    pub(crate) retrying_count: u32,
    pub(crate) conflict_count: u32,
    pub(crate) failed_count: u32,
    pub(crate) last_completed_at: Option<i64>,
    pub(crate) last_error_code: Option<String>,
    pub(crate) recommended_action: SyncRecommendedAction,
    pub(crate) can_pause: bool,
    pub(crate) can_resume: bool,
    pub(crate) can_sync_now: bool,
    pub(crate) can_retry: bool,
    pub(crate) entities: Vec<SyncEntityStatus>,
}

#[derive(Clone)]
struct SyncSession {
    account_id: String,
    device_id: String,
    store: Arc<LocalDataStore>,
    runner: SyncRunner,
}

struct ControlState {
    session: Option<SyncSession>,
    running: bool,
    transient_status: Option<SyncStatus>,
    transient_error_code: Option<String>,
}

#[derive(Clone)]
pub(crate) struct SyncControlApplication {
    state: Arc<Mutex<ControlState>>,
    listener: StatusListener,
}

impl SyncControlApplication {
    pub(crate) fn new(listener: StatusListener) -> Self {
        Self {
            state: Arc::new(Mutex::new(ControlState {
                session: None,
                running: false,
                transient_status: None,
                transient_error_code: None,
            })),
            listener,
        }
    }

    /// Called by the native authentication boundary after it obtains a short-lived token. Tokens
    /// stay inside the generated client and are never included in state snapshots or diagnostics.
    #[allow(dead_code)]
    pub(crate) fn configure_cloud_session(
        &self,
        store: Arc<LocalDataStore>,
        base_path: impl Into<String>,
        access_token: impl Into<String>,
        account_id: String,
        device_id: String,
    ) -> Result<SyncStatusSnapshot, String> {
        let transport = Arc::new(
            CloudSyncTransport::new(base_path, access_token)
                .map_err(|_| "The sync network runtime could not start".to_owned())?,
        );
        let worker = Arc::new(
            SyncWorker::new(
                Arc::clone(&store),
                transport,
                account_id.clone(),
                device_id.clone(),
            )
            .map_err(|error| error.to_string())?,
        );
        self.install_session(
            store,
            account_id,
            device_id,
            Arc::new(move || worker.run_once()),
        )
    }

    fn install_session(
        &self,
        store: Arc<LocalDataStore>,
        account_id: String,
        device_id: String,
        runner: SyncRunner,
    ) -> Result<SyncStatusSnapshot, String> {
        let mut state = lock(&self.state);
        if state.running {
            return Err("A sync cycle must finish before the Cloud session changes".into());
        }
        store
            .register_sync_device(&account_id, &device_id)
            .map_err(|error| error.to_string())?;
        state.session = Some(SyncSession {
            account_id,
            device_id,
            store,
            runner,
        });
        state.transient_status = None;
        state.transient_error_code = None;
        drop(state);
        self.snapshot()
    }

    pub(crate) fn snapshot(&self) -> Result<SyncStatusSnapshot, String> {
        let state = lock(&self.state);
        snapshot_from_state(&state)
    }

    pub(crate) fn conflicts(&self) -> Result<Vec<SyncConflictRecoveryRecord>, String> {
        let session = lock(&self.state).session.clone();
        let Some(session) = session else {
            return Ok(Vec::new());
        };
        session
            .store
            .list_sync_conflict_recovery(&session.account_id)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn stage_resolution(
        &self,
        conflict_id: &str,
        request: &Value,
    ) -> Result<SyncStatusSnapshot, String> {
        let session = lock(&self.state)
            .session
            .clone()
            .ok_or_else(|| "Sign in before resolving a Sync conflict".to_owned())?;
        session
            .store
            .stage_conflict_resolution(&session.account_id, conflict_id, request)
            .map_err(|error| error.to_string())?;
        self.start_cycle(false)
    }

    pub(crate) fn pause(&self) -> Result<SyncStatusSnapshot, String> {
        let session = lock(&self.state).session.clone();
        if let Some(session) = session {
            session
                .store
                .set_sync_paused(&session.account_id, &session.device_id, true)
                .map_err(|error| error.to_string())?;
        }
        self.emit_snapshot()
    }

    pub(crate) fn resume(&self) -> Result<SyncStatusSnapshot, String> {
        let session = lock(&self.state).session.clone();
        if let Some(session) = session {
            session
                .store
                .set_sync_paused(&session.account_id, &session.device_id, false)
                .map_err(|error| error.to_string())?;
        }
        self.emit_snapshot()
    }

    pub(crate) fn sync_now(&self) -> Result<SyncStatusSnapshot, String> {
        self.start_cycle(false)
    }

    pub(crate) fn retry_now(&self) -> Result<SyncStatusSnapshot, String> {
        self.start_cycle(true)
    }

    fn start_cycle(&self, retry_now: bool) -> Result<SyncStatusSnapshot, String> {
        let session = {
            let mut state = lock(&self.state);
            let Some(session) = state.session.clone() else {
                return snapshot_from_state(&state);
            };
            let paused = session
                .store
                .sync_device_state(&session.account_id, &session.device_id)
                .map_err(|error| error.to_string())?
                .is_some_and(|device| device.paused);
            if state.running || paused {
                return snapshot_from_state(&state);
            }
            if retry_now {
                session
                    .store
                    .retry_sync_now(&session.account_id, &session.device_id)
                    .map_err(|error| error.to_string())?;
            }
            state.running = true;
            state.transient_status = None;
            state.transient_error_code = None;
            session
        };
        let initial = self.emit_snapshot()?;
        let application = self.clone();
        thread::Builder::new()
            .name("testpapers-sync-control".into())
            .spawn(move || {
                let result = (session.runner)();
                application.finish_cycle(result);
            })
            .map_err(|_| {
                lock(&self.state).running = false;
                "The background sync worker could not start".to_owned()
            })?;
        Ok(initial)
    }

    fn finish_cycle(&self, result: Result<SyncRunReport, SyncWorkerError>) {
        {
            let mut state = lock(&self.state);
            state.running = false;
            match result {
                Ok(report) => {
                    state.transient_status = match report.deferred_reason {
                        Some(TransportErrorKind::Offline) => Some(SyncStatus::Offline),
                        Some(TransportErrorKind::RateLimited) => Some(SyncStatus::Retrying),
                        _ => None,
                    };
                    state.transient_error_code = None;
                }
                Err(SyncWorkerError::Transport(error)) => {
                    state.transient_status = Some(match error.kind {
                        TransportErrorKind::AuthenticationRequired
                        | TransportErrorKind::DeviceRevoked => SyncStatus::AuthRequired,
                        TransportErrorKind::Offline => SyncStatus::Offline,
                        TransportErrorKind::RateLimited => SyncStatus::Retrying,
                        _ => SyncStatus::Failed,
                    });
                    state.transient_error_code = Some(stable_error_code(&error.code));
                }
                Err(SyncWorkerError::Local(_)) => {
                    state.transient_status = Some(SyncStatus::Failed);
                    state.transient_error_code = Some("SYNC_LOCAL_FAILURE".into());
                }
            }
        }
        let _ = self.emit_snapshot();
    }

    fn emit_snapshot(&self) -> Result<SyncStatusSnapshot, String> {
        let snapshot = self.snapshot()?;
        (self.listener)(snapshot.clone());
        Ok(snapshot)
    }
}

fn snapshot_from_state(state: &ControlState) -> Result<SyncStatusSnapshot, String> {
    let Some(session) = &state.session else {
        return Ok(empty_snapshot());
    };
    let data = session
        .store
        .sync_control_data(&session.account_id, &session.device_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "The sync device is not registered".to_owned())?;
    Ok(build_snapshot(
        &session.account_id,
        &session.device_id,
        data,
        state.running,
        state.transient_status,
        state.transient_error_code.clone(),
    ))
}

fn build_snapshot(
    account_id: &str,
    device_id: &str,
    data: SyncControlData,
    running: bool,
    transient_status: Option<SyncStatus>,
    transient_error_code: Option<String>,
) -> SyncStatusSnapshot {
    let status = if data.device.authentication_state != "ready" {
        SyncStatus::AuthRequired
    } else if data.conflict_count > 0 {
        SyncStatus::Conflict
    } else if data.failed_count > 0 {
        SyncStatus::Failed
    } else if running || data.device.runtime_phase != SyncRuntimePhase::Idle {
        SyncStatus::Syncing
    } else if let Some(transient) = transient_status {
        transient
    } else if data.retrying_count > 0 {
        SyncStatus::Retrying
    } else if data.pending_count > 0 {
        SyncStatus::Pending
    } else {
        SyncStatus::Synced
    };
    let paused = data.device.paused;
    let recommended_action = if paused {
        SyncRecommendedAction::Resume
    } else {
        match status {
            SyncStatus::Synced => SyncRecommendedAction::None,
            SyncStatus::Pending => SyncRecommendedAction::SyncNow,
            SyncStatus::Syncing => SyncRecommendedAction::Wait,
            SyncStatus::Offline | SyncStatus::Retrying => SyncRecommendedAction::Retry,
            SyncStatus::Conflict => SyncRecommendedAction::ResolveConflict,
            SyncStatus::AuthRequired => SyncRecommendedAction::SignIn,
            SyncStatus::Failed => SyncRecommendedAction::ReviewFailure,
        }
    };
    let entities = data
        .entities
        .into_iter()
        .map(|entity| SyncEntityStatus {
            entity_type: entity.entity_type,
            entity_id: entity.entity_id,
            status: status_from_queue(&entity.status),
        })
        .collect();
    SyncStatusSnapshot {
        schema_version: SYNC_CLIENT_STATE_SCHEMA_VERSION,
        protocol_version: SYNC_PROTOCOL_VERSION,
        account_id: Some(account_id.into()),
        device_id: Some(device_id.into()),
        status,
        paused,
        phase: data.device.runtime_phase,
        pending_count: data.pending_count,
        retrying_count: data.retrying_count,
        conflict_count: data.conflict_count,
        failed_count: data.failed_count,
        last_completed_at: data.device.last_completed_at,
        last_error_code: transient_error_code
            .or(data.last_error_code)
            .or(data.device.last_error_code)
            .map(|code| stable_error_code(&code)),
        recommended_action,
        can_pause: !paused,
        can_resume: paused,
        can_sync_now: !paused && !running,
        can_retry: !paused
            && matches!(
                status,
                SyncStatus::Offline | SyncStatus::Retrying | SyncStatus::Failed
            ),
        entities,
    }
}

fn status_from_queue(status: &str) -> SyncStatus {
    match status {
        "pending" => SyncStatus::Pending,
        "syncing" | "in_flight" => SyncStatus::Syncing,
        "retrying" => SyncStatus::Retrying,
        "conflict" => SyncStatus::Conflict,
        "failed" => SyncStatus::Failed,
        _ => SyncStatus::Failed,
    }
}

fn stable_error_code(code: &str) -> String {
    if !code.is_empty()
        && code.len() <= 128
        && code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        code.to_owned()
    } else {
        "SYNC_REMOTE_FAILURE".into()
    }
}

fn empty_snapshot() -> SyncStatusSnapshot {
    SyncStatusSnapshot {
        schema_version: SYNC_CLIENT_STATE_SCHEMA_VERSION,
        protocol_version: SYNC_PROTOCOL_VERSION,
        account_id: None,
        device_id: None,
        status: SyncStatus::AuthRequired,
        paused: false,
        phase: SyncRuntimePhase::Idle,
        pending_count: 0,
        retrying_count: 0,
        conflict_count: 0,
        failed_count: 0,
        last_completed_at: None,
        last_error_code: None,
        recommended_action: SyncRecommendedAction::SignIn,
        can_pause: false,
        can_resume: false,
        can_sync_now: false,
        can_retry: false,
        entities: Vec::new(),
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests;
