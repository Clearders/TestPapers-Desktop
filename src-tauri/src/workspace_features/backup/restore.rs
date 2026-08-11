use super::archive::{BackupArchive, BackupArchiveError};
use super::manifest::BackupManifest;
use crate::workspace_features::zip_store::ZipLimits;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) trait DatabasePreflight: Send + Sync {
    fn schema_version(&self, database: &Path) -> Result<u32, String>;
    fn migrate_in_place(&self, database: &Path, from: u32, to: u32) -> Result<(), String>;
    fn validate(&self, database: &Path) -> Result<(), String>;
}

pub(crate) trait WorkspaceHealth: Send + Sync {
    fn validate_workspace(&self, workspace: &Path) -> Result<(), String>;
}

#[derive(Clone, Debug)]
pub(crate) struct RestorePreflightRequest {
    pub(crate) staging_directory: PathBuf,
    pub(crate) supported_schema_version: u32,
    pub(crate) archive_limits: ZipLimits,
}

#[derive(Clone, Debug)]
pub(crate) struct PreparedRestore {
    pub(crate) staging_directory: PathBuf,
    pub(crate) database_path: PathBuf,
    pub(crate) manifest: BackupManifest,
}

pub(crate) fn preflight_restore(
    plaintext_archive: &[u8],
    request: &RestorePreflightRequest,
    database: &dyn DatabasePreflight,
) -> Result<PreparedRestore, RestoreError> {
    if request.supported_schema_version == 0 {
        return Err(RestoreError::InvalidRequest(
            "supported schema version must be positive".into(),
        ));
    }
    let verified = BackupArchive::inspect(plaintext_archive, request.archive_limits)
        .map_err(RestoreError::Archive)?;
    if verified.manifest.schema_version > request.supported_schema_version {
        return Err(RestoreError::NewerSchema {
            backup: verified.manifest.schema_version,
            supported: request.supported_schema_version,
        });
    }
    verified
        .extract_to(&request.staging_directory)
        .map_err(RestoreError::Archive)?;
    let database_path = request.staging_directory.join("workspace.sqlite3");
    let actual_schema = database
        .schema_version(&database_path)
        .map_err(RestoreError::Database)?;
    if actual_schema != verified.manifest.schema_version {
        return Err(RestoreError::SchemaMismatch {
            manifest: verified.manifest.schema_version,
            database: actual_schema,
        });
    }
    if actual_schema < request.supported_schema_version {
        database
            .migrate_in_place(
                &database_path,
                actual_schema,
                request.supported_schema_version,
            )
            .map_err(RestoreError::Migration)?;
        let migrated_schema = database
            .schema_version(&database_path)
            .map_err(RestoreError::Database)?;
        if migrated_schema != request.supported_schema_version {
            return Err(RestoreError::Migration(
                "migration adapter did not reach the requested schema".into(),
            ));
        }
    }
    database
        .validate(&database_path)
        .map_err(RestoreError::Database)?;
    Ok(PreparedRestore {
        staging_directory: request.staging_directory.clone(),
        database_path,
        manifest: verified.manifest,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct SwapPaths {
    pub(crate) current: PathBuf,
    pub(crate) staging: PathBuf,
    pub(crate) rollback: PathBuf,
    /// A failed installed workspace is retained here for diagnostics; it is never deleted silently.
    pub(crate) failed: PathBuf,
}

/// Installs a preflighted workspace while an external maintenance lease keeps every database
/// connection and feature queue closed. Success deliberately leaves `rollback` intact.
pub(crate) fn install_preflighted_restore(
    prepared: PreparedRestore,
    paths: &SwapPaths,
    health: &dyn WorkspaceHealth,
) -> Result<(), RestoreError> {
    validate_swap_paths(&prepared, paths)?;
    fs::rename(&paths.current, &paths.rollback).map_err(|error| RestoreError::Swap {
        phase: "preserve-current",
        detail: error.to_string(),
    })?;
    if let Err(error) = fs::rename(&paths.staging, &paths.current) {
        rollback_rename(paths, "install-staging")?;
        return Err(RestoreError::Swap {
            phase: "install-staging",
            detail: error.to_string(),
        });
    }
    if let Err(health_error) = health.validate_workspace(&paths.current) {
        fs::rename(&paths.current, &paths.failed).map_err(|error| {
            RestoreError::RollbackFailed {
                phase: "preserve-failed-install",
                detail: error.to_string(),
            }
        })?;
        fs::rename(&paths.rollback, &paths.current).map_err(|error| {
            RestoreError::RollbackFailed {
                phase: "restore-previous-workspace",
                detail: error.to_string(),
            }
        })?;
        health.validate_workspace(&paths.current).map_err(|error| {
            RestoreError::RollbackFailed {
                phase: "validate-previous-workspace",
                detail: error,
            }
        })?;
        return Err(RestoreError::HealthCheckRolledBack(health_error));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum RestoreError {
    InvalidRequest(String),
    Archive(BackupArchiveError),
    NewerSchema { backup: u32, supported: u32 },
    SchemaMismatch { manifest: u32, database: u32 },
    Database(String),
    Migration(String),
    InvalidSwap(String),
    Swap { phase: &'static str, detail: String },
    HealthCheckRolledBack(String),
    RollbackFailed { phase: &'static str, detail: String },
}

impl fmt::Display for RestoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => {
                write!(formatter, "invalid restore request: {message}")
            }
            Self::Archive(error) => write!(formatter, "restore archive failed preflight: {error}"),
            Self::NewerSchema { backup, supported } => write!(
                formatter,
                "backup schema {backup} is newer than supported schema {supported}"
            ),
            Self::SchemaMismatch { manifest, database } => write!(
                formatter,
                "manifest schema {manifest} does not match database schema {database}"
            ),
            Self::Database(message) => {
                write!(formatter, "restored database failed validation: {message}")
            }
            Self::Migration(message) => {
                write!(formatter, "restored database migration failed: {message}")
            }
            Self::InvalidSwap(message) => write!(formatter, "invalid restore swap: {message}"),
            Self::Swap { phase, detail } => {
                write!(formatter, "restore swap failed during {phase}: {detail}")
            }
            Self::HealthCheckRolledBack(message) => write!(
                formatter,
                "restored workspace failed health check and was rolled back: {message}"
            ),
            Self::RollbackFailed { phase, detail } => write!(
                formatter,
                "restore rollback failed during {phase}: {detail}"
            ),
        }
    }
}

fn validate_swap_paths(prepared: &PreparedRestore, paths: &SwapPaths) -> Result<(), RestoreError> {
    if prepared.staging_directory != paths.staging {
        return Err(RestoreError::InvalidSwap(
            "prepared staging directory does not match swap staging directory".into(),
        ));
    }
    if prepared.database_path != paths.staging.join("workspace.sqlite3")
        || !prepared.database_path.is_file()
        || !paths.current.is_dir()
        || !paths.staging.is_dir()
    {
        return Err(RestoreError::InvalidSwap(
            "current or prepared workspace is missing".into(),
        ));
    }
    if paths.rollback.exists() || paths.failed.exists() {
        return Err(RestoreError::InvalidSwap(
            "rollback or failed-install destination already exists".into(),
        ));
    }
    let current_parent = canonical_parent(&paths.current)?;
    for path in [&paths.staging, &paths.rollback, &paths.failed] {
        if canonical_parent(path)? != current_parent {
            return Err(RestoreError::InvalidSwap(
                "all swap paths must share one filesystem directory".into(),
            ));
        }
        if path.file_name().is_none() {
            return Err(RestoreError::InvalidSwap(
                "swap target has no basename".into(),
            ));
        }
    }
    Ok(())
}

fn canonical_parent(path: &Path) -> Result<PathBuf, RestoreError> {
    path.parent()
        .ok_or_else(|| RestoreError::InvalidSwap("swap path has no parent".into()))?
        .canonicalize()
        .map_err(|error| RestoreError::InvalidSwap(error.to_string()))
}

fn rollback_rename(paths: &SwapPaths, phase: &'static str) -> Result<(), RestoreError> {
    fs::rename(&paths.rollback, &paths.current).map_err(|error| RestoreError::RollbackFailed {
        phase,
        detail: error.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FileHealth;
    impl WorkspaceHealth for FileHealth {
        fn validate_workspace(&self, workspace: &Path) -> Result<(), String> {
            let marker = fs::read_to_string(workspace.join("health.txt"))
                .map_err(|error| error.to_string())?;
            (marker == "healthy")
                .then_some(())
                .ok_or_else(|| "unhealthy".into())
        }
    }

    fn prepared(root: &Path, marker: &str) -> (PreparedRestore, SwapPaths) {
        let current = root.join("workspace");
        let staging = root.join("restore-staging");
        fs::create_dir(&current).unwrap();
        fs::create_dir(&staging).unwrap();
        fs::write(current.join("health.txt"), "healthy").unwrap();
        fs::write(staging.join("health.txt"), marker).unwrap();
        fs::write(staging.join("workspace.sqlite3"), "db").unwrap();
        let manifest = BackupManifest {
            format_version: 1,
            workspace_id: "018f0000-0000-7000-8000-000000000001".into(),
            app_version: "1".into(),
            schema_version: 1,
            created_at_micros: 1,
            kind: super::super::manifest::BackupKind::Manual,
            entity_counts: Default::default(),
            files: vec![
                super::super::manifest::ManifestFile::from_bytes(
                    "workspace.sqlite3".into(),
                    super::super::manifest::BackupFileRole::Database,
                    b"db",
                ),
                super::super::manifest::ManifestFile::from_bytes(
                    "workspace.v1.json".into(),
                    super::super::manifest::BackupFileRole::WorkspaceMetadata,
                    b"{}",
                ),
            ],
        };
        (
            PreparedRestore {
                staging_directory: staging.clone(),
                database_path: staging.join("workspace.sqlite3"),
                manifest,
            },
            SwapPaths {
                current,
                staging,
                rollback: root.join("pre-restore-rollback"),
                failed: root.join("failed-restore"),
            },
        )
    }

    #[test]
    fn healthy_restore_keeps_previous_workspace_as_rollback() {
        let root = tempfile::tempdir().unwrap();
        let (prepared, paths) = prepared(root.path(), "healthy");
        install_preflighted_restore(prepared, &paths, &FileHealth).unwrap();
        assert_eq!(
            fs::read_to_string(paths.current.join("health.txt")).unwrap(),
            "healthy"
        );
        assert!(paths.rollback.join("health.txt").is_file());
    }

    #[test]
    fn failed_health_check_atomically_restores_previous_workspace() {
        let root = tempfile::tempdir().unwrap();
        let (prepared, paths) = prepared(root.path(), "broken");
        assert!(matches!(
            install_preflighted_restore(prepared, &paths, &FileHealth),
            Err(RestoreError::HealthCheckRolledBack(_))
        ));
        assert_eq!(
            fs::read_to_string(paths.current.join("health.txt")).unwrap(),
            "healthy"
        );
        assert_eq!(
            fs::read_to_string(paths.failed.join("health.txt")).unwrap(),
            "broken"
        );
    }
}
