use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    infrastructure::workspace::WorkspaceBootstrap,
    local_data::ImportInspection,
    workspace_features::{
        backup::{AutomaticBackupState, BackupScheduleConfig, PreparedRestore, WorkspacePointer},
        jobs::{JobCoordinator, JobEventSink},
    },
};

const LOCAL_WORKSPACE_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Debug)]
pub(crate) struct DirectoryGrant {
    pub(crate) id: String,
    pub(crate) path: PathBuf,
    pub(crate) display_name: String,
    pub(crate) writable: bool,
    pub(crate) available_bytes: Option<u64>,
}

#[derive(Clone, Debug)]
pub(crate) struct RestoreGrant {
    pub(crate) id: String,
    pub(crate) prepared: PreparedRestore,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceRootActivator {
    bootstrap: WorkspaceBootstrap,
    pointer_path: PathBuf,
}

impl WorkspacePointer for WorkspaceRootActivator {
    fn activate(&self, destination: &Path) -> Result<(), String> {
        self.bootstrap
            .persist_root_pointer(&self.pointer_path, destination)
            .map_err(|error| error.message)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedBackupRuntime {
    schema_version: u8,
    config: BackupScheduleConfig,
    destination_path: Option<PathBuf>,
    destination_display_name: Option<String>,
    automatic_state: PersistedAutomaticBackupState,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedAutomaticBackupState {
    last_success_micros: Option<i64>,
    last_attempt_micros: Option<i64>,
    consecutive_failures: u32,
}

impl From<&AutomaticBackupState> for PersistedAutomaticBackupState {
    fn from(value: &AutomaticBackupState) -> Self {
        Self {
            last_success_micros: value.last_success_micros,
            last_attempt_micros: value.last_attempt_micros,
            consecutive_failures: value.consecutive_failures,
        }
    }
}

impl From<PersistedAutomaticBackupState> for AutomaticBackupState {
    fn from(value: PersistedAutomaticBackupState) -> Self {
        Self {
            last_success_micros: value.last_success_micros,
            last_attempt_micros: value.last_attempt_micros,
            consecutive_failures: value.consecutive_failures,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct BackupRuntime {
    pub(crate) config: BackupScheduleConfig,
    pub(crate) destination_path: Option<PathBuf>,
    pub(crate) destination_display_name: Option<String>,
    pub(crate) automatic_state: AutomaticBackupState,
}

impl From<PersistedBackupRuntime> for BackupRuntime {
    fn from(value: PersistedBackupRuntime) -> Self {
        Self {
            config: value.config,
            destination_path: value.destination_path,
            destination_display_name: value.destination_display_name,
            automatic_state: value.automatic_state.into(),
        }
    }
}

pub(crate) struct LocalWorkspaceApplication {
    jobs: JobCoordinator,
    imports: Mutex<BTreeMap<String, ImportInspection>>,
    directories: Mutex<BTreeMap<String, DirectoryGrant>>,
    restores: Mutex<BTreeMap<String, RestoreGrant>>,
    backup: Mutex<BackupRuntime>,
    backup_settings_path: Option<PathBuf>,
    workspace_pointer_path: Option<PathBuf>,
    workspace_bootstrap: WorkspaceBootstrap,
}

impl LocalWorkspaceApplication {
    pub(crate) fn new(
        event_sink: Arc<dyn JobEventSink>,
        backup_settings_path: Option<PathBuf>,
        workspace_pointer_path: Option<PathBuf>,
        workspace_bootstrap: WorkspaceBootstrap,
    ) -> Self {
        let backup = backup_settings_path
            .as_deref()
            .and_then(load_backup_runtime)
            .unwrap_or_default();
        Self {
            jobs: JobCoordinator::new(Some(event_sink)),
            imports: Mutex::new(BTreeMap::new()),
            directories: Mutex::new(BTreeMap::new()),
            restores: Mutex::new(BTreeMap::new()),
            backup: Mutex::new(backup),
            backup_settings_path,
            workspace_pointer_path,
            workspace_bootstrap,
        }
    }

    pub(crate) fn jobs(&self) -> &JobCoordinator {
        &self.jobs
    }

    pub(crate) fn register_import(&self, inspection: ImportInspection) -> String {
        let id = Uuid::now_v7().to_string();
        lock(&self.imports).insert(id.clone(), inspection);
        id
    }

    pub(crate) fn take_import(&self, id: &str) -> Option<ImportInspection> {
        lock(&self.imports).remove(id)
    }

    pub(crate) fn restore_import(&self, id: String, inspection: ImportInspection) {
        lock(&self.imports).insert(id, inspection);
    }

    pub(crate) fn discard_import(&self, id: &str) -> bool {
        lock(&self.imports).remove(id).is_some()
    }

    pub(crate) fn register_directory(
        &self,
        path: PathBuf,
        writable: bool,
        available_bytes: Option<u64>,
    ) -> DirectoryGrant {
        let display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Selected folder")
            .to_owned();
        let grant = DirectoryGrant {
            id: Uuid::now_v7().to_string(),
            path,
            display_name,
            writable,
            available_bytes,
        };
        lock(&self.directories).insert(grant.id.clone(), grant.clone());
        grant
    }

    pub(crate) fn directory(&self, id: &str) -> Option<DirectoryGrant> {
        lock(&self.directories).get(id).cloned()
    }

    pub(crate) fn register_restore(&self, grant: RestoreGrant) {
        lock(&self.restores).insert(grant.id.clone(), grant);
    }

    pub(crate) fn take_restore(&self, id: &str) -> Option<RestoreGrant> {
        lock(&self.restores).remove(id)
    }

    pub(crate) fn discard_restore(&self, id: &str) -> Option<RestoreGrant> {
        lock(&self.restores).remove(id)
    }

    pub(crate) fn backup(&self) -> BackupRuntime {
        lock(&self.backup).clone()
    }

    pub(crate) fn configure_backup(
        &self,
        config: BackupScheduleConfig,
        destination: Option<DirectoryGrant>,
    ) -> Result<BackupRuntime, String> {
        config.validate().map_err(|error| error.to_string())?;
        let snapshot = {
            let mut runtime = lock(&self.backup);
            runtime.config = config;
            if let Some(destination) = destination {
                runtime.destination_path = Some(destination.path);
                runtime.destination_display_name = Some(destination.display_name);
            }
            runtime.clone()
        };
        self.persist_backup(&snapshot)?;
        Ok(snapshot)
    }

    pub(crate) fn record_backup_attempt(&self, at_micros: i64, success: bool) {
        let snapshot = {
            let mut runtime = lock(&self.backup);
            runtime.automatic_state.last_attempt_micros = Some(at_micros);
            if success {
                runtime.automatic_state.last_success_micros = Some(at_micros);
                runtime.automatic_state.consecutive_failures = 0;
            } else {
                runtime.automatic_state.consecutive_failures = runtime
                    .automatic_state
                    .consecutive_failures
                    .saturating_add(1);
            }
            runtime.clone()
        };
        let _ = self.persist_backup(&snapshot);
    }

    pub(crate) fn workspace_root_activator(&self) -> Option<WorkspaceRootActivator> {
        self.workspace_pointer_path
            .clone()
            .map(|pointer_path| WorkspaceRootActivator {
                bootstrap: self.workspace_bootstrap.clone(),
                pointer_path,
            })
    }

    pub(crate) fn workspace_root(&self) -> Result<PathBuf, crate::domain::EngineFailure> {
        self.workspace_bootstrap.current_root()
    }

    pub(crate) fn workspace_id(&self) -> Result<String, crate::domain::EngineFailure> {
        self.workspace_bootstrap
            .current_identity()
            .map(|identity| identity.workspace_id.to_string())
    }

    fn persist_backup(&self, runtime: &BackupRuntime) -> Result<(), String> {
        let Some(path) = &self.backup_settings_path else {
            return Err("Backup settings cannot be persisted in this session.".into());
        };
        persist_backup_runtime(path, runtime)
    }
}

fn load_backup_runtime(path: &Path) -> Option<BackupRuntime> {
    let bytes = fs::read(path).ok()?;
    let persisted = serde_json::from_slice::<PersistedBackupRuntime>(&bytes).ok()?;
    (persisted.schema_version == LOCAL_WORKSPACE_SCHEMA_VERSION).then(|| persisted.into())
}

fn persist_backup_runtime(path: &Path, runtime: &BackupRuntime) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Backup settings path has no parent.".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(
        ".backup-schedule.v1.{}.tmp",
        Uuid::now_v7().as_simple()
    ));
    let persisted = PersistedBackupRuntime {
        schema_version: LOCAL_WORKSPACE_SCHEMA_VERSION,
        config: runtime.config.clone(),
        destination_path: runtime.destination_path.clone(),
        destination_display_name: runtime.destination_display_name.clone(),
        automatic_state: (&runtime.automatic_state).into(),
    };
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        let bytes = serde_json::to_vec_pretty(&persisted).map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        if path.exists() {
            fs::remove_file(path).map_err(|error| error.to_string())?;
        }
        fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoEvents;
    impl JobEventSink for NoEvents {
        fn updated(&self, _: crate::workspace_features::jobs::JobSnapshot) {}
    }

    #[test]
    fn import_and_directory_grants_are_opaque_and_one_use() {
        let directory = tempfile::tempdir().unwrap();
        let application = LocalWorkspaceApplication::new(
            Arc::new(NoEvents),
            Some(directory.path().join("backup.json")),
            Some(directory.path().join("pointer.json")),
            WorkspaceBootstrap::at(directory.path().join("workspace")),
        );
        let import_id = application.register_import(ImportInspection::default());
        assert!(Uuid::parse_str(&import_id).is_ok());
        assert!(application.take_import(&import_id).is_some());
        assert!(application.take_import(&import_id).is_none());

        let grant = application.register_directory(directory.path().to_path_buf(), true, None);
        assert_ne!(grant.id, grant.path.to_string_lossy());
        assert_eq!(application.directory(&grant.id).unwrap().path, grant.path);
    }

    #[test]
    fn backup_configuration_is_persisted_without_exposing_it_to_the_webview() {
        let directory = tempfile::tempdir().unwrap();
        let settings = directory.path().join("backup.json");
        let application = LocalWorkspaceApplication::new(
            Arc::new(NoEvents),
            Some(settings.clone()),
            Some(directory.path().join("pointer.json")),
            WorkspaceBootstrap::at(directory.path().join("workspace")),
        );
        let destination =
            application.register_directory(directory.path().join("backups"), true, Some(1_000));
        let config = BackupScheduleConfig {
            enabled: true,
            destination_id: Some(destination.id.clone()),
            interval_minutes: 120,
            retention_days: 7,
            encrypted: false,
            key_id: None,
            recovery_key_confirmed: false,
        };
        application
            .configure_backup(config, Some(destination))
            .unwrap();

        let reloaded = LocalWorkspaceApplication::new(
            Arc::new(NoEvents),
            Some(settings),
            Some(directory.path().join("pointer.json")),
            WorkspaceBootstrap::at(directory.path().join("workspace")),
        );
        assert!(reloaded.backup().config.enabled);
        assert_eq!(
            reloaded.backup().destination_display_name.as_deref(),
            Some("backups")
        );
    }
}
