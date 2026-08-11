use super::manifest::{BackupManifest, ManifestError};
use crate::workspace_features::hash::sha256;
use crate::workspace_features::zip_store::{
    read_stored_zip, write_stored_zip, ZipEntry, ZipError, ZipLimits,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) struct BackupArchive;

impl BackupArchive {
    pub(crate) fn build(
        manifest: &BackupManifest,
        payloads: Vec<ZipEntry>,
    ) -> Result<Vec<u8>, BackupArchiveError> {
        manifest.validate().map_err(BackupArchiveError::Manifest)?;
        verify_payloads(manifest, &payloads)?;
        let manifest_bytes = serde_json::to_vec(manifest)
            .map_err(|error| BackupArchiveError::Json(error.to_string()))?;
        let mut entries = Vec::with_capacity(payloads.len() + 1);
        entries.push(ZipEntry::new("manifest.json", manifest_bytes));
        entries.extend(payloads);
        write_stored_zip(&entries).map_err(BackupArchiveError::Zip)
    }

    pub(crate) fn inspect(
        bytes: &[u8],
        limits: ZipLimits,
    ) -> Result<VerifiedBackup, BackupArchiveError> {
        let entries = read_stored_zip(bytes, limits).map_err(BackupArchiveError::Zip)?;
        let manifest_entries = entries
            .iter()
            .filter(|entry| entry.path == "manifest.json")
            .collect::<Vec<_>>();
        if manifest_entries.len() != 1 {
            return Err(BackupArchiveError::MissingManifest);
        }
        let manifest: BackupManifest = serde_json::from_slice(&manifest_entries[0].bytes)
            .map_err(|error| BackupArchiveError::Json(error.to_string()))?;
        manifest.validate().map_err(BackupArchiveError::Manifest)?;
        let payloads = entries
            .into_iter()
            .filter(|entry| entry.path != "manifest.json")
            .collect::<Vec<_>>();
        verify_payloads(&manifest, &payloads)?;
        Ok(VerifiedBackup {
            manifest,
            files: payloads
                .into_iter()
                .map(|entry| (entry.path, entry.bytes))
                .collect(),
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VerifiedBackup {
    pub(crate) manifest: BackupManifest,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
}

impl VerifiedBackup {
    /// Extracts only into a caller-created, empty staging directory. `create_new` prevents a
    /// pre-existing file or symlink from being overwritten.
    pub(crate) fn extract_to(&self, staging: &Path) -> Result<(), BackupArchiveError> {
        if !staging.is_dir() {
            return Err(BackupArchiveError::Extraction(
                "restore staging directory does not exist".into(),
            ));
        }
        if fs::read_dir(staging)
            .map_err(|error| BackupArchiveError::Extraction(error.to_string()))?
            .next()
            .is_some()
        {
            return Err(BackupArchiveError::Extraction(
                "restore staging directory must be empty".into(),
            ));
        }
        for (relative, bytes) in &self.files {
            let target = joined_safe(staging, relative)?;
            let parent = target.parent().expect("archive entry has a parent");
            create_directories_without_symlinks(staging, parent)?;
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&target)
                .map_err(|error| BackupArchiveError::Extraction(error.to_string()))?;
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| BackupArchiveError::Extraction(error.to_string()))?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum BackupArchiveError {
    Zip(ZipError),
    Manifest(ManifestError),
    MissingManifest,
    MissingFile(String),
    UndeclaredFile(String),
    SizeMismatch(String),
    HashMismatch(String),
    Json(String),
    Extraction(String),
}

impl fmt::Display for BackupArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zip(error) => write!(formatter, "invalid backup ZIP: {error}"),
            Self::Manifest(error) => write!(formatter, "invalid backup manifest: {error}"),
            Self::MissingManifest => formatter.write_str("backup has no manifest.json"),
            Self::MissingFile(path) => write!(formatter, "backup is missing {path}"),
            Self::UndeclaredFile(path) => {
                write!(formatter, "backup contains undeclared file {path}")
            }
            Self::SizeMismatch(path) => write!(formatter, "backup size mismatch for {path}"),
            Self::HashMismatch(path) => write!(formatter, "backup SHA-256 mismatch for {path}"),
            Self::Json(error) => write!(formatter, "backup manifest JSON is invalid: {error}"),
            Self::Extraction(error) => write!(formatter, "backup extraction failed: {error}"),
        }
    }
}

fn verify_payloads(
    manifest: &BackupManifest,
    payloads: &[ZipEntry],
) -> Result<(), BackupArchiveError> {
    let expected = manifest
        .files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let actual_paths = payloads
        .iter()
        .map(|entry| entry.path.as_str())
        .collect::<BTreeSet<_>>();
    for path in expected.keys() {
        if !actual_paths.contains(path) {
            return Err(BackupArchiveError::MissingFile((*path).into()));
        }
    }
    for entry in payloads {
        let Some(file) = expected.get(entry.path.as_str()) else {
            return Err(BackupArchiveError::UndeclaredFile(entry.path.clone()));
        };
        if file.size != entry.bytes.len() as u64 {
            return Err(BackupArchiveError::SizeMismatch(entry.path.clone()));
        }
        if sha256(&entry.bytes).to_hex() != file.sha256 {
            return Err(BackupArchiveError::HashMismatch(entry.path.clone()));
        }
    }
    Ok(())
}

fn joined_safe(root: &Path, relative: &str) -> Result<PathBuf, BackupArchiveError> {
    crate::workspace_features::zip_store::validate_archive_path(relative)
        .map_err(BackupArchiveError::Zip)?;
    Ok(relative
        .split('/')
        .fold(root.to_path_buf(), |path, component| path.join(component)))
}

fn create_directories_without_symlinks(
    root: &Path,
    parent: &Path,
) -> Result<(), BackupArchiveError> {
    let relative = parent.strip_prefix(root).map_err(|_| {
        BackupArchiveError::Extraction("entry escaped restore staging directory".into())
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(BackupArchiveError::Extraction(
                    "restore staging contains a symbolic link".into(),
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(BackupArchiveError::Extraction(
                    "restore staging path collides with a file".into(),
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(&current)
                .map_err(|error| BackupArchiveError::Extraction(error.to_string()))?,
            Err(error) => return Err(BackupArchiveError::Extraction(error.to_string())),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_features::backup::manifest::{BackupFileRole, BackupKind, ManifestFile};

    fn fixture() -> (BackupManifest, Vec<ZipEntry>) {
        let files = vec![
            ZipEntry::new("workspace.sqlite3", b"sqlite".to_vec()),
            ZipEntry::new("workspace.v1.json", b"{}".to_vec()),
            ZipEntry::new("blobs/aa/hash", b"blob".to_vec()),
        ];
        let manifest = BackupManifest::new(
            "018f0000-0000-7000-8000-000000000001".into(),
            "1.0.0".into(),
            1,
            1,
            BackupKind::Manual,
            BTreeMap::new(),
            vec![
                ManifestFile::from_bytes(
                    "workspace.sqlite3".into(),
                    BackupFileRole::Database,
                    b"sqlite",
                ),
                ManifestFile::from_bytes(
                    "workspace.v1.json".into(),
                    BackupFileRole::WorkspaceMetadata,
                    b"{}",
                ),
                ManifestFile::from_bytes("blobs/aa/hash".into(), BackupFileRole::Blob, b"blob"),
            ],
        )
        .unwrap();
        (manifest, files)
    }

    #[test]
    fn builds_inspects_and_extracts_verified_backup() {
        let (manifest, files) = fixture();
        let archive = BackupArchive::build(&manifest, files).unwrap();
        let verified = BackupArchive::inspect(&archive, ZipLimits::default()).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        verified.extract_to(temporary.path()).unwrap();
        assert_eq!(
            fs::read(temporary.path().join("workspace.sqlite3")).unwrap(),
            b"sqlite"
        );
        assert_eq!(
            fs::read(temporary.path().join("blobs/aa/hash")).unwrap(),
            b"blob"
        );
    }

    #[test]
    fn detects_tampering_before_extraction() {
        let (manifest, mut files) = fixture();
        files[0].bytes.push(0);
        assert!(matches!(
            BackupArchive::build(&manifest, files),
            Err(BackupArchiveError::SizeMismatch(_))
        ));
    }
}
