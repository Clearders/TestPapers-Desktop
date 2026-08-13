use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::{backup::Backup, Connection, TransactionBehavior};
use uuid::Uuid;

use super::{
    error::{LocalDataError, LocalDataResult},
    StoreConfig,
};

pub(crate) const LATEST_SCHEMA_VERSION: u32 = 3;

const MIGRATIONS: &[(u32, &str)] = &[
    (1, include_str!("../../migrations/0001_local_data.sql")),
    (2, include_str!("../../migrations/0002_sync_state.sql")),
    (3, include_str!("../../migrations/0003_sync_delivery.sql")),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MigrationReport {
    pub(crate) from_version: u32,
    pub(crate) to_version: u32,
    pub(crate) rollback_path: Option<PathBuf>,
}

pub(super) fn open_migrated_database(
    config: &StoreConfig,
) -> LocalDataResult<(Connection, MigrationReport)> {
    validate_config(config)?;
    let parent = config
        .database_path
        .parent()
        .ok_or_else(|| LocalDataError::UnsafePath(config.database_path.clone()))?;
    fs::create_dir_all(parent)?;
    fs::create_dir_all(&config.blob_root)?;
    recover_interrupted_swap(&config.database_path)?;

    if !config.database_path.exists() {
        return create_new_database(config);
    }

    let source = Connection::open(&config.database_path)?;
    source.busy_timeout(Duration::from_secs(5))?;
    let from_version = schema_version(&source)?;
    if from_version > LATEST_SCHEMA_VERSION {
        return Err(LocalDataError::UnsupportedSchema {
            found: from_version,
            supported: LATEST_SCHEMA_VERSION,
        });
    }
    if from_version == LATEST_SCHEMA_VERSION {
        configure_runtime_connection(&source)?;
        validate_database(&source)?;
        validate_workspace(&source, config)?;
        return Ok((
            source,
            MigrationReport {
                from_version,
                to_version: from_version,
                rollback_path: None,
            },
        ));
    }

    migrate_existing_database(config, source, from_version)
}

fn recover_interrupted_swap(database_path: &Path) -> LocalDataResult<()> {
    if database_path.exists() {
        return Ok(());
    }
    let parent = database_path
        .parent()
        .ok_or_else(|| LocalDataError::UnsafePath(database_path.to_owned()))?;
    let file_name = database_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| LocalDataError::UnsafePath(database_path.to_owned()))?;
    let rollback_prefix = format!("{file_name}.rollback-v");
    let mut candidates = Vec::new();
    for entry in fs::read_dir(parent)? {
        let entry = entry?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| name.starts_with(&rollback_prefix))
        {
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            candidates.push((modified, entry.path()));
        }
    }
    if let Some((_, rollback)) = candidates.into_iter().max_by_key(|(modified, _)| *modified) {
        fs::rename(rollback, database_path)?;
    }
    Ok(())
}

fn create_new_database(config: &StoreConfig) -> LocalDataResult<(Connection, MigrationReport)> {
    let staging_path = sibling_path(&config.database_path, "migrating");
    let create_result = (|| {
        let mut staging = Connection::open(&staging_path)?;
        apply_migrations(&mut staging, 0)?;
        initialize_workspace(&staging, config)?;
        validate_database(&staging)?;
        drop(staging);
        fs::rename(&staging_path, &config.database_path)?;
        open_checked(&config.database_path, config)
    })();

    match create_result {
        Ok(connection) => Ok((
            connection,
            MigrationReport {
                from_version: 0,
                to_version: LATEST_SCHEMA_VERSION,
                rollback_path: None,
            },
        )),
        Err(error) => {
            let _ = fs::remove_file(&staging_path);
            Err(error)
        }
    }
}

fn migrate_existing_database(
    config: &StoreConfig,
    source: Connection,
    from_version: u32,
) -> LocalDataResult<(Connection, MigrationReport)> {
    let staging_path = sibling_path(&config.database_path, "migrating");
    let rollback_path = sibling_path(&config.database_path, &format!("rollback-v{from_version}"));

    let migration_result = (|| {
        let mut staging = Connection::open(&staging_path)?;
        {
            let backup = Backup::new(&source, &mut staging)?;
            backup.run_to_completion(128, Duration::from_millis(5), None)?;
        }
        apply_migrations(&mut staging, from_version)?;
        initialize_workspace(&staging, config)?;
        validate_database(&staging)?;

        // Make the original main file self-contained before it becomes the rollback copy.
        let checkpoint_busy: i64 =
            source.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
        if checkpoint_busy != 0 {
            return Err(LocalDataError::Busy(
                "the original database could not be checkpointed before migration".into(),
            ));
        }
        drop(staging);
        drop(source);

        fs::rename(&config.database_path, &rollback_path)?;
        if let Err(swap_error) = fs::rename(&staging_path, &config.database_path) {
            let _ = fs::rename(&rollback_path, &config.database_path);
            return Err(LocalDataError::Io(swap_error));
        }

        match open_checked(&config.database_path, config) {
            Ok(connection) => Ok(connection),
            Err(error) => {
                let failed_path = sibling_path(&config.database_path, "failed-migration");
                let _ = fs::rename(&config.database_path, failed_path);
                let _ = fs::rename(&rollback_path, &config.database_path);
                Err(error)
            }
        }
    })();

    match migration_result {
        Ok(connection) => Ok((
            connection,
            MigrationReport {
                from_version,
                to_version: LATEST_SCHEMA_VERSION,
                rollback_path: Some(rollback_path),
            },
        )),
        Err(error) => {
            let _ = fs::remove_file(&staging_path);
            Err(error)
        }
    }
}

fn apply_migrations(connection: &mut Connection, from_version: u32) -> LocalDataResult<()> {
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    for (version, sql) in MIGRATIONS {
        if *version <= from_version {
            continue;
        }
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", version)?;
        if *version > 1 {
            transaction.execute(
                "UPDATE workspace_meta SET schema_version = ?1 WHERE singleton = 1",
                [version],
            )?;
        }
        transaction.commit()?;
    }
    Ok(())
}

fn initialize_workspace(connection: &Connection, config: &StoreConfig) -> LocalDataResult<()> {
    connection.execute(
        "INSERT OR IGNORE INTO workspace_meta(
            singleton, workspace_id, local_principal_id, schema_version, created_at
         ) VALUES (1, ?1, ?2, ?3, ?4)",
        (
            &config.workspace_id,
            &config.local_principal_id,
            LATEST_SCHEMA_VERSION,
            now_micros(),
        ),
    )?;
    validate_workspace(connection, config)
}

fn open_checked(path: &Path, config: &StoreConfig) -> LocalDataResult<Connection> {
    let connection = Connection::open(path)?;
    configure_runtime_connection(&connection)?;
    validate_database(&connection)?;
    validate_workspace(&connection, config)?;
    Ok(connection)
}

pub(super) fn configure_runtime_connection(connection: &Connection) -> LocalDataResult<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys = ON;
         PRAGMA journal_mode = WAL;
         PRAGMA synchronous = FULL;",
    )?;
    Ok(())
}

pub(super) fn validate_database(connection: &Connection) -> LocalDataResult<()> {
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if integrity != "ok" {
        return Err(LocalDataError::Corrupt(format!(
            "SQLite integrity_check returned {integrity}"
        )));
    }
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    if statement.query([])?.next()?.is_some() {
        return Err(LocalDataError::Corrupt(
            "SQLite foreign_key_check found a violation".into(),
        ));
    }
    Ok(())
}

fn validate_workspace(connection: &Connection, config: &StoreConfig) -> LocalDataResult<()> {
    let (workspace_id, principal_id, schema): (String, String, u32) = connection.query_row(
        "SELECT workspace_id, local_principal_id, schema_version
         FROM workspace_meta WHERE singleton = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    if workspace_id != config.workspace_id {
        return Err(LocalDataError::WorkspaceMismatch {
            field: "workspaceId",
        });
    }
    if principal_id != config.local_principal_id {
        return Err(LocalDataError::WorkspaceMismatch {
            field: "localPrincipalId",
        });
    }
    if schema != LATEST_SCHEMA_VERSION {
        return Err(LocalDataError::Corrupt(format!(
            "workspace metadata schema {schema} differs from SQLite schema {}",
            LATEST_SCHEMA_VERSION
        )));
    }
    Ok(())
}

fn validate_config(config: &StoreConfig) -> LocalDataResult<()> {
    if !config.database_path.is_absolute() || !config.blob_root.is_absolute() {
        return Err(LocalDataError::UnsafePath(config.database_path.clone()));
    }
    validate_canonical_uuid(&config.workspace_id, "workspaceId")?;
    validate_canonical_uuid(&config.local_principal_id, "localPrincipalId")?;
    Ok(())
}

pub(super) fn validate_canonical_uuid(value: &str, field: &'static str) -> LocalDataResult<()> {
    match Uuid::parse_str(value) {
        Ok(uuid) if uuid.to_string() == value => Ok(()),
        _ => Err(LocalDataError::Validation(vec![format!(
            "{field} must be a canonical lowercase UUID"
        )])),
    }
}

fn schema_version(connection: &Connection) -> LocalDataResult<u32> {
    Ok(connection.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

fn sibling_path(database_path: &Path, label: &str) -> PathBuf {
    let file_name = database_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace.sqlite3");
    database_path.with_file_name(format!(
        "{file_name}.{label}-{}-{}",
        now_micros(),
        Uuid::now_v7()
    ))
}

pub(super) fn now_micros() -> i64 {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(duration.as_micros()).unwrap_or(i64::MAX)
}
