use super::archive::{BackupArchive, BackupArchiveError};
use super::manifest::{BackupFileRole, BackupKind, BackupManifest, ManifestError, ManifestFile};
use crate::workspace_features::zip_store::ZipEntry;
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub(crate) struct BackupPayloadSource {
    pub(crate) archive_path: String,
    pub(crate) source_path: PathBuf,
    pub(crate) role: BackupFileRole,
}

pub(crate) trait ConsistentDatabaseSnapshot: Send + Sync {
    /// Must use SQLite's online backup API rather than copying the live WAL database file.
    fn snapshot_to(&self, destination: &Path) -> Result<(), String>;

    /// Reads entity counts and the exact blob/template inventory from the completed snapshot.
    fn inventory(
        &self,
        snapshot_database: &Path,
    ) -> Result<(BTreeMap<String, u64>, Vec<BackupPayloadSource>), String>;
}

#[derive(Clone, Debug)]
pub(crate) struct BackupCreateRequest {
    pub(crate) staging_directory: PathBuf,
    pub(crate) workspace_id: String,
    pub(crate) app_version: String,
    pub(crate) schema_version: u32,
    pub(crate) created_at_micros: i64,
    pub(crate) kind: BackupKind,
    pub(crate) max_file_bytes: u64,
    pub(crate) max_archive_payload_bytes: u64,
}

impl BackupCreateRequest {
    pub(crate) fn with_defaults(
        staging_directory: PathBuf,
        workspace_id: String,
        app_version: String,
        schema_version: u32,
        created_at_micros: i64,
        kind: BackupKind,
    ) -> Self {
        Self {
            staging_directory,
            workspace_id,
            app_version,
            schema_version,
            created_at_micros,
            kind,
            max_file_bytes: 512 * 1024 * 1024,
            max_archive_payload_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

pub(crate) fn create_consistent_backup(
    request: &BackupCreateRequest,
    database: &dyn ConsistentDatabaseSnapshot,
) -> Result<Vec<u8>, BackupCreateError> {
    require_empty_staging(&request.staging_directory)?;
    let database_path = request.staging_directory.join("workspace.sqlite3");
    database
        .snapshot_to(&database_path)
        .map_err(BackupCreateError::Snapshot)?;
    if !database_path.is_file() {
        return Err(BackupCreateError::Snapshot(
            "snapshot adapter did not produce workspace.sqlite3".into(),
        ));
    }
    let (entity_counts, mut inventory) = database
        .inventory(&database_path)
        .map_err(BackupCreateError::Inventory)?;
    if inventory
        .iter()
        .any(|source| source.role == BackupFileRole::Database)
    {
        return Err(BackupCreateError::Inventory(
            "inventory must not declare a second database".into(),
        ));
    }
    inventory.push(BackupPayloadSource {
        archive_path: "workspace.sqlite3".into(),
        source_path: database_path,
        role: BackupFileRole::Database,
    });
    inventory.sort_by(|left, right| left.archive_path.cmp(&right.archive_path));

    let mut total = 0_u64;
    let mut payloads = Vec::with_capacity(inventory.len());
    let mut manifest_files = Vec::with_capacity(inventory.len());
    for source in inventory {
        let metadata = fs::symlink_metadata(&source.source_path).map_err(|error| {
            BackupCreateError::Read(source.archive_path.clone(), error.to_string())
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(BackupCreateError::UnsafeSource(source.archive_path));
        }
        if metadata.len() > request.max_file_bytes {
            return Err(BackupCreateError::FileTooLarge(source.archive_path));
        }
        total = total
            .checked_add(metadata.len())
            .ok_or(BackupCreateError::PayloadTooLarge)?;
        if total > request.max_archive_payload_bytes {
            return Err(BackupCreateError::PayloadTooLarge);
        }
        let bytes = fs::read(&source.source_path).map_err(|error| {
            BackupCreateError::Read(source.archive_path.clone(), error.to_string())
        })?;
        if bytes.len() as u64 != metadata.len() {
            return Err(BackupCreateError::Read(
                source.archive_path,
                "file changed while it was being read".into(),
            ));
        }
        manifest_files.push(ManifestFile::from_bytes(
            source.archive_path.clone(),
            source.role,
            &bytes,
        ));
        payloads.push(ZipEntry::new(source.archive_path, bytes));
    }
    let manifest = BackupManifest::new(
        request.workspace_id.clone(),
        request.app_version.clone(),
        request.schema_version,
        request.created_at_micros,
        request.kind,
        entity_counts,
        manifest_files,
    )
    .map_err(BackupCreateError::Manifest)?;
    BackupArchive::build(&manifest, payloads).map_err(BackupCreateError::Archive)
}

/// Writes a completed plaintext or age-encrypted archive beside the selected target, fsyncs it,
/// and then renames it into place. Existing targets are deliberately not overwritten.
pub(crate) fn write_new_backup_atomically(
    archive: &[u8],
    target: &Path,
) -> Result<(), BackupCreateError> {
    if target.exists() {
        return Err(BackupCreateError::TargetExists);
    }
    let parent = target.parent().ok_or(BackupCreateError::InvalidTarget)?;
    if !parent.is_dir() || target.file_name().is_none() {
        return Err(BackupCreateError::InvalidTarget);
    }
    let filename = target
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !filename.ends_with(".tpbackup") && !filename.ends_with(".tpbackup.age") {
        return Err(BackupCreateError::InvalidTarget);
    }
    let temporary = parent.join(format!(
        ".{}.{}.partial",
        target
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("backup"),
        Uuid::now_v7()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| BackupCreateError::Write(error.to_string()))?;
    let write_result = file.write_all(archive).and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(BackupCreateError::Write(error.to_string()));
    }
    if let Err(error) = fs::rename(&temporary, target) {
        let _ = fs::remove_file(&temporary);
        return Err(BackupCreateError::Write(error.to_string()));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum BackupCreateError {
    InvalidStaging(String),
    Snapshot(String),
    Inventory(String),
    UnsafeSource(String),
    FileTooLarge(String),
    PayloadTooLarge,
    Read(String, String),
    Manifest(ManifestError),
    Archive(BackupArchiveError),
    TargetExists,
    InvalidTarget,
    Write(String),
}

impl fmt::Display for BackupCreateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidStaging(message) => {
                write!(formatter, "invalid backup staging directory: {message}")
            }
            Self::Snapshot(message) => write!(formatter, "database snapshot failed: {message}"),
            Self::Inventory(message) => write!(formatter, "backup inventory failed: {message}"),
            Self::UnsafeSource(path) => write!(formatter, "backup source is unsafe: {path}"),
            Self::FileTooLarge(path) => {
                write!(formatter, "backup source exceeds the size limit: {path}")
            }
            Self::PayloadTooLarge => formatter.write_str("backup payload exceeds the size limit"),
            Self::Read(path, error) => {
                write!(formatter, "could not read backup source {path}: {error}")
            }
            Self::Manifest(error) => write!(formatter, "could not build backup manifest: {error}"),
            Self::Archive(error) => write!(formatter, "could not build backup archive: {error}"),
            Self::TargetExists => formatter.write_str("backup target already exists"),
            Self::InvalidTarget => {
                formatter.write_str("backup target must be a .tpbackup or .tpbackup.age file")
            }
            Self::Write(message) => write!(formatter, "could not write backup: {message}"),
        }
    }
}

fn require_empty_staging(path: &Path) -> Result<(), BackupCreateError> {
    if !path.is_dir() {
        return Err(BackupCreateError::InvalidStaging(
            "directory does not exist".into(),
        ));
    }
    if fs::read_dir(path)
        .map_err(|error| BackupCreateError::InvalidStaging(error.to_string()))?
        .next()
        .is_some()
    {
        return Err(BackupCreateError::InvalidStaging(
            "directory is not empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_features::backup::archive::BackupArchive;
    use crate::workspace_features::zip_store::ZipLimits;

    struct FakeDatabase;

    impl ConsistentDatabaseSnapshot for FakeDatabase {
        fn snapshot_to(&self, destination: &Path) -> Result<(), String> {
            fs::write(destination, b"sqlite-snapshot").map_err(|error| error.to_string())
        }

        fn inventory(
            &self,
            snapshot: &Path,
        ) -> Result<(BTreeMap<String, u64>, Vec<BackupPayloadSource>), String> {
            assert!(snapshot.is_file());
            let metadata = snapshot.parent().unwrap().join("workspace.v1.json");
            fs::write(&metadata, b"{}").map_err(|error| error.to_string())?;
            Ok((
                BTreeMap::from([("questions".into(), 3)]),
                vec![BackupPayloadSource {
                    archive_path: "workspace.v1.json".into(),
                    source_path: metadata,
                    role: BackupFileRole::WorkspaceMetadata,
                }],
            ))
        }
    }

    #[test]
    fn creates_archive_from_consistent_snapshot_adapter() {
        let staging = tempfile::tempdir().unwrap();
        let request = BackupCreateRequest::with_defaults(
            staging.path().to_path_buf(),
            "018f0000-0000-7000-8000-000000000001".into(),
            "1.0.0".into(),
            1,
            1,
            BackupKind::Manual,
        );
        let archive = create_consistent_backup(&request, &FakeDatabase).unwrap();
        let verified = BackupArchive::inspect(&archive, ZipLimits::default()).unwrap();
        assert_eq!(verified.manifest.entity_counts["questions"], 3);
        assert_eq!(verified.files["workspace.sqlite3"], b"sqlite-snapshot");
    }

    #[test]
    fn atomic_writer_refuses_existing_target() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("manual.tpbackup");
        write_new_backup_atomically(b"one", &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"one");
        assert!(matches!(
            write_new_backup_atomically(b"two", &target),
            Err(BackupCreateError::TargetExists)
        ));
    }
}
