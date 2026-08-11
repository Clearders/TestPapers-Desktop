use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use rusqlite::{backup::Backup, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    canonical::digest_hex,
    error::{LocalDataError, LocalDataResult},
    migration, LocalDataStore,
};

const ENTITY_TABLES: [(&str, &str); 7] = [
    ("questions", "questions"),
    ("papers", "papers"),
    ("drafts", "drafts"),
    ("attachments", "attachments"),
    ("comments", "comments"),
    ("favorites", "favorites"),
    ("settings", "settings"),
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupBlobSource {
    pub(crate) blob_hash: String,
    pub(crate) byte_size: u64,
    pub(crate) source_path: PathBuf,
    pub(crate) archive_relative_path: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BackupInventory {
    pub(crate) live_entity_counts: BTreeMap<String, u64>,
    pub(crate) blobs: Vec<BackupBlobSource>,
}

impl LocalDataStore {
    /// Creates a self-contained SQLite snapshot without copying a live WAL file. The destination
    /// must not exist and is removed if backup or validation fails.
    pub(crate) fn snapshot_to(&self, destination: &Path) -> LocalDataResult<BackupInventory> {
        validate_snapshot_destination(&self.database_path, destination)?;
        let parent = destination
            .parent()
            .ok_or_else(|| LocalDataError::UnsafePath(destination.to_owned()))?;
        fs::create_dir_all(parent)?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)?;

        let result = (|| {
            let source = Connection::open_with_flags(
                &self.database_path,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )?;
            source.busy_timeout(Duration::from_secs(5))?;
            let mut snapshot = Connection::open(destination)?;
            {
                let backup = Backup::new(&source, &mut snapshot)?;
                backup.run_to_completion(128, Duration::from_millis(5), None)?;
            }
            snapshot.execute_batch("PRAGMA foreign_keys = ON;")?;
            migration::validate_database(&snapshot)?;
            inventory_from(&snapshot, &self.blob_root)
        })();
        if result.is_err() {
            let _ = fs::remove_file(destination);
        }
        result
    }

    pub(crate) fn backup_inventory(&self) -> LocalDataResult<BackupInventory> {
        inventory_from(&self.connection(), &self.blob_root)
    }
}

fn inventory_from(connection: &Connection, blob_root: &Path) -> LocalDataResult<BackupInventory> {
    let mut live_entity_counts = BTreeMap::new();
    for (name, table) in ENTITY_TABLES {
        let count: i64 = connection.query_row(
            &format!("SELECT count(*) FROM {table} WHERE deleted_at IS NULL"),
            [],
            |row| row.get(0),
        )?;
        live_entity_counts.insert(
            name.into(),
            u64::try_from(count)
                .map_err(|_| LocalDataError::Corrupt(format!("negative {name} count")))?,
        );
    }

    let canonical_root = fs::canonicalize(blob_root)?;
    let mut statement = connection.prepare(
        "SELECT blob_hash, relative_path, byte_size
         FROM attachments
         GROUP BY blob_hash, relative_path, byte_size
         ORDER BY blob_hash, relative_path, byte_size",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut by_hash = BTreeMap::<String, (String, i64)>::new();
    for row in rows {
        let (hash, relative_path, byte_size) = row?;
        validate_hash(&hash)?;
        let expected_relative = format!("sha256/{}/{hash}", &hash[..2]);
        if relative_path != expected_relative {
            return Err(LocalDataError::Corrupt(format!(
                "attachment blob {hash} has an unsafe relative path"
            )));
        }
        match by_hash.get(&hash) {
            Some(existing) if existing != &(relative_path.clone(), byte_size) => {
                return Err(LocalDataError::Corrupt(format!(
                    "attachment blob {hash} has conflicting metadata"
                )))
            }
            Some(_) => {}
            None => {
                by_hash.insert(hash, (relative_path, byte_size));
            }
        }
    }

    let mut blobs = Vec::with_capacity(by_hash.len());
    for (hash, (relative_path, byte_size)) in by_hash {
        let byte_size = u64::try_from(byte_size)
            .map_err(|_| LocalDataError::Corrupt(format!("negative blob size for {hash}")))?;
        let source_path = blob_root.join(Path::new(&relative_path));
        let metadata = fs::symlink_metadata(&source_path)?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err(LocalDataError::UnsafePath(source_path));
        }
        let canonical_source = fs::canonicalize(&source_path)?;
        if !canonical_source.starts_with(&canonical_root) {
            return Err(LocalDataError::UnsafePath(canonical_source));
        }
        if metadata.len() != byte_size {
            return Err(LocalDataError::Corrupt(format!(
                "attachment blob {hash} size differs from metadata"
            )));
        }
        verify_hash(&canonical_source, &hash)?;
        blobs.push(BackupBlobSource {
            blob_hash: hash,
            byte_size,
            source_path: canonical_source,
            archive_relative_path: format!("blobs/{relative_path}"),
        });
    }
    Ok(BackupInventory {
        live_entity_counts,
        blobs,
    })
}

fn validate_snapshot_destination(database_path: &Path, destination: &Path) -> LocalDataResult<()> {
    if !destination.is_absolute() || destination == database_path || destination.exists() {
        return Err(LocalDataError::UnsafePath(destination.to_owned()));
    }
    Ok(())
}

fn validate_hash(value: &str) -> LocalDataResult<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(LocalDataError::Corrupt(
            "attachment has an invalid blob hash".into(),
        ))
    }
}

fn verify_hash(path: &Path, expected: &str) -> LocalDataResult<()> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    if digest_hex(&digest.finalize()) == expected {
        Ok(())
    } else {
        Err(LocalDataError::Corrupt(format!(
            "attachment blob {} failed SHA-256 verification",
            path.display()
        )))
    }
}
