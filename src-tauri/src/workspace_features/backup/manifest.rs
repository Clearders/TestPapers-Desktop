use crate::workspace_features::hash::{sha256, Sha256Digest};
use crate::workspace_features::zip_store::validate_archive_path;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub(crate) const BACKUP_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupKind {
    Manual,
    Automatic,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BackupFileRole {
    Database,
    WorkspaceMetadata,
    Blob,
    Template,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestFile {
    pub(crate) path: String,
    pub(crate) size: u64,
    pub(crate) sha256: String,
    pub(crate) role: BackupFileRole,
}

impl ManifestFile {
    pub(crate) fn from_bytes(path: String, role: BackupFileRole, bytes: &[u8]) -> Self {
        Self {
            path,
            size: bytes.len() as u64,
            sha256: sha256(bytes).to_hex(),
            role,
        }
    }

    pub(crate) fn digest(&self) -> Result<Sha256Digest, ManifestError> {
        Sha256Digest::from_hex(&self.sha256)
            .map_err(|_| ManifestError::InvalidDigest(self.path.clone()))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupManifest {
    pub(crate) format_version: u32,
    pub(crate) workspace_id: String,
    pub(crate) app_version: String,
    pub(crate) schema_version: u32,
    pub(crate) created_at_micros: i64,
    pub(crate) kind: BackupKind,
    pub(crate) entity_counts: BTreeMap<String, u64>,
    pub(crate) files: Vec<ManifestFile>,
}

impl BackupManifest {
    pub(crate) fn new(
        workspace_id: String,
        app_version: String,
        schema_version: u32,
        created_at_micros: i64,
        kind: BackupKind,
        entity_counts: BTreeMap<String, u64>,
        mut files: Vec<ManifestFile>,
    ) -> Result<Self, ManifestError> {
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let manifest = Self {
            format_version: BACKUP_FORMAT_VERSION,
            workspace_id,
            app_version,
            schema_version,
            created_at_micros,
            kind,
            entity_counts,
            files,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub(crate) fn validate(&self) -> Result<(), ManifestError> {
        if self.format_version != BACKUP_FORMAT_VERSION {
            return Err(ManifestError::UnsupportedFormat(self.format_version));
        }
        if !is_canonical_uuid(&self.workspace_id) {
            return Err(ManifestError::InvalidWorkspaceId);
        }
        if self.app_version.trim().is_empty()
            || self.schema_version == 0
            || self.created_at_micros <= 0
        {
            return Err(ManifestError::InvalidMetadata);
        }
        let mut paths = BTreeSet::new();
        let mut database_count = 0;
        let mut workspace_metadata_count = 0;
        for file in &self.files {
            validate_archive_path(&file.path)
                .map_err(|_| ManifestError::UnsafePath(file.path.clone()))?;
            if file.path == "manifest.json" {
                return Err(ManifestError::UnsafePath(file.path.clone()));
            }
            if !paths.insert(&file.path) {
                return Err(ManifestError::DuplicatePath(file.path.clone()));
            }
            file.digest()?;
            if file.role == BackupFileRole::Database {
                database_count += 1;
                if file.path != "workspace.sqlite3" {
                    return Err(ManifestError::InvalidDatabasePath);
                }
            }
            if file.role == BackupFileRole::WorkspaceMetadata {
                workspace_metadata_count += 1;
                if file.path != "workspace.v1.json" {
                    return Err(ManifestError::InvalidWorkspaceMetadataPath);
                }
            }
        }
        if database_count != 1 {
            return Err(ManifestError::MissingDatabase);
        }
        if workspace_metadata_count != 1 {
            return Err(ManifestError::MissingWorkspaceMetadata);
        }
        if self
            .files
            .windows(2)
            .any(|pair| pair[0].path >= pair[1].path)
        {
            return Err(ManifestError::UnsortedFiles);
        }
        Ok(())
    }

    pub(crate) fn database(&self) -> &ManifestFile {
        self.files
            .iter()
            .find(|file| file.role == BackupFileRole::Database)
            .expect("validated manifest has one database")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ManifestError {
    UnsupportedFormat(u32),
    InvalidWorkspaceId,
    InvalidMetadata,
    UnsafePath(String),
    DuplicatePath(String),
    InvalidDigest(String),
    MissingDatabase,
    InvalidDatabasePath,
    MissingWorkspaceMetadata,
    InvalidWorkspaceMetadataPath,
    UnsortedFiles,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat(version) => {
                write!(formatter, "unsupported backup format {version}")
            }
            Self::InvalidWorkspaceId => formatter.write_str("backup workspace ID is invalid"),
            Self::InvalidMetadata => formatter.write_str("backup metadata is invalid"),
            Self::UnsafePath(path) => write!(formatter, "unsafe backup path {path}"),
            Self::DuplicatePath(path) => write!(formatter, "duplicate backup path {path}"),
            Self::InvalidDigest(path) => write!(formatter, "invalid SHA-256 for {path}"),
            Self::MissingDatabase => {
                formatter.write_str("backup must contain exactly one database")
            }
            Self::InvalidDatabasePath => formatter.write_str("database must be workspace.sqlite3"),
            Self::MissingWorkspaceMetadata => {
                formatter.write_str("backup must contain exactly one workspace metadata file")
            }
            Self::InvalidWorkspaceMetadataPath => {
                formatter.write_str("workspace metadata must be workspace.v1.json")
            }
            Self::UnsortedFiles => formatter.write_str("manifest files must be sorted by path"),
        }
    }
}

fn is_canonical_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(byte),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_sorts_files_and_requires_exact_database_role() {
        let manifest = BackupManifest::new(
            "018f0000-0000-7000-8000-000000000001".into(),
            "1.2.3".into(),
            3,
            1,
            BackupKind::Manual,
            BTreeMap::new(),
            vec![
                ManifestFile::from_bytes("templates/a.docx".into(), BackupFileRole::Template, b"a"),
                ManifestFile::from_bytes(
                    "workspace.sqlite3".into(),
                    BackupFileRole::Database,
                    b"db",
                ),
                ManifestFile::from_bytes(
                    "workspace.v1.json".into(),
                    BackupFileRole::WorkspaceMetadata,
                    b"{}",
                ),
            ],
        )
        .unwrap();
        assert_eq!(manifest.files[0].path, "templates/a.docx");
        assert_eq!(manifest.files[1].path, "workspace.sqlite3");
        assert_eq!(manifest.files[2].path, "workspace.v1.json");
        assert_eq!(
            serde_json::to_value(&manifest).unwrap()["files"][2]["role"],
            "workspace_metadata"
        );

        let error = BackupManifest::new(
            "018f0000-0000-7000-8000-000000000001".into(),
            "1".into(),
            1,
            1,
            BackupKind::Manual,
            BTreeMap::new(),
            vec![
                ManifestFile::from_bytes("db.sqlite".into(), BackupFileRole::Database, b"db"),
                ManifestFile::from_bytes(
                    "workspace.v1.json".into(),
                    BackupFileRole::WorkspaceMetadata,
                    b"{}",
                ),
            ],
        )
        .unwrap_err();
        assert_eq!(error, ManifestError::InvalidDatabasePath);
    }
}
