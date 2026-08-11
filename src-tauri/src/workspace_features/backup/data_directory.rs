use crate::workspace_features::hash::sha256_file;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::restore::WorkspaceHealth;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DestinationProbe {
    pub(crate) writable: bool,
    pub(crate) available_bytes: u64,
    pub(crate) same_volume: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct DataDirectoryPlan {
    pub(crate) source: PathBuf,
    pub(crate) destination: PathBuf,
    pub(crate) staging: PathBuf,
    pub(crate) failed: PathBuf,
    pub(crate) required_bytes: u64,
    pub(crate) same_volume: bool,
}

impl DataDirectoryPlan {
    pub(crate) fn inspect(
        source: &Path,
        destination: &Path,
        protected_backup_directory: Option<&Path>,
        required_bytes: u64,
        probe: DestinationProbe,
    ) -> Result<Self, DataDirectoryError> {
        let source = source
            .canonicalize()
            .map_err(|error| DataDirectoryError::InvalidSource(error.to_string()))?;
        if !source.is_dir() {
            return Err(DataDirectoryError::InvalidSource(
                "source is not a directory".into(),
            ));
        }
        if !destination.is_absolute() || destination.file_name().is_none() || destination.exists() {
            return Err(DataDirectoryError::InvalidDestination(
                "destination must be a new absolute directory".into(),
            ));
        }
        let parent = destination
            .parent()
            .ok_or_else(|| {
                DataDirectoryError::InvalidDestination("destination has no parent".into())
            })?
            .canonicalize()
            .map_err(|error| DataDirectoryError::InvalidDestination(error.to_string()))?;
        let destination = parent.join(destination.file_name().expect("checked basename"));
        if destination.starts_with(&source) || source.starts_with(&destination) {
            return Err(DataDirectoryError::InvalidDestination(
                "destination and current workspace must not contain each other".into(),
            ));
        }
        if let Some(backup) = protected_backup_directory {
            let backup = backup
                .canonicalize()
                .map_err(|error| DataDirectoryError::InvalidDestination(error.to_string()))?;
            if destination.starts_with(&backup) || backup.starts_with(&destination) {
                return Err(DataDirectoryError::InvalidDestination(
                    "destination and backup directory must not contain each other".into(),
                ));
            }
        }
        if !probe.writable {
            return Err(DataDirectoryError::NotWritable);
        }
        let required_with_margin = required_bytes
            .checked_add(required_bytes / 10)
            .ok_or(DataDirectoryError::SizeOverflow)?;
        if probe.available_bytes < required_with_margin {
            return Err(DataDirectoryError::InsufficientSpace {
                required: required_with_margin,
                available: probe.available_bytes,
            });
        }
        let nonce = Uuid::now_v7();
        let staging = parent.join(format!(".testpapers-migration-{nonce}.staging"));
        let failed = parent.join(format!(".testpapers-migration-{nonce}.failed"));
        Ok(Self {
            source,
            destination,
            staging,
            failed,
            required_bytes,
            same_volume: probe.same_volume,
        })
    }
}

pub(crate) trait WorkspacePointer: Send + Sync {
    /// Atomically persists the bootstrap pointer only after the new copy passes health checks.
    fn activate(&self, destination: &Path) -> Result<(), String>;
}

/// Copies to a sibling staging directory, verifies every byte, atomically installs the staging
/// directory, validates it, and finally changes the bootstrap pointer. The source is always kept as
/// the rollback workspace until a separate user-confirmed cleanup operation.
pub(crate) fn migrate_data_directory(
    plan: &DataDirectoryPlan,
    health: &dyn WorkspaceHealth,
    pointer: &dyn WorkspacePointer,
) -> Result<(), DataDirectoryError> {
    if plan.destination.exists() || plan.staging.exists() || plan.failed.exists() {
        return Err(DataDirectoryError::DestinationOccupied);
    }
    fs::create_dir(&plan.staging).map_err(DataDirectoryError::Io)?;
    if let Err(error) = copy_tree_verified(&plan.source, &plan.staging) {
        let _ = fs::rename(&plan.staging, &plan.failed);
        return Err(error);
    }
    fs::rename(&plan.staging, &plan.destination).map_err(DataDirectoryError::Io)?;
    if let Err(error) = health.validate_workspace(&plan.destination) {
        fs::rename(&plan.destination, &plan.failed).map_err(DataDirectoryError::Io)?;
        return Err(DataDirectoryError::Health(error));
    }
    if let Err(error) = pointer.activate(&plan.destination) {
        fs::rename(&plan.destination, &plan.failed).map_err(DataDirectoryError::Io)?;
        return Err(DataDirectoryError::Pointer(error));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum DataDirectoryError {
    InvalidSource(String),
    InvalidDestination(String),
    NotWritable,
    SizeOverflow,
    InsufficientSpace { required: u64, available: u64 },
    DestinationOccupied,
    Symlink(PathBuf),
    UnsupportedFile(PathBuf),
    Verification(PathBuf),
    Health(String),
    Pointer(String),
    Io(io::Error),
}

impl fmt::Display for DataDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSource(message) => {
                write!(formatter, "invalid source data directory: {message}")
            }
            Self::InvalidDestination(message) => {
                write!(formatter, "invalid destination data directory: {message}")
            }
            Self::NotWritable => formatter.write_str("destination data directory is not writable"),
            Self::SizeOverflow => formatter.write_str("workspace size overflowed"),
            Self::InsufficientSpace {
                required,
                available,
            } => write!(
                formatter,
                "destination needs {required} bytes but only {available} are available"
            ),
            Self::DestinationOccupied => {
                formatter.write_str("migration staging or destination already exists")
            }
            Self::Symlink(path) => write!(
                formatter,
                "workspace migration does not follow symbolic link {}",
                path.display()
            ),
            Self::UnsupportedFile(path) => write!(
                formatter,
                "workspace contains unsupported file type {}",
                path.display()
            ),
            Self::Verification(path) => write!(
                formatter,
                "copied file failed verification: {}",
                path.display()
            ),
            Self::Health(message) => write!(
                formatter,
                "migrated workspace failed health check: {message}"
            ),
            Self::Pointer(message) => write!(
                formatter,
                "could not activate migrated workspace: {message}"
            ),
            Self::Io(error) => write!(formatter, "data-directory migration failed: {error}"),
        }
    }
}

fn copy_tree_verified(source: &Path, destination: &Path) -> Result<(), DataDirectoryError> {
    let mut entries = fs::read_dir(source)
        .map_err(DataDirectoryError::Io)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(DataDirectoryError::Io)?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let source_path = entry.path();
        let metadata = fs::symlink_metadata(&source_path).map_err(DataDirectoryError::Io)?;
        let destination_path = destination.join(entry.file_name());
        if metadata.file_type().is_symlink() {
            return Err(DataDirectoryError::Symlink(source_path));
        }
        if metadata.is_dir() {
            fs::create_dir(&destination_path).map_err(DataDirectoryError::Io)?;
            copy_tree_verified(&source_path, &destination_path)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(DataDirectoryError::UnsupportedFile(source_path));
        }
        copy_file_synced(&source_path, &destination_path)?;
        let source_hash = sha256_file(&source_path).map_err(DataDirectoryError::Io)?;
        let destination_hash = sha256_file(&destination_path).map_err(DataDirectoryError::Io)?;
        if source_hash != destination_hash {
            return Err(DataDirectoryError::Verification(destination_path));
        }
    }
    Ok(())
}

fn copy_file_synced(source: &Path, destination: &Path) -> Result<(), DataDirectoryError> {
    let mut input = fs::File::open(source).map_err(DataDirectoryError::Io)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(DataDirectoryError::Io)?;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input.read(&mut buffer).map_err(DataDirectoryError::Io)?;
        if count == 0 {
            break;
        }
        output
            .write_all(&buffer[..count])
            .map_err(DataDirectoryError::Io)?;
    }
    output.sync_all().map_err(DataDirectoryError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct Healthy;
    impl WorkspaceHealth for Healthy {
        fn validate_workspace(&self, workspace: &Path) -> Result<(), String> {
            workspace
                .join("workspace.sqlite3")
                .is_file()
                .then_some(())
                .ok_or_else(|| "missing database".into())
        }
    }

    #[derive(Default)]
    struct Pointer(Mutex<Option<PathBuf>>);
    impl WorkspacePointer for Pointer {
        fn activate(&self, destination: &Path) -> Result<(), String> {
            *self.0.lock().unwrap() = Some(destination.to_path_buf());
            Ok(())
        }
    }

    #[test]
    fn migration_verifies_copy_activates_pointer_and_keeps_source() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("workspace");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("workspace.sqlite3"), b"db").unwrap();
        fs::create_dir(source.join("blobs")).unwrap();
        fs::write(source.join("blobs/hash"), b"blob").unwrap();
        let destination = root.path().join("moved-workspace");
        let plan = DataDirectoryPlan::inspect(
            &source,
            &destination,
            None,
            6,
            DestinationProbe {
                writable: true,
                available_bytes: 100,
                same_volume: true,
            },
        )
        .unwrap();
        let activated_destination = plan.destination.clone();
        let pointer = Pointer::default();
        migrate_data_directory(&plan, &Healthy, &pointer).unwrap();
        assert!(source.join("workspace.sqlite3").is_file());
        assert_eq!(fs::read(destination.join("blobs/hash")).unwrap(), b"blob");
        assert_eq!(*pointer.0.lock().unwrap(), Some(activated_destination));
    }

    #[test]
    fn plan_rejects_destination_inside_workspace() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("workspace");
        fs::create_dir(&source).unwrap();
        let error = DataDirectoryPlan::inspect(
            &source,
            &source.join("nested"),
            None,
            1,
            DestinationProbe {
                writable: true,
                available_bytes: 100,
                same_volume: true,
            },
        )
        .unwrap_err();
        assert!(matches!(error, DataDirectoryError::InvalidDestination(_)));
    }
}
