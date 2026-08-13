//! Persistent Sync v1 pull/apply/ack/push worker.
//!
//! Network calls happen outside the SQLite mutex. Local page application and queue transitions are
//! short immediate transactions, so editing remains available while transport is slow or offline.

use std::{collections::HashMap, fmt, sync::Arc, thread};

use serde_json::Value;
use testpapers_cloud_api::{
    adapter::CloudApi,
    apis::{self, sync_api},
    models,
};

use crate::local_data::{
    LocalDataError, LocalDataStore, PreparedSyncBatch, RemoteSyncChange, SyncOperationOutcome,
    SyncRuntimePhase,
};

const PROTOCOL_VERSION: i32 = 1;
const PAGE_SIZE: i32 = 100;
const MAX_PAGES_PER_RUN: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TransportErrorKind {
    Offline,
    AuthenticationRequired,
    DeviceRevoked,
    CursorExpired,
    SnapshotExpired,
    RateLimited,
    Fatal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransportError {
    pub(crate) kind: TransportErrorKind,
    pub(crate) code: String,
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "sync transport failed: {}", self.code)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PullPage {
    pub(crate) changes: Vec<RemoteSyncChange>,
    pub(crate) next_cursor: String,
    pub(crate) has_more: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SnapshotPage {
    pub(crate) snapshot_id: String,
    pub(crate) entries: Vec<RemoteSyncChange>,
    pub(crate) next_cursor: String,
    pub(crate) has_more: bool,
    pub(crate) resume_cursor: String,
}

pub(crate) trait SyncTransport: Send + Sync + 'static {
    fn pull(&self, cursor: Option<&str>, page_size: i32) -> Result<PullPage, TransportError>;
    fn acknowledge(&self, device_id: &str, cursor: &str) -> Result<(), TransportError>;
    fn snapshot(
        &self,
        cursor: Option<&str>,
        page_size: i32,
    ) -> Result<SnapshotPage, TransportError>;
    fn push(
        &self,
        device_id: &str,
        batch: &PreparedSyncBatch,
    ) -> Result<Vec<SyncOperationOutcome>, TransportError>;
}

pub(crate) struct CloudSyncTransport {
    api: CloudApi,
    runtime: tokio::runtime::Runtime,
}

impl CloudSyncTransport {
    pub(crate) fn new(
        base_path: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            api: CloudApi::new(base_path, access_token),
            runtime: tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?,
        })
    }
}

impl SyncTransport for CloudSyncTransport {
    fn pull(&self, cursor: Option<&str>, page_size: i32) -> Result<PullPage, TransportError> {
        let response = self
            .runtime
            .block_on(sync_api::pull_sync_changes(
                self.api.configuration(),
                sync_api::PullSyncChangesParams {
                    cursor: cursor.map(str::to_owned),
                    page_size: Some(page_size),
                },
            ))
            .map_err(map_api_error)?;
        let data = *response.data;
        validate_protocol(data.protocol_version)?;
        Ok(PullPage {
            changes: data
                .changes
                .into_iter()
                .map(remote_change_from_model)
                .collect::<Result<_, _>>()?,
            next_cursor: data.next_cursor,
            has_more: data.has_more,
        })
    }

    fn acknowledge(&self, device_id: &str, cursor: &str) -> Result<(), TransportError> {
        let response = self
            .runtime
            .block_on(sync_api::ack_sync_cursor(
                self.api.configuration(),
                sync_api::AckSyncCursorParams {
                    sync_ack_request: models::SyncAckRequest::new(
                        cursor.to_owned(),
                        device_id.to_owned(),
                        PROTOCOL_VERSION,
                    ),
                },
            ))
            .map_err(map_api_error)?;
        validate_protocol(response.data.protocol_version)
    }

    fn snapshot(
        &self,
        cursor: Option<&str>,
        page_size: i32,
    ) -> Result<SnapshotPage, TransportError> {
        let response = self
            .runtime
            .block_on(sync_api::get_sync_snapshot(
                self.api.configuration(),
                sync_api::GetSyncSnapshotParams {
                    cursor: cursor.map(str::to_owned),
                    page_size: Some(page_size),
                },
            ))
            .map_err(map_api_error)?;
        let data = *response.data;
        validate_protocol(data.protocol_version)?;
        Ok(SnapshotPage {
            snapshot_id: data.snapshot_id,
            entries: data
                .entries
                .into_iter()
                .map(remote_change_from_model)
                .collect::<Result<_, _>>()?,
            next_cursor: data.next_cursor,
            has_more: data.has_more,
            resume_cursor: data.resume_cursor,
        })
    }

    fn push(
        &self,
        device_id: &str,
        batch: &PreparedSyncBatch,
    ) -> Result<Vec<SyncOperationOutcome>, TransportError> {
        let mutations = batch
            .operations
            .iter()
            .map(operation_to_model)
            .collect::<Result<Vec<_>, _>>()?;
        let response = self
            .runtime
            .block_on(sync_api::push_sync_mutations(
                self.api.configuration(),
                sync_api::PushSyncMutationsParams {
                    sync_push_request: models::SyncPushRequest::new(
                        batch.batch_id.clone(),
                        device_id.to_owned(),
                        mutations,
                        PROTOCOL_VERSION,
                    ),
                },
            ))
            .map_err(map_api_error)?;
        let data = *response.data;
        validate_protocol(data.protocol_version)?;
        if data.batch_id != batch.batch_id {
            return Err(fatal("SYNC_RESPONSE_BATCH_MISMATCH"));
        }
        data.results
            .into_iter()
            .map(|result| {
                let response =
                    serde_json::to_value(&result).map_err(|_| fatal("SYNC_RESPONSE_INVALID"))?;
                Ok(SyncOperationOutcome {
                    operation_id: result.operation_id,
                    status: result.status.to_string(),
                    entity_version: result.entity_version.flatten().map(i64::from),
                    content_hash: result.content_hash.flatten(),
                    conflict_id: result.conflict_id.flatten(),
                    response,
                })
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SyncRunReport {
    pub(crate) pulled_changes: u32,
    pub(crate) pushed_operations: u32,
    pub(crate) rebuilt_snapshot: bool,
    pub(crate) deferred: bool,
}

#[derive(Debug)]
pub(crate) enum SyncWorkerError {
    Local(LocalDataError),
    Transport(TransportError),
}

impl From<LocalDataError> for SyncWorkerError {
    fn from(value: LocalDataError) -> Self {
        Self::Local(value)
    }
}

pub(crate) struct SyncWorker<T: SyncTransport> {
    store: Arc<LocalDataStore>,
    transport: Arc<T>,
    account_id: String,
    device_id: String,
}

impl<T: SyncTransport> SyncWorker<T> {
    pub(crate) fn new(
        store: Arc<LocalDataStore>,
        transport: Arc<T>,
        account_id: String,
        device_id: String,
    ) -> Result<Self, LocalDataError> {
        store.register_sync_device(&account_id, &device_id)?;
        Ok(Self {
            store,
            transport,
            account_id,
            device_id,
        })
    }

    /// Runs a bounded synchronization pass. Callers normally invoke this on a background thread.
    pub(crate) fn run_once(&self) -> Result<SyncRunReport, SyncWorkerError> {
        let mut report = SyncRunReport::default();
        self.store.set_sync_runtime_phase(
            &self.account_id,
            &self.device_id,
            SyncRuntimePhase::Pull,
            None,
        )?;
        let cursor = self
            .store
            .sync_device_state(&self.account_id, &self.device_id)?
            .and_then(|state| state.acknowledged_cursor);
        match self.pull_pages(cursor.as_deref(), &mut report) {
            Ok(()) => {}
            Err(SyncWorkerError::Transport(error))
                if error.kind == TransportErrorKind::CursorExpired =>
            {
                self.rebuild_snapshot(&mut report)?;
            }
            Err(error) => return self.finish_error(error, &mut report),
        }

        self.store.set_sync_runtime_phase(
            &self.account_id,
            &self.device_id,
            SyncRuntimePhase::Push,
            None,
        )?;
        if let Some(batch) =
            self.store
                .prepare_sync_batch(&self.account_id, &self.device_id, 100)?
        {
            self.store.set_sync_runtime_phase(
                &self.account_id,
                &self.device_id,
                SyncRuntimePhase::Push,
                Some(&batch.batch_id),
            )?;
            match self.transport.push(&self.device_id, &batch) {
                Ok(outcomes) => {
                    self.store.set_sync_runtime_phase(
                        &self.account_id,
                        &self.device_id,
                        SyncRuntimePhase::Settle,
                        Some(&batch.batch_id),
                    )?;
                    self.store
                        .settle_sync_batch(&self.account_id, &batch.batch_id, &outcomes)?;
                    report.pushed_operations = u32::try_from(outcomes.len()).unwrap_or(u32::MAX);
                }
                Err(error) => {
                    self.store.retry_sync_batch(&batch.batch_id, &error.code)?;
                    return self.finish_error(SyncWorkerError::Transport(error), &mut report);
                }
            }
        }
        self.store
            .set_sync_authentication_state(&self.account_id, &self.device_id, "ready")?;
        self.store.set_sync_runtime_phase(
            &self.account_id,
            &self.device_id,
            SyncRuntimePhase::Idle,
            None,
        )?;
        Ok(report)
    }

    pub(crate) fn run_once_in_background(
        self: Arc<Self>,
    ) -> thread::JoinHandle<Result<SyncRunReport, SyncWorkerError>> {
        thread::spawn(move || self.run_once())
    }

    fn pull_pages(
        &self,
        initial_cursor: Option<&str>,
        report: &mut SyncRunReport,
    ) -> Result<(), SyncWorkerError> {
        let mut cursor = initial_cursor.map(str::to_owned);
        for _ in 0..MAX_PAGES_PER_RUN {
            let page = self
                .transport
                .pull(cursor.as_deref(), PAGE_SIZE)
                .map_err(SyncWorkerError::Transport)?;
            self.store.set_sync_runtime_phase(
                &self.account_id,
                &self.device_id,
                SyncRuntimePhase::Apply,
                None,
            )?;
            self.store.apply_remote_page(
                &self.account_id,
                &self.device_id,
                &page.changes,
                &page.next_cursor,
            )?;
            report.pulled_changes = report
                .pulled_changes
                .saturating_add(u32::try_from(page.changes.len()).unwrap_or(u32::MAX));
            self.store.set_sync_runtime_phase(
                &self.account_id,
                &self.device_id,
                SyncRuntimePhase::Ack,
                None,
            )?;
            self.transport
                .acknowledge(&self.device_id, &page.next_cursor)
                .map_err(SyncWorkerError::Transport)?;
            self.store
                .commit_pulled_cursor(&self.account_id, &self.device_id)?;
            if !page.has_more {
                return Ok(());
            }
            cursor = Some(page.next_cursor);
            self.store.set_sync_runtime_phase(
                &self.account_id,
                &self.device_id,
                SyncRuntimePhase::Pull,
                None,
            )?;
        }
        Err(SyncWorkerError::Transport(fatal(
            "SYNC_PAGE_LIMIT_EXCEEDED",
        )))
    }

    fn rebuild_snapshot(&self, report: &mut SyncRunReport) -> Result<(), SyncWorkerError> {
        let mut cursor = None;
        let mut rebuild_id: Option<(String, String)> = None;
        for _ in 0..MAX_PAGES_PER_RUN {
            let page = self
                .transport
                .snapshot(cursor.as_deref(), PAGE_SIZE)
                .map_err(SyncWorkerError::Transport)?;
            let id = match &rebuild_id {
                Some((expected_snapshot, rebuild_id)) => {
                    if expected_snapshot != &page.snapshot_id {
                        return Err(SyncWorkerError::Transport(fatal(
                            "SYNC_SNAPSHOT_ID_CHANGED",
                        )));
                    }
                    rebuild_id.clone()
                }
                None => {
                    let id = self.store.begin_snapshot_rebuild(
                        &self.account_id,
                        &self.device_id,
                        &page.snapshot_id,
                    )?;
                    rebuild_id = Some((page.snapshot_id.clone(), id.clone()));
                    id
                }
            };
            self.store
                .append_snapshot_page(&id, &page.entries, &page.resume_cursor)?;
            if !page.has_more {
                let local_cursor = self.store.complete_snapshot_rebuild(&id)?;
                if local_cursor != page.resume_cursor {
                    return Err(SyncWorkerError::Transport(fatal(
                        "SYNC_SNAPSHOT_CURSOR_MISMATCH",
                    )));
                }
                self.transport
                    .acknowledge(&self.device_id, &local_cursor)
                    .map_err(SyncWorkerError::Transport)?;
                self.store
                    .commit_pulled_cursor(&self.account_id, &self.device_id)?;
                report.rebuilt_snapshot = true;
                return Ok(());
            }
            cursor = Some(page.next_cursor);
        }
        Err(SyncWorkerError::Transport(fatal(
            "SYNC_SNAPSHOT_PAGE_LIMIT_EXCEEDED",
        )))
    }

    fn finish_error(
        &self,
        error: SyncWorkerError,
        report: &mut SyncRunReport,
    ) -> Result<SyncRunReport, SyncWorkerError> {
        if let SyncWorkerError::Transport(transport) = &error {
            let auth_state = match transport.kind {
                TransportErrorKind::AuthenticationRequired => Some("required"),
                TransportErrorKind::DeviceRevoked => Some("revoked"),
                _ => None,
            };
            if let Some(state) = auth_state {
                self.store.set_sync_authentication_state(
                    &self.account_id,
                    &self.device_id,
                    state,
                )?;
            }
            if matches!(
                transport.kind,
                TransportErrorKind::Offline | TransportErrorKind::RateLimited
            ) {
                report.deferred = true;
            }
        }
        self.store.set_sync_runtime_phase(
            &self.account_id,
            &self.device_id,
            SyncRuntimePhase::Idle,
            None,
        )?;
        if report.deferred {
            Ok(*report)
        } else {
            Err(error)
        }
    }
}

fn operation_to_model(
    operation: &crate::local_data::PreparedSyncOperation,
) -> Result<models::SyncMutation, TransportError> {
    let mut mutation = models::SyncMutation::new(
        operation.entity_id.clone(),
        parse_entity_type(&operation.entity_type)?,
        parse_mutation_kind(&operation.kind)?,
        operation.operation_id.clone(),
    );
    mutation.base_version = operation
        .base_version
        .map(|version| i32::try_from(version).map(Some))
        .transpose()
        .map_err(|_| fatal("SYNC_VERSION_OUT_OF_RANGE"))?;
    mutation.base_content_hash = operation.base_content_hash.clone().map(Some);
    mutation.depends_on = Some(operation.dependencies.clone());
    mutation.payload = operation
        .payload
        .clone()
        .map(|payload| {
            serde_json::from_value::<HashMap<String, Value>>(payload)
                .map(Some)
                .map_err(|_| fatal("SYNC_PAYLOAD_NOT_OBJECT"))
        })
        .transpose()?;
    if operation.payload.is_none() {
        mutation.payload = Some(None);
    }
    Ok(mutation)
}

fn remote_change_from_model(
    change: models::SyncChange,
) -> Result<RemoteSyncChange, TransportError> {
    let snapshot = change
        .snapshot
        .flatten()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| fatal("SYNC_RESPONSE_INVALID"))?;
    Ok(RemoteSyncChange {
        sequence: change.sequence,
        entity_type: change.entity_type.to_string(),
        entity_id: change.entity_id,
        kind: change.kind.to_string(),
        version: i64::from(change.version),
        content_hash: change.content_hash,
        snapshot,
        updated_at: change.updated_at.timestamp_micros(),
    })
}

fn parse_entity_type(value: &str) -> Result<models::SyncEntityType, TransportError> {
    match value {
        "question" => Ok(models::SyncEntityType::Question),
        "paper" => Ok(models::SyncEntityType::Paper),
        "draft" => Ok(models::SyncEntityType::Draft),
        "attachment" => Ok(models::SyncEntityType::Attachment),
        "comment" => Ok(models::SyncEntityType::Comment),
        "favorite" => Ok(models::SyncEntityType::Favorite),
        "setting" => Ok(models::SyncEntityType::Setting),
        _ => Err(fatal("SYNC_ENTITY_TYPE_INVALID")),
    }
}

fn parse_mutation_kind(value: &str) -> Result<models::SyncMutationKind, TransportError> {
    match value {
        "create" => Ok(models::SyncMutationKind::Create),
        "update" => Ok(models::SyncMutationKind::Update),
        "delete" => Ok(models::SyncMutationKind::Delete),
        "restore" => Ok(models::SyncMutationKind::Restore),
        "rename" => Ok(models::SyncMutationKind::Rename),
        "attach" => Ok(models::SyncMutationKind::Attach),
        "detach" => Ok(models::SyncMutationKind::Detach),
        _ => Err(fatal("SYNC_MUTATION_KIND_INVALID")),
    }
}

fn validate_protocol(version: i32) -> Result<(), TransportError> {
    if version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(fatal("SYNC_PROTOCOL_UNSUPPORTED"))
    }
}

fn map_api_error<T>(error: apis::Error<T>) -> TransportError {
    match error {
        apis::Error::ResponseError(response) => {
            let code = serde_json::from_str::<Value>(&response.content)
                .ok()
                .and_then(|value| {
                    value
                        .get("error")
                        .and_then(|error| error.get("code"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
                .unwrap_or_else(|| format!("HTTP_{}", response.status.as_u16()));
            let kind = match (response.status.as_u16(), code.as_str()) {
                (401, "SYNC_DEVICE_REQUIRED") | (403, "SYNC_DEVICE_REQUIRED") => {
                    TransportErrorKind::DeviceRevoked
                }
                (401, _) => TransportErrorKind::AuthenticationRequired,
                (410, "SYNC_CURSOR_EXPIRED") => TransportErrorKind::CursorExpired,
                (410, "SYNC_SNAPSHOT_EXPIRED") => TransportErrorKind::SnapshotExpired,
                (429, _) => TransportErrorKind::RateLimited,
                _ => TransportErrorKind::Fatal,
            };
            TransportError { kind, code }
        }
        apis::Error::Reqwest(_) | apis::Error::Io(_) => TransportError {
            kind: TransportErrorKind::Offline,
            code: "SYNC_TRANSPORT_UNAVAILABLE".into(),
        },
        apis::Error::Serde(_) => fatal("SYNC_RESPONSE_INVALID"),
    }
}

fn fatal(code: &str) -> TransportError {
    TransportError {
        kind: TransportErrorKind::Fatal,
        code: code.into(),
    }
}

#[cfg(test)]
mod tests;
