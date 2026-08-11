use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use uuid::{Uuid, Version};

use crate::domain::{EngineFailure, SuggestedAction, ENGINE_SCHEMA_VERSION};

const IDENTITY_FILE_NAME: &str = "workspace.v1.json";
const LOCK_FILE_NAME: &str = ".testpapers.workspace.lock";
const POINTER_FILE_NAME: &str = "workspace-pointer.v1.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceIdentity {
    pub(crate) workspace_id: Uuid,
    pub(crate) local_principal_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredWorkspaceIdentity {
    schema_version: u8,
    workspace_id: Uuid,
    local_principal_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredWorkspacePointer {
    schema_version: u8,
    root: PathBuf,
}

impl StoredWorkspaceIdentity {
    fn create() -> Self {
        Self {
            schema_version: ENGINE_SCHEMA_VERSION,
            workspace_id: Uuid::now_v7(),
            local_principal_id: Uuid::now_v7(),
        }
    }

    fn validate(self) -> Result<WorkspaceIdentity, EngineFailure> {
        if self.schema_version != ENGINE_SCHEMA_VERSION
            || self.workspace_id.get_version() != Some(Version::SortRand)
            || self.local_principal_id.get_version() != Some(Version::SortRand)
        {
            return Err(invalid_identity());
        }
        Ok(WorkspaceIdentity {
            workspace_id: self.workspace_id,
            local_principal_id: self.local_principal_id,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WorkspaceBootstrap {
    root: Arc<RwLock<Result<PathBuf, EngineFailure>>>,
}

impl WorkspaceBootstrap {
    #[cfg(test)]
    pub(crate) fn at(root: PathBuf) -> Self {
        Self {
            root: Arc::new(RwLock::new(Ok(root))),
        }
    }

    pub(crate) fn unavailable(message: impl Into<String>) -> Self {
        Self {
            root: Arc::new(RwLock::new(Err(EngineFailure::new(
                "workspace_unavailable",
                message,
                true,
                SuggestedAction::ChooseDirectory,
            )))),
        }
    }

    pub(crate) fn configured(app_data_directory: PathBuf) -> (Self, PathBuf) {
        let pointer_path = app_data_directory.join(POINTER_FILE_NAME);
        let default_root = app_data_directory.join("workspace");
        let root = match fs::read(&pointer_path) {
            Ok(bytes) => serde_json::from_slice::<StoredWorkspacePointer>(&bytes)
                .ok()
                .filter(|pointer| pointer.schema_version == ENGINE_SCHEMA_VERSION)
                .map(|pointer| pointer.root)
                .filter(|root| valid_workspace_root(root))
                .ok_or_else(|| {
                    EngineFailure::new(
                        "workspace_pointer_invalid",
                        "The configured workspace directory is invalid or unreadable",
                        true,
                        SuggestedAction::ChooseDirectory,
                    )
                }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(default_root),
            Err(_) => Err(EngineFailure::new(
                "workspace_pointer_unavailable",
                "The configured workspace directory could not be read",
                true,
                SuggestedAction::ChooseDirectory,
            )),
        };
        (
            Self {
                root: Arc::new(RwLock::new(root)),
            },
            pointer_path,
        )
    }

    pub(crate) fn persist_root_pointer(
        &self,
        pointer_path: &Path,
        root: &Path,
    ) -> Result<(), EngineFailure> {
        if !valid_workspace_root(root) {
            return Err(workspace_unavailable());
        }
        let parent = pointer_path.parent().ok_or_else(workspace_unavailable)?;
        fs::create_dir_all(parent).map_err(|_| workspace_unavailable())?;
        let temporary = parent.join(format!(
            ".workspace-pointer.v1.{}.tmp",
            Uuid::now_v7().as_simple()
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)
                .map_err(|_| workspace_unavailable())?;
            let bytes = serde_json::to_vec_pretty(&StoredWorkspacePointer {
                schema_version: ENGINE_SCHEMA_VERSION,
                root: root.to_path_buf(),
            })
            .map_err(|_| workspace_unavailable())?;
            file.write_all(&bytes)
                .map_err(|_| workspace_unavailable())?;
            file.sync_all().map_err(|_| workspace_unavailable())?;
            drop(file);
            if pointer_path.exists() {
                fs::remove_file(pointer_path).map_err(|_| workspace_unavailable())?;
            }
            fs::rename(&temporary, pointer_path).map_err(|_| workspace_unavailable())?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        } else {
            self.replace_root(root.to_path_buf());
        }
        result
    }

    /// Changes the root used by the next supervised open. The caller is responsible for
    /// persisting the bootstrap pointer atomically before invoking this method.
    pub(crate) fn replace_root(&self, root: PathBuf) {
        *self
            .root
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Ok(root);
    }

    pub(crate) fn current_root(&self) -> Result<PathBuf, EngineFailure> {
        self.root
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub(crate) fn current_identity(&self) -> Result<WorkspaceIdentity, EngineFailure> {
        let root = self.current_root()?;
        load_or_create_identity(&root)
    }

    pub(crate) fn open(&self) -> Result<WorkspaceLease, EngineFailure> {
        let root = self.current_root()?;
        fs::create_dir_all(&root).map_err(|_| workspace_unavailable())?;

        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(root.join(LOCK_FILE_NAME))
            .map_err(|_| workspace_unavailable())?;
        lock.try_lock().map_err(|_| {
            EngineFailure::new(
                "workspace_locked",
                "The workspace is already in use by another TestPapers Desktop process",
                true,
                SuggestedAction::RestartApp,
            )
        })?;

        let identity = load_or_create_identity(&root)?;
        Ok(WorkspaceLease {
            root,
            identity,
            _lock: lock,
        })
    }
}

fn valid_workspace_root(root: &Path) -> bool {
    root.is_absolute() && root.file_name().is_some()
}

#[derive(Debug)]
pub(crate) struct WorkspaceLease {
    root: PathBuf,
    identity: WorkspaceIdentity,
    _lock: File,
}

impl WorkspaceLease {
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn identity(&self) -> &WorkspaceIdentity {
        &self.identity
    }
}

fn load_or_create_identity(root: &Path) -> Result<WorkspaceIdentity, EngineFailure> {
    let path = root.join(IDENTITY_FILE_NAME);
    match OpenOptions::new().read(true).open(&path) {
        Ok(mut file) => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|_| invalid_identity())?;
            serde_json::from_slice::<StoredWorkspaceIdentity>(&contents)
                .map_err(|_| invalid_identity())?
                .validate()
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let stored = StoredWorkspaceIdentity::create();
            persist_identity(root, &path, &stored)?;
            stored.validate()
        }
        Err(_) => Err(workspace_unavailable()),
    }
}

fn persist_identity(
    root: &Path,
    target: &Path,
    identity: &StoredWorkspaceIdentity,
) -> Result<(), EngineFailure> {
    let temporary = root.join(format!(".workspace.v1.{}.tmp", Uuid::now_v7().as_simple()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| workspace_unavailable())?;
        let bytes = serde_json::to_vec_pretty(identity).map_err(|_| invalid_identity())?;
        file.write_all(&bytes)
            .map_err(|_| workspace_unavailable())?;
        file.sync_all().map_err(|_| workspace_unavailable())?;
        drop(file);
        fs::rename(&temporary, target).map_err(|_| workspace_unavailable())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn workspace_unavailable() -> EngineFailure {
    EngineFailure::new(
        "workspace_unavailable",
        "The Local Engine cannot access its workspace directory",
        true,
        SuggestedAction::ChooseDirectory,
    )
}

fn invalid_identity() -> EngineFailure {
    EngineFailure::new(
        "workspace_identity_invalid",
        "The workspace identity is invalid or unreadable",
        false,
        SuggestedAction::ContactSupport,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_excludes_a_second_process_and_preserves_identity() {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap = WorkspaceBootstrap::at(directory.path().join("workspace"));

        let first = bootstrap.open().unwrap();
        let first_identity = first.identity().clone();
        let error = bootstrap.open().unwrap_err();
        assert_eq!(error.code, "workspace_locked");
        assert!(!error
            .message
            .contains(directory.path().to_string_lossy().as_ref()));

        drop(first);
        let reopened = bootstrap.open().unwrap();
        assert_eq!(reopened.identity(), &first_identity);
        assert_eq!(reopened.root(), directory.path().join("workspace"));
    }

    #[test]
    fn invalid_identity_is_rejected_without_leaking_a_path() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(IDENTITY_FILE_NAME), b"not-json").unwrap();

        let error = WorkspaceBootstrap::at(root).open().unwrap_err();
        assert_eq!(error.code, "workspace_identity_invalid");
        assert!(!error
            .message
            .contains(directory.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn stored_identity_is_versioned_and_camel_case() {
        let stored = StoredWorkspaceIdentity::create();
        let value = serde_json::to_value(stored).unwrap();
        assert_eq!(value["schemaVersion"], ENGINE_SCHEMA_VERSION);
        assert!(value["workspaceId"].is_string());
        assert!(value["localPrincipalId"].is_string());
        assert!(value.get("workspace_id").is_none());
    }

    #[test]
    fn clones_observe_an_activated_workspace_root() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let second = directory.path().join("second");
        let bootstrap = WorkspaceBootstrap::at(first);
        let worker_view = bootstrap.clone();

        bootstrap.replace_root(second.clone());

        assert_eq!(worker_view.current_root().unwrap(), second);
    }

    #[test]
    fn configured_root_pointer_is_versioned_and_reloaded() {
        let directory = tempfile::tempdir().unwrap();
        let app_data = directory.path().join("app-data");
        let destination = directory.path().join("moved-workspace");
        let (bootstrap, pointer_path) = WorkspaceBootstrap::configured(app_data.clone());
        assert_eq!(
            bootstrap.current_root().unwrap(),
            app_data.join("workspace")
        );

        bootstrap
            .persist_root_pointer(&pointer_path, &destination)
            .unwrap();
        let (reloaded, _) = WorkspaceBootstrap::configured(app_data);

        assert_eq!(reloaded.current_root().unwrap(), destination);
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(pointer_path).unwrap()).unwrap();
        assert_eq!(value["schemaVersion"], ENGINE_SCHEMA_VERSION);
        assert!(value["root"].is_string());
    }
}
