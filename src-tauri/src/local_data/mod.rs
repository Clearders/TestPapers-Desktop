mod attachments;
mod backup;
mod canonical;
mod error;
mod imports;
mod migration;
mod model;
mod papers;
mod questions;
mod sync_delivery;
mod sync_state;

use std::{
    path::PathBuf,
    sync::{Mutex, MutexGuard},
};

use rusqlite::Connection;

pub(crate) use attachments::MAX_ATTACHMENT_BYTES;
pub(crate) use backup::{BackupBlobSource, BackupInventory};
pub(crate) use error::{LocalDataError, LocalDataResult};
pub(crate) use imports::{ImportCommitResult, ImportInspection, ImportRow};
pub(crate) use migration::{MigrationReport, LATEST_SCHEMA_VERSION};
pub(crate) use model::{
    AttachmentRecord, CreateQuestion, DeletedFilter, Difficulty, EssayBlankSpace, HistoryAction,
    MutationBase, NewQuestionAttachment, PendingMutation, QuestionContent, QuestionRecord,
    QuestionRevision, QuestionSearch, QuestionSearchPage, QuestionType, ReplicationScope,
    SyncQueueState, UpdateQuestion,
};
pub(crate) use sync_delivery::{
    PreparedConflictResolution, PreparedSyncBatch, PreparedSyncOperation, RemoteEntityBaseline,
    RemoteSyncChange, SyncConflictRecoveryRecord, SyncOperationOutcome,
};
pub(crate) use sync_state::{
    StartupRecoveryReport, SyncControlData, SyncDeviceState, SyncEntityDeliveryState,
    SyncRuntimePhase,
};

#[derive(Clone, Debug)]
pub(crate) struct StoreConfig {
    pub(crate) database_path: PathBuf,
    pub(crate) blob_root: PathBuf,
    pub(crate) workspace_id: String,
    pub(crate) local_principal_id: String,
}

pub(crate) struct LocalDataStore {
    connection: Mutex<Connection>,
    database_path: PathBuf,
    blob_root: PathBuf,
    workspace_id: String,
    local_principal_id: String,
    migration_report: MigrationReport,
    startup_recovery_report: StartupRecoveryReport,
}

impl LocalDataStore {
    pub(crate) fn open(config: StoreConfig) -> LocalDataResult<Self> {
        let (mut connection, migration_report) = migration::open_migrated_database(&config)?;
        let startup_recovery_report = sync_state::recover_startup(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            database_path: config.database_path,
            blob_root: config.blob_root,
            workspace_id: config.workspace_id,
            local_principal_id: config.local_principal_id,
            migration_report,
            startup_recovery_report,
        })
    }

    pub(crate) fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub(crate) fn local_principal_id(&self) -> &str {
        &self.local_principal_id
    }

    pub(crate) fn migration_report(&self) -> &MigrationReport {
        &self.migration_report
    }

    pub(crate) fn startup_recovery_report(&self) -> StartupRecoveryReport {
        self.startup_recovery_report
    }

    pub(crate) fn verify_integrity(&self) -> LocalDataResult<()> {
        migration::validate_database(&self.connection())
    }

    pub(crate) fn checkpoint(&self) -> LocalDataResult<()> {
        let busy: i64 =
            self.connection()
                .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
        if busy == 0 {
            Ok(())
        } else {
            Err(LocalDataError::Busy(
                "the SQLite write-ahead log could not be checkpointed".into(),
            ))
        }
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod performance_tests;
