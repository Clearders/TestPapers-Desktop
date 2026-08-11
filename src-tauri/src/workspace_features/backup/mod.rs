//! Consistent workspace backup, restore and data-directory management (CLE-28).

mod archive;
mod data_directory;
mod encryption;
mod manifest;
mod restore;
mod scheduler;
mod service;

pub(crate) use archive::{BackupArchive, BackupArchiveError, VerifiedBackup};
pub(crate) use data_directory::{
    migrate_data_directory, DataDirectoryError, DataDirectoryPlan, DestinationProbe,
    WorkspacePointer,
};
pub(crate) use encryption::{
    AgeBackend, BackupEncryption, BackupEncryptionError, KeychainIdentityProvider, SecretBytes,
    UnlockMaterial,
};
pub(crate) use manifest::{
    BackupFileRole, BackupKind, BackupManifest, ManifestError, ManifestFile, BACKUP_FORMAT_VERSION,
};
pub(crate) use restore::{
    install_preflighted_restore, preflight_restore, DatabasePreflight, PreparedRestore,
    RestoreError, RestorePreflightRequest, SwapPaths, WorkspaceHealth,
};
pub(crate) use scheduler::{
    AutomaticBackupState, BackupScheduleConfig, ScheduleError, ScheduledBackupCandidate,
};
pub(crate) use service::{
    create_consistent_backup, write_new_backup_atomically, BackupCreateError, BackupCreateRequest,
    BackupPayloadSource, ConsistentDatabaseSnapshot,
};
