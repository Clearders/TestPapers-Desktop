use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use tauri::{AppHandle, Manager, State};
use uuid::{Uuid, Version};

use crate::{
    application::{EngineSupervisor, LocalWorkspaceApplication, WorkspaceRuntime},
    domain::{EngineFailure, EngineState, SuggestedAction},
    infrastructure::{
        backup_crypto::{AuditedAgeBackend, KeyringAgeIdentityProvider},
        dialogs,
    },
    local_data::{
        BackupInventory, CreateQuestion, DeletedFilter, LocalDataError, LocalDataStore,
        NewQuestionAttachment, QuestionRecord, QuestionSearch, ReplicationScope, UpdateQuestion,
        LATEST_SCHEMA_VERSION, MAX_ATTACHMENT_BYTES,
    },
    workspace_features::{
        backup::{
            create_consistent_backup, install_preflighted_restore, migrate_data_directory,
            preflight_restore, write_new_backup_atomically, BackupArchive, BackupCreateRequest,
            BackupEncryption, BackupFileRole, BackupKind, BackupPayloadSource,
            BackupScheduleConfig, ConsistentDatabaseSnapshot, DataDirectoryPlan, DatabasePreflight,
            DestinationProbe, RestorePreflightRequest, ScheduledBackupCandidate, SecretBytes,
            SwapPaths, UnlockMaterial, WorkspaceHealth,
        },
        export::{
            build_docx, build_tex, AttachmentSource, BundledTectonic, ExportArtifact, ExportFormat,
            ExportOptions, TectonicRunner,
        },
        generation::{
            generate, GenerationError, GenerationObserver, GenerationRequest, GeneticOptions,
            QuestionTypeTarget,
        },
        jobs::{
            CancelError, JobContext, JobFailure, JobId, JobKind, JobSnapshot, JobState,
            MaintenanceError, MaintenanceSubmitError, SubmitError,
        },
        paper::{
            AnswerSnapshot, AttachmentSnapshot, Difficulty as PaperDifficulty, EssayBlankSpace,
            PaperItemSnapshot, PaperSnapshot, PaperStatus, QuestionSnapshot,
            QuestionType as PaperQuestionType, ReplicationScope as PaperReplicationScope,
            PAPER_SCHEMA_VERSION,
        },
    },
};

use super::local_dto::{
    BackupPreflightDto, BackupScheduleDto, BackupScheduleEncryptionDto, BackupScheduleInputDto,
    CommandErrorDto, CreateWorkspaceBackupInputDto, DirectorySelectionDto, ExportPaperInputDto,
    GeneratePaperInputDto, ImportInspectionDto, JobSummaryDto, ManualBackupEncryptionDto,
    MutationBaseDto, QuestionDto, QuestionInputDto, QuestionRevisionDto, QuestionSearchPageDto,
    QuestionSearchRequestDto, RecoveryKeyDto, RestoreUnlockDto,
};

type CommandResult<T> = Result<T, CommandErrorDto>;

const AUTOMATIC_BACKUP_INITIAL_DELAY: Duration = Duration::from_secs(5);
const AUTOMATIC_BACKUP_POLL_INTERVAL: Duration = Duration::from_secs(60);
const MAX_RETENTION_INSPECTION_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub(crate) struct AutomaticBackupScheduler {
    stop: Arc<(Mutex<bool>, Condvar)>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl AutomaticBackupScheduler {
    pub(crate) fn start(app: AppHandle) -> std::io::Result<Self> {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("testpapers-automatic-backup".into())
            .spawn(move || automatic_backup_loop(app, worker_stop))?;
        Ok(Self {
            stop,
            worker: Mutex::new(Some(worker)),
        })
    }

    pub(crate) fn shutdown(&self) {
        let (lock, changed) = &*self.stop;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        changed.notify_all();
        if let Some(worker) = self
            .worker
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            let _ = worker.join();
        }
    }
}

impl Drop for AutomaticBackupScheduler {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct ActiveAutomaticBackup {
    id: JobId,
    destination: PathBuf,
    workspace_id: String,
    retention_days: u32,
}

fn automatic_backup_loop(app: AppHandle, stop: Arc<(Mutex<bool>, Condvar)>) {
    let mut delay = AUTOMATIC_BACKUP_INITIAL_DELAY;
    let mut active: Option<ActiveAutomaticBackup> = None;
    loop {
        if wait_for_scheduler_stop(&stop, delay) {
            return;
        }
        delay = AUTOMATIC_BACKUP_POLL_INTERVAL;
        automatic_backup_tick(&app, &mut active);
    }
}

fn wait_for_scheduler_stop(stop: &Arc<(Mutex<bool>, Condvar)>, delay: Duration) -> bool {
    let (lock, changed) = &**stop;
    let stopped = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if *stopped {
        return true;
    }
    let (stopped, _) = changed
        .wait_timeout(stopped, delay)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *stopped
}

fn automatic_backup_tick(app: &AppHandle, active: &mut Option<ActiveAutomaticBackup>) {
    let application = app.state::<LocalWorkspaceApplication>();
    if let Some(current) = active.as_ref() {
        let Some(snapshot) = application.jobs().get(&current.id) else {
            application.record_backup_attempt(now_micros(), false);
            *active = None;
            return;
        };
        if !matches!(
            snapshot.state,
            JobState::Completed | JobState::Failed | JobState::Cancelled
        ) {
            return;
        }
        let success = snapshot.state == JobState::Completed;
        application.record_backup_attempt(now_micros(), success);
        if success {
            prune_automatic_backups(
                &current.destination,
                &current.workspace_id,
                current.retention_days,
                now_micros(),
            );
        }
        *active = None;
    }

    let runtime = application.backup();
    let now = now_micros();
    if !runtime
        .automatic_state
        .is_due(now, &runtime.config)
        .unwrap_or(false)
    {
        return;
    }
    let Some(destination) = runtime.destination_path else {
        application.record_backup_attempt(now, false);
        return;
    };
    if !safe_backup_destination(&destination) {
        application.record_backup_attempt(now, false);
        return;
    }
    let engine = app.state::<EngineSupervisor>();
    let Some(workspace) = engine.workspace() else {
        return;
    };
    let workspace_id = workspace.identity.workspace_id.to_string();
    let retention_days = runtime.config.retention_days;
    let config = runtime.config;
    let target = destination.join(automatic_backup_file_name(
        &workspace_id,
        now,
        config.encrypted,
    ));
    let app_version = app.package_info().version.to_string();
    let submitted = application.jobs().submit(JobKind::Backup, move |context| {
        let plaintext = create_workspace_archive(
            &workspace,
            &app_version,
            BackupKind::Automatic,
            &context,
        )?;
        context.update_progress("encrypting", 2, Some(3));
        context.cancellation().checkpoint()?;
        let archive = if config.encrypted {
            let key_id = config.key_id.as_deref().ok_or_else(|| {
                JobFailure::fatal(
                    "AUTOMATIC_BACKUP_KEY_MISSING",
                    "Automatic backup encryption is not configured.",
                )
            })?;
            let provider = KeyringAgeIdentityProvider::production();
            let recipient = provider.recipient(key_id).map_err(|_| {
                JobFailure::recoverable(
                    "AUTOMATIC_BACKUP_KEY_UNAVAILABLE",
                    "The automatic backup key is unavailable in the OS credential store.",
                )
            })?;
            BackupEncryption::Recipient {
                recipient: &recipient,
                key_id,
            }
            .encrypt(&plaintext, &AuditedAgeBackend::new())
            .map_err(|_| {
                JobFailure::recoverable(
                    "AUTOMATIC_BACKUP_ENCRYPTION_FAILED",
                    "The automatic backup could not be encrypted.",
                )
            })?
        } else {
            plaintext
        };
        context.commit_started();
        write_new_backup_atomically(&archive, &target).map_err(|_| {
            JobFailure::recoverable(
                "AUTOMATIC_BACKUP_WRITE_FAILED",
                "The automatic backup could not be written to its configured destination.",
            )
        })?;
        context.update_progress("completed", 3, Some(3));
        Ok(json!({
            "displayName": target.file_name().and_then(|name| name.to_str()).unwrap_or("Automatic workspace backup"),
            "encrypted": config.encrypted,
            "byteSize": archive.len()
        }))
    });
    match submitted {
        Ok(id) => {
            *active = Some(ActiveAutomaticBackup {
                id,
                destination,
                workspace_id,
                retention_days,
            });
        }
        Err(_) => application.record_backup_attempt(now, false),
    }
}

fn safe_backup_destination(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
}

fn automatic_backup_file_name(workspace_id: &str, created_at: i64, encrypted: bool) -> String {
    let extension = if encrypted {
        "tpbackup.age"
    } else {
        "tpbackup"
    };
    format!(
        "TestPapers-Automatic-{workspace_id}-{created_at}-{}.{}",
        Uuid::now_v7().as_simple(),
        extension
    )
}

fn is_automatic_backup_file(name: &str) -> bool {
    name.starts_with("TestPapers-Automatic-")
        && (name.ends_with(".tpbackup") || name.ends_with(".tpbackup.age"))
}

fn prune_automatic_backups(destination: &Path, workspace_id: &str, retention_days: u32, now: i64) {
    if !safe_backup_destination(destination) {
        return;
    }
    let Ok(entries) = fs::read_dir(destination) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_automatic_backup_file(&name) {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAX_RETENTION_INSPECTION_BYTES
        {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let plaintext = if bytes.starts_with(b"age-encryption.org/v1") {
            let provider = KeyringAgeIdentityProvider::production();
            let key_id = backup_key_id(workspace_id);
            match (UnlockMaterial::Keychain {
                key_id: &key_id,
                provider: &provider,
            })
            .decrypt(&bytes, &AuditedAgeBackend::new())
            {
                Ok(plaintext) => plaintext,
                Err(_) => continue,
            }
        } else {
            bytes
        };
        let Ok(verified) = BackupArchive::inspect(&plaintext, Default::default()) else {
            continue;
        };
        let candidate = ScheduledBackupCandidate {
            path_id: name,
            workspace_id: verified.manifest.workspace_id.clone(),
            created_at_micros: verified.manifest.created_at_micros,
            automatic: verified.manifest.kind == BackupKind::Automatic,
            verified: true,
        };
        if candidate.can_delete_for_retention(now, workspace_id, retention_days) {
            let _ = fs::remove_file(path);
        }
    }
}

fn ready_workspace(engine: &EngineSupervisor) -> CommandResult<WorkspaceRuntime> {
    let snapshot = engine.snapshot();
    if snapshot.maintenance_mode
        && snapshot
            .last_error
            .as_ref()
            .is_none_or(|failure| failure.code != "maintenance_pause_timeout")
    {
        return Err(CommandErrorDto::recoverable(
            "workspace_maintenance",
            "The local workspace is temporarily unavailable during maintenance.",
            SuggestedAction::Retry,
        ));
    }
    if !snapshot.database_available {
        return Err(CommandErrorDto::recoverable(
            "local_engine_not_ready",
            "The Local Engine database is not ready.",
            SuggestedAction::Retry,
        ));
    }
    engine.workspace().ok_or_else(|| {
        CommandErrorDto::recoverable(
            "local_engine_not_ready",
            "The Local Engine workspace is not ready.",
            SuggestedAction::Retry,
        )
    })
}

fn ensure_maintenance_available(engine: &EngineSupervisor) -> CommandResult<()> {
    let snapshot = engine.snapshot();
    if snapshot.state == EngineState::Stopping {
        return Err(CommandErrorDto::fatal(
            "local_engine_stopping",
            "The Local Engine is stopping and cannot start workspace maintenance.",
            SuggestedAction::RestartApp,
        ));
    }
    if snapshot.maintenance_mode {
        return Err(CommandErrorDto::recoverable(
            "workspace_maintenance",
            "Another exclusive workspace maintenance operation is already active.",
            SuggestedAction::Retry,
        ));
    }
    Ok(())
}

fn map_engine_failure(failure: EngineFailure) -> CommandErrorDto {
    if failure.recoverable {
        CommandErrorDto::recoverable(failure.code, failure.message, failure.suggested_action)
    } else {
        CommandErrorDto::fatal(failure.code, failure.message, failure.suggested_action)
    }
}

async fn blocking<T, F>(operation: F) -> CommandResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> CommandResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| {
            CommandErrorDto::recoverable(
                "local_worker_unavailable",
                "The Local Engine worker stopped before completing the operation.",
                SuggestedAction::Retry,
            )
        })?
}

fn map_local_data_error(error: LocalDataError) -> CommandErrorDto {
    match error {
        LocalDataError::Validation(errors) => CommandErrorDto::recoverable(
            "validation_failed",
            errors.join("; "),
            SuggestedAction::Retry,
        ),
        LocalDataError::NotFound { entity, id } => CommandErrorDto::recoverable(
            "entity_not_found",
            format!("{entity} {id} was not found."),
            SuggestedAction::Retry,
        ),
        LocalDataError::StaleBase {
            current_version,
            current_content_hash,
            candidate_id,
        } => CommandErrorDto::recoverable(
            "stale_mutation_base",
            format!(
                "This item changed locally (version {current_version}, hash {current_content_hash}); candidate {candidate_id} was preserved."
            ),
            SuggestedAction::Retry,
        ),
        LocalDataError::EntityDeleted { entity, id } => CommandErrorDto::recoverable(
            "entity_deleted",
            format!("{entity} {id} must be restored before it can be edited."),
            SuggestedAction::Retry,
        ),
        LocalDataError::Busy(_) => CommandErrorDto::recoverable(
            "database_busy",
            "The local database is busy; retry the operation.",
            SuggestedAction::Retry,
        ),
        LocalDataError::UnsupportedSchema { .. } => CommandErrorDto::fatal(
            "database_schema_too_new",
            "This workspace requires a newer TestPapers Desktop version.",
            SuggestedAction::RestartApp,
        ),
        LocalDataError::Corrupt(_) | LocalDataError::WorkspaceMismatch { .. } => {
            CommandErrorDto::recoverable(
                "database_integrity_failed",
                "The local workspace failed an integrity check.",
                SuggestedAction::Restore,
            )
        }
        LocalDataError::UnsafePath(_)
        | LocalDataError::Io(_)
        | LocalDataError::Sqlite(_)
        | LocalDataError::Json(_)
        | LocalDataError::Csv(_) => CommandErrorDto::recoverable(
            "local_data_operation_failed",
            "The local data operation could not be completed.",
            SuggestedAction::Retry,
        ),
    }
}

fn question_dto(
    store: &LocalDataStore,
    question: QuestionRecord,
) -> Result<QuestionDto, LocalDataError> {
    let attachments = store.list_question_attachments(&question.id, false)?;
    Ok(QuestionDto::new(question, attachments))
}

#[tauri::command]
pub(crate) async fn search_questions(
    engine: State<'_, EngineSupervisor>,
    request: QuestionSearchRequestDto,
) -> CommandResult<QuestionSearchPageDto> {
    let store = ready_workspace(&engine)?.store;
    blocking(move || {
        let page = store
            .search_questions(request.into())
            .map_err(map_local_data_error)?;
        let items = page
            .items
            .into_iter()
            .map(|question| question_dto(&store, question))
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_local_data_error)?;
        Ok(QuestionSearchPageDto::new(items, page.next_cursor))
    })
    .await
}

#[tauri::command]
pub(crate) async fn get_question(
    engine: State<'_, EngineSupervisor>,
    id: String,
) -> CommandResult<QuestionDto> {
    let store = ready_workspace(&engine)?.store;
    blocking(move || {
        let question = store.get_question(&id).map_err(map_local_data_error)?;
        question_dto(&store, question).map_err(map_local_data_error)
    })
    .await
}

#[tauri::command]
pub(crate) async fn add_question_image(
    app: AppHandle,
    engine: State<'_, EngineSupervisor>,
    question_id: String,
    caption: Option<String>,
) -> CommandResult<Option<QuestionDto>> {
    let store = ready_workspace(&engine)?.store;
    let dialog_app = app.clone();
    let selection = blocking(move || {
        dialogs::select_question_image(&dialog_app).map_err(|message| {
            CommandErrorDto::recoverable(
                "question_image_selection_failed",
                message,
                SuggestedAction::Retry,
            )
        })
    })
    .await?;
    let Some(path) = selection else {
        return Ok(None);
    };
    blocking(move || {
        let metadata = fs::metadata(&path).map_err(|_| {
            CommandErrorDto::recoverable(
                "question_image_unavailable",
                "The selected image is unavailable.",
                SuggestedAction::Retry,
            )
        })?;
        if !metadata.is_file() || metadata.len() > MAX_ATTACHMENT_BYTES {
            return Err(CommandErrorDto::recoverable(
                "question_image_too_large",
                "Question images must be files no larger than 30 MB.",
                SuggestedAction::Retry,
            ));
        }
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                CommandErrorDto::recoverable(
                    "question_image_name_invalid",
                    "The selected image has an unsupported file name.",
                    SuggestedAction::Retry,
                )
            })?
            .to_owned();
        let media_type = question_image_media_type(&path).ok_or_else(|| {
            CommandErrorDto::recoverable(
                "question_image_type_unsupported",
                "Select a PNG, JPEG, GIF, or WebP image.",
                SuggestedAction::Retry,
            )
        })?;
        let position = u32::try_from(
            store
                .list_question_attachments(&question_id, false)
                .map_err(map_local_data_error)?
                .len(),
        )
        .map_err(|_| {
            CommandErrorDto::recoverable(
                "question_image_limit_reached",
                "This question has too many images.",
                SuggestedAction::Retry,
            )
        })?;
        let mut file = File::open(&path).map_err(|_| {
            CommandErrorDto::recoverable(
                "question_image_unavailable",
                "The selected image could not be opened.",
                SuggestedAction::Retry,
            )
        })?;
        if !verify_question_image_signature(&mut file, media_type).map_err(|_| {
            CommandErrorDto::recoverable(
                "question_image_unavailable",
                "The selected image could not be read.",
                SuggestedAction::Retry,
            )
        })? {
            return Err(CommandErrorDto::recoverable(
                "question_image_content_invalid",
                "The selected file content does not match its image type.",
                SuggestedAction::Retry,
            ));
        }
        store
            .add_question_attachment(
                NewQuestionAttachment {
                    question_id: question_id.clone(),
                    file_name,
                    media_type: media_type.to_owned(),
                    caption,
                    position,
                    uploaded_by_id: Some(store.local_principal_id().to_owned()),
                },
                file,
            )
            .map_err(map_local_data_error)?;
        let question = store
            .get_question(&question_id)
            .map_err(map_local_data_error)?;
        question_dto(&store, question)
            .map(Some)
            .map_err(map_local_data_error)
    })
    .await
}

fn question_image_media_type(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())?
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn verify_question_image_signature(
    reader: &mut (impl Read + Seek),
    media_type: &str,
) -> std::io::Result<bool> {
    let mut header = [0_u8; 12];
    let length = reader.read(&mut header)?;
    reader.seek(SeekFrom::Start(0))?;
    Ok(match media_type {
        "image/png" => length >= 8 && header[..8] == *b"\x89PNG\r\n\x1a\n",
        "image/jpeg" => length >= 3 && header[..3] == *b"\xff\xd8\xff",
        "image/gif" => length >= 6 && (header[..6] == *b"GIF87a" || header[..6] == *b"GIF89a"),
        "image/webp" => length >= 12 && header[..4] == *b"RIFF" && header[8..12] == *b"WEBP",
        _ => false,
    })
}

#[tauri::command]
pub(crate) async fn create_question(
    engine: State<'_, EngineSupervisor>,
    input: QuestionInputDto,
) -> CommandResult<QuestionDto> {
    let store = ready_workspace(&engine)?.store;
    blocking(move || {
        let question = store
            .create_question(CreateQuestion {
                owner_id: None,
                replication_scope: ReplicationScope::LocalPrivate,
                content: input.into(),
            })
            .map_err(map_local_data_error)?;
        question_dto(&store, question).map_err(map_local_data_error)
    })
    .await
}

#[tauri::command]
pub(crate) async fn update_question(
    engine: State<'_, EngineSupervisor>,
    id: String,
    base: MutationBaseDto,
    input: QuestionInputDto,
) -> CommandResult<QuestionDto> {
    let store = ready_workspace(&engine)?.store;
    blocking(move || {
        let question = store
            .update_question(
                &id,
                UpdateQuestion {
                    mutation_base: base.into(),
                    content: input.into(),
                },
            )
            .map_err(map_local_data_error)?;
        question_dto(&store, question).map_err(map_local_data_error)
    })
    .await
}

#[tauri::command]
pub(crate) async fn delete_question(
    engine: State<'_, EngineSupervisor>,
    id: String,
    base: MutationBaseDto,
) -> CommandResult<QuestionDto> {
    let workspace = ready_workspace(&engine)?;
    let actor_id = workspace.identity.local_principal_id.to_string();
    let store = workspace.store;
    blocking(move || {
        let question = store
            .delete_question(&id, base.into(), &actor_id)
            .map_err(map_local_data_error)?;
        question_dto(&store, question).map_err(map_local_data_error)
    })
    .await
}

#[tauri::command]
pub(crate) async fn restore_question(
    engine: State<'_, EngineSupervisor>,
    id: String,
    base: MutationBaseDto,
) -> CommandResult<QuestionDto> {
    let store = ready_workspace(&engine)?.store;
    blocking(move || {
        let question = store
            .restore_question(&id, base.into())
            .map_err(map_local_data_error)?;
        question_dto(&store, question).map_err(map_local_data_error)
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_question_revisions(
    engine: State<'_, EngineSupervisor>,
    id: String,
) -> CommandResult<Vec<QuestionRevisionDto>> {
    let store = ready_workspace(&engine)?.store;
    blocking(move || {
        store
            .list_question_revisions(&id)
            .map(|revisions| {
                revisions
                    .into_iter()
                    .map(|revision| QuestionRevisionDto::new(&id, revision))
                    .collect()
            })
            .map_err(map_local_data_error)
    })
    .await
}

#[tauri::command]
pub(crate) async fn revert_question(
    engine: State<'_, EngineSupervisor>,
    id: String,
    base: MutationBaseDto,
    version: i64,
) -> CommandResult<QuestionDto> {
    let store = ready_workspace(&engine)?.store;
    blocking(move || {
        let question = store
            .revert_question(&id, version, base.into())
            .map_err(map_local_data_error)?;
        question_dto(&store, question).map_err(map_local_data_error)
    })
    .await
}

#[tauri::command]
pub(crate) async fn select_question_import(
    app: AppHandle,
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
) -> CommandResult<Option<ImportInspectionDto>> {
    let store = ready_workspace(&engine)?.store;
    let selection = blocking(move || {
        dialogs::select_question_import(&app).map_err(|message| {
            CommandErrorDto::recoverable(
                "question_import_selection_failed",
                message,
                SuggestedAction::Retry,
            )
        })
    })
    .await?;
    let Some(path) = selection else {
        return Ok(None);
    };
    let display_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Selected question data")
        .to_owned();
    let inspection = blocking(move || {
        let file = File::open(&path).map_err(|_| {
            CommandErrorDto::recoverable(
                "question_import_unavailable",
                "The selected question file could not be opened.",
                SuggestedAction::Retry,
            )
        })?;
        store
            .inspect_question_import(file, &display_name)
            .map(|inspection| (display_name, inspection))
            .map_err(map_local_data_error)
    })
    .await?;
    let import_id = application.register_import(inspection.1.clone());
    Ok(Some(ImportInspectionDto::new(
        import_id,
        inspection.0,
        &inspection.1,
    )))
}

#[tauri::command]
pub(crate) fn commit_question_import(
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
    import_id: String,
) -> CommandResult<JobSummaryDto> {
    let store = ready_workspace(&engine)?.store;
    let inspection = application.take_import(&import_id).ok_or_else(|| {
        CommandErrorDto::recoverable(
            "question_import_expired",
            "This question import preview has expired; select the file again.",
            SuggestedAction::Retry,
        )
    })?;
    let total = inspection.valid_count() as u64;
    let backup = inspection.clone();
    let submitted = application.jobs().submit(JobKind::Import, move |context| {
        context.update_progress("validating", 0, Some(total));
        context.cancellation().checkpoint()?;
        context.commit_started();
        let result = store.commit_question_import(&inspection).map_err(|error| {
            JobFailure::recoverable("IMPORT_FAILED", map_local_data_error(error).to_string())
        })?;
        context.update_progress("completed", result.created_ids.len() as u64, Some(total));
        Ok(json!({
            "createdIds": result.created_ids,
            "invalidRows": result.invalid_rows
        }))
    });
    let id = match submitted {
        Ok(id) => id,
        Err(error) => {
            application.restore_import(import_id, backup);
            return Err(map_submit_error(error));
        }
    };
    job_snapshot(application.jobs().get(&id))
}

#[tauri::command]
pub(crate) fn discard_question_import(
    application: State<'_, LocalWorkspaceApplication>,
    import_id: String,
) -> CommandResult<()> {
    if application.discard_import(&import_id) {
        Ok(())
    } else {
        Err(CommandErrorDto::recoverable(
            "question_import_expired",
            "This question import preview has already been discarded.",
            SuggestedAction::Retry,
        ))
    }
}

#[tauri::command]
pub(crate) fn generate_paper(
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
    input: GeneratePaperInputDto,
) -> CommandResult<JobSummaryDto> {
    let workspace = ready_workspace(&engine)?;
    let total_marks = input.total_marks.parse::<u32>().map_err(|_| {
        CommandErrorDto::recoverable(
            "invalid_total_marks",
            "Generated papers currently require a positive whole-number total mark.",
            SuggestedAction::Retry,
        )
    })?;
    let request = GenerationRequest {
        total_marks,
        difficulty_coefficient: input.difficulty_coefficient,
        question_types: input
            .question_types
            .into_iter()
            .map(|target| QuestionTypeTarget {
                question_type: target.question_type,
                count: target.count,
            })
            .collect(),
        subjects: input.subjects.clone(),
        required_tags: input.required_tags,
        preferred_tags: input.preferred_tags,
        random_seed: input.seed,
        options: GeneticOptions::default(),
    };
    let store = workspace.store;
    let owner_id = workspace.identity.local_principal_id.to_string();
    let title = input.title;
    let subject = input.subjects.join(", ");
    let duration_minutes = input.duration_minutes;
    let total_marks_text = input.total_marks;
    let submitted = application
        .jobs()
        .submit(JobKind::Generation, move |context| {
            let candidates = load_generation_candidates(&store, &request, &context)?;
            context.update_progress("optimizing", 0, Some(request.options.generations as u64));
            let observer = JobGenerationObserver(context.clone());
            let generated =
                generate(&request, &candidates, &observer).map_err(map_generation_error)?;
            context.cancellation().checkpoint()?;
            let paper = PaperSnapshot {
                id: Uuid::now_v7().to_string(),
                owner_id,
                replication_scope: PaperReplicationScope::LocalPrivate,
                schema_version: PAPER_SCHEMA_VERSION,
                version: 1,
                content_hash: "0".repeat(64),
                created_at_micros: 0,
                updated_at_micros: 0,
                deleted_at_micros: None,
                title: title.trim().to_owned(),
                subject,
                duration_minutes,
                total_marks: total_marks_text,
                status: PaperStatus::Draft,
                items: generated
                    .selected
                    .into_iter()
                    .enumerate()
                    .map(|(order, generated)| PaperItemSnapshot {
                        id: Uuid::now_v7().to_string(),
                        question_id: Some(generated.question.id.clone()),
                        order: order as u32,
                        marks: Some(generated.marks.to_string()),
                        question_snapshot: generated.question,
                    })
                    .collect(),
            };
            context.commit_started();
            let accepted = store
                .create_paper_snapshot(&paper)
                .map_err(|error| job_local_data_failure("PAPER_SAVE_FAILED", error))?;
            Ok(json!({
                "paperId": accepted.id,
                "version": accepted.version,
                "contentHash": accepted.content_hash,
                "diagnostics": generated.diagnostics
            }))
        });
    let id = submitted.map_err(map_submit_error)?;
    job_snapshot(application.jobs().get(&id))
}

#[tauri::command]
pub(crate) async fn export_paper(
    app: AppHandle,
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
    input: ExportPaperInputDto,
) -> CommandResult<JobSummaryDto> {
    let workspace = ready_workspace(&engine)?;
    let store = Arc::clone(&workspace.store);
    let paper_id = input.paper_id.clone();
    let paper = blocking(move || {
        store
            .load_paper_snapshot(&paper_id)
            .map_err(map_local_data_error)?
            .ok_or_else(|| {
                CommandErrorDto::recoverable(
                    "paper_not_found",
                    "The selected paper was not found.",
                    SuggestedAction::Retry,
                )
            })
    })
    .await?;
    let suggested_name = export_suggested_name(&paper.title, input.format);
    let dialog_app = app.clone();
    let format = input.format;
    let target = blocking(move || {
        dialogs::select_paper_export(&dialog_app, format, &suggested_name).map_err(|message| {
            CommandErrorDto::recoverable(
                "paper_export_selection_failed",
                message,
                SuggestedAction::Retry,
            )
        })
    })
    .await?
    .ok_or_else(|| {
        CommandErrorDto::recoverable(
            "paper_export_cancelled",
            "Paper export was cancelled.",
            SuggestedAction::Retry,
        )
    })?;
    let resource_root = app.path().resource_dir().map_err(|_| {
        CommandErrorDto::recoverable(
            "paper_export_resources_unavailable",
            "The packaged export resources are unavailable.",
            SuggestedAction::RestartApp,
        )
    })?;
    let options = ExportOptions {
        include_answers: input.include_answers,
        question_order: input.question_order,
        layout_density: input.layout_density,
    };
    let store = workspace.store;
    let workspace_root = workspace.root;
    let submitted = application.jobs().submit(JobKind::Export, move |context| {
        context.update_progress("rendering", 0, Some(3));
        context.cancellation().checkpoint()?;
        let attachments = StoreAttachmentSource(Arc::clone(&store));
        let artifact = match format {
            ExportFormat::Docx => build_docx(&paper, options, &attachments).map_err(|error| {
                JobFailure::recoverable("DOCX_EXPORT_FAILED", error.to_string())
            })?,
            ExportFormat::Tex => build_tex(&paper, options, &attachments)
                .map_err(|error| JobFailure::recoverable("TEX_EXPORT_FAILED", error.to_string()))?,
            ExportFormat::Pdf => build_pdf_artifact(
                &paper,
                options,
                &attachments,
                &workspace_root,
                &resource_root,
                &context,
            )?,
        };
        context.update_progress("validating", 2, Some(3));
        context.cancellation().checkpoint()?;
        context.commit_started();
        publish_export_artifact(&artifact, &target)
            .map_err(|message| JobFailure::recoverable("EXPORT_WRITE_FAILED", message))?;
        let display_name = target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Paper export")
            .to_owned();
        context.update_progress("completed", 3, Some(3));
        Ok(json!({
            "paperId": paper.id,
            "displayName": display_name,
            "format": format,
            "byteSize": artifact.bytes.len()
        }))
    });
    let id = submitted.map_err(map_submit_error)?;
    job_snapshot(application.jobs().get(&id))
}

#[tauri::command]
pub(crate) fn get_backup_schedule(
    application: State<'_, LocalWorkspaceApplication>,
) -> BackupScheduleDto {
    BackupScheduleDto::new(application.backup(), now_micros())
}

#[tauri::command]
pub(crate) async fn select_backup_destination(
    app: AppHandle,
    application: State<'_, LocalWorkspaceApplication>,
) -> CommandResult<Option<DirectorySelectionDto>> {
    let selection = blocking(move || {
        dialogs::select_backup_destination(&app).map_err(|message| {
            CommandErrorDto::recoverable(
                "backup_destination_selection_failed",
                message,
                SuggestedAction::ChooseDirectory,
            )
        })
    })
    .await?;
    let Some(path) = selection else {
        return Ok(None);
    };
    let probe = blocking(move || probe_directory(path)).await?;
    Ok(Some(
        application
            .register_directory(probe.0, probe.1, probe.2)
            .into(),
    ))
}

#[tauri::command]
pub(crate) fn prepare_backup_encryption(
    engine: State<'_, EngineSupervisor>,
) -> CommandResult<RecoveryKeyDto> {
    let workspace = ready_workspace(&engine)?;
    let key_id = backup_key_id(&workspace.identity.workspace_id.to_string());
    let provider = KeyringAgeIdentityProvider::production();
    provider
        .load_or_create_recipient(&key_id)
        .map_err(|error| {
            CommandErrorDto::recoverable(
                "backup_keychain_unavailable",
                error.to_string(),
                SuggestedAction::Retry,
            )
        })?;
    let recovery = provider
        .export_recovery_identity(&key_id)
        .map_err(|error| {
            CommandErrorDto::recoverable(
                "backup_recovery_key_unavailable",
                error.to_string(),
                SuggestedAction::Retry,
            )
        })?;
    let recovery_key = std::str::from_utf8(recovery.expose())
        .map_err(|_| {
            CommandErrorDto::fatal(
                "backup_recovery_key_invalid",
                "The generated recovery key is invalid.",
                SuggestedAction::ContactSupport,
            )
        })?
        .to_owned();
    Ok(RecoveryKeyDto::new(key_id, recovery_key))
}

#[tauri::command]
pub(crate) fn configure_backup_schedule(
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
    input: BackupScheduleInputDto,
) -> CommandResult<BackupScheduleDto> {
    let workspace = ready_workspace(&engine)?;
    let current = application.backup();
    let destination = input
        .destination_selection_id
        .as_deref()
        .map(|id| {
            application.directory(id).ok_or_else(|| {
                CommandErrorDto::recoverable(
                    "backup_destination_expired",
                    "The selected backup destination has expired; choose it again.",
                    SuggestedAction::ChooseDirectory,
                )
            })
        })
        .transpose()?;
    if destination.as_ref().is_some_and(|grant| !grant.writable) {
        return Err(CommandErrorDto::recoverable(
            "backup_destination_read_only",
            "The selected backup destination is not writable.",
            SuggestedAction::ChooseDirectory,
        ));
    }
    let encrypted = matches!(input.encryption_mode, BackupScheduleEncryptionDto::Keychain);
    let key_id = encrypted.then(|| backup_key_id(&workspace.identity.workspace_id.to_string()));
    if input.enabled && encrypted {
        let provider = KeyringAgeIdentityProvider::production();
        provider
            .load_or_create_recipient(key_id.as_deref().expect("encrypted key ID"))
            .map_err(|error| {
                CommandErrorDto::recoverable(
                    "backup_keychain_unavailable",
                    error.to_string(),
                    SuggestedAction::Retry,
                )
            })?;
    }
    let destination_id = destination
        .as_ref()
        .map(|grant| grant.id.clone())
        .or(current.config.destination_id);
    let config = BackupScheduleConfig {
        enabled: input.enabled,
        destination_id,
        interval_minutes: input.interval_minutes,
        retention_days: input.retention_days,
        encrypted,
        key_id,
        recovery_key_confirmed: input.recovery_key_confirmed,
    };
    let runtime = application
        .configure_backup(config, destination)
        .map_err(|message| {
            CommandErrorDto::recoverable("backup_schedule_invalid", message, SuggestedAction::Retry)
        })?;
    Ok(BackupScheduleDto::new(runtime, now_micros()))
}

#[tauri::command]
pub(crate) async fn create_workspace_backup(
    app: AppHandle,
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
    input: CreateWorkspaceBackupInputDto,
) -> CommandResult<JobSummaryDto> {
    let workspace = ready_workspace(&engine)?;
    let passphrase = match input.encryption_mode {
        ManualBackupEncryptionDto::Passphrase => {
            let value = input.passphrase.unwrap_or_default().into_bytes();
            if value.len() < 8 {
                return Err(CommandErrorDto::recoverable(
                    "backup_passphrase_too_short",
                    "Portable backup passphrases must contain at least eight UTF-8 bytes.",
                    SuggestedAction::Retry,
                ));
            }
            Some(value)
        }
        ManualBackupEncryptionDto::None => None,
    };
    let encrypted = passphrase.is_some();
    let suggested_name = if encrypted {
        format!("TestPapers-{}.tpbackup.age", now_micros())
    } else {
        format!("TestPapers-{}.tpbackup", now_micros())
    };
    let dialog_app = app.clone();
    let target = blocking(move || {
        dialogs::select_manual_backup_target(&dialog_app, encrypted, &suggested_name).map_err(
            |message| {
                CommandErrorDto::recoverable(
                    "backup_target_selection_failed",
                    message,
                    SuggestedAction::ChooseDirectory,
                )
            },
        )
    })
    .await?
    .ok_or_else(|| {
        CommandErrorDto::recoverable(
            "backup_cancelled",
            "Workspace backup was cancelled.",
            SuggestedAction::Retry,
        )
    })?;
    let app_version = app.package_info().version.to_string();
    let submitted = application.jobs().submit(JobKind::Backup, move |context| {
        let plaintext = create_workspace_archive(
            &workspace,
            &app_version,
            BackupKind::Manual,
            &context,
        )?;
        context.update_progress("encrypting", 2, Some(3));
        context.cancellation().checkpoint()?;
        let archive = if let Some(passphrase) = passphrase {
            let secret = SecretBytes::new(passphrase).map_err(|error| {
                JobFailure::recoverable("BACKUP_PASSPHRASE_INVALID", error.to_string())
            })?;
            BackupEncryption::Passphrase(&secret)
                .encrypt(&plaintext, &AuditedAgeBackend::new())
                .map_err(|error| {
                    JobFailure::recoverable("BACKUP_ENCRYPTION_FAILED", error.to_string())
                })?
        } else {
            plaintext
        };
        context.commit_started();
        write_new_backup_atomically(&archive, &target).map_err(|error| {
            JobFailure::recoverable("BACKUP_WRITE_FAILED", error.to_string())
        })?;
        context.update_progress("completed", 3, Some(3));
        Ok(json!({
            "displayName": target.file_name().and_then(|name| name.to_str()).unwrap_or("Workspace backup"),
            "encrypted": encrypted,
            "byteSize": archive.len()
        }))
    });
    let id = submitted.map_err(map_submit_error)?;
    job_snapshot(application.jobs().get(&id))
}

#[tauri::command]
pub(crate) async fn select_workspace_restore(
    app: AppHandle,
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
    unlock: RestoreUnlockDto,
) -> CommandResult<Option<BackupPreflightDto>> {
    ensure_maintenance_available(&engine)?;
    let dialog_app = app.clone();
    let selection = blocking(move || {
        dialogs::select_restore_archive(&dialog_app).map_err(|message| {
            CommandErrorDto::recoverable(
                "restore_selection_failed",
                message,
                SuggestedAction::Restore,
            )
        })
    })
    .await?;
    let Some(path) = selection else {
        return Ok(None);
    };
    let display_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Selected workspace backup")
        .to_owned();
    let current_root = application.workspace_root().map_err(map_engine_failure)?;
    let current_workspace_id = application.workspace_id().ok();
    let restore_workspace_id = current_workspace_id.clone();
    let prepared = blocking(move || {
        let metadata = fs::metadata(&path).map_err(|_| {
            CommandErrorDto::recoverable(
                "restore_archive_unavailable",
                "The selected backup archive is unavailable.",
                SuggestedAction::Restore,
            )
        })?;
        if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 * 1024 {
            return Err(CommandErrorDto::recoverable(
                "restore_archive_too_large",
                "The selected backup archive exceeds the supported size.",
                SuggestedAction::Restore,
            ));
        }
        let encrypted_archive = fs::read(&path).map_err(|_| {
            CommandErrorDto::recoverable(
                "restore_archive_unavailable",
                "The selected backup archive could not be read.",
                SuggestedAction::Restore,
            )
        })?;
        let encrypted = encrypted_archive.starts_with(b"age-encryption.org/v1");
        let plaintext = if encrypted {
            decrypt_restore_archive(&encrypted_archive, &unlock, restore_workspace_id.as_deref())?
        } else {
            encrypted_archive
        };
        let parent = current_root.parent().ok_or_else(|| {
            CommandErrorDto::fatal(
                "restore_workspace_invalid",
                "The current workspace has no valid parent directory.",
                SuggestedAction::ContactSupport,
            )
        })?;
        let staging = parent.join(format!(".testpapers-restore-{}.staging", Uuid::now_v7()));
        fs::create_dir(&staging).map_err(|_| {
            CommandErrorDto::recoverable(
                "restore_staging_unavailable",
                "Restore staging could not be created beside the workspace.",
                SuggestedAction::Restore,
            )
        })?;
        let request = RestorePreflightRequest {
            staging_directory: staging.clone(),
            supported_schema_version: LATEST_SCHEMA_VERSION,
            archive_limits: Default::default(),
        };
        match preflight_restore(&plaintext, &request, &SqliteRestorePreflight) {
            Ok(prepared) => Ok((prepared, encrypted)),
            Err(error) => {
                let _ = fs::remove_dir_all(staging);
                Err(CommandErrorDto::recoverable(
                    "restore_preflight_failed",
                    error.to_string(),
                    SuggestedAction::Restore,
                ))
            }
        }
    })
    .await?;
    let restore_id = Uuid::now_v7().to_string();
    let manifest = prepared.0.manifest.clone();
    let warnings = current_workspace_id
        .as_ref()
        .is_some_and(|workspace_id| manifest.workspace_id != *workspace_id)
        .then(|| "This backup belongs to a different workspace and will replace the current workspace identity.".to_owned())
        .into_iter()
        .collect();
    application.register_restore(crate::application::RestoreGrant {
        id: restore_id.clone(),
        prepared: prepared.0,
    });
    Ok(Some(BackupPreflightDto::new(
        restore_id,
        display_name,
        manifest.workspace_id,
        manifest.app_version,
        manifest.schema_version,
        manifest.created_at_micros,
        prepared.1,
        false,
        true,
        warnings,
    )))
}

#[tauri::command]
pub(crate) fn commit_workspace_restore(
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
    restore_id: String,
) -> CommandResult<JobSummaryDto> {
    ensure_maintenance_available(&engine)?;
    let grant = application.take_restore(&restore_id).ok_or_else(|| {
        CommandErrorDto::recoverable(
            "restore_preflight_expired",
            "This restore preflight has expired; select the backup again.",
            SuggestedAction::Restore,
        )
    })?;
    let backup = grant.clone();
    let supervisor = engine.inner().clone();
    let current = application.workspace_root().map_err(map_engine_failure)?;
    let submitted = application.jobs().submit_maintenance(
        JobKind::Restore,
        Duration::from_secs(30),
        move |context| {
            supervisor
                .pause_for_maintenance(Duration::from_secs(10))
                .map_err(|failure| {
                    JobFailure::recoverable("RESTORE_PAUSE_FAILED", failure.message)
                })?;
            if let Err(cancelled) = context.cancellation().checkpoint() {
                let _ = supervisor.resume_after_maintenance();
                return Err(cancelled);
            }
            context.commit_started();
            let parent = current.parent().ok_or_else(|| {
                JobFailure::fatal("RESTORE_PATH_INVALID", "Workspace parent is unavailable.")
            })?;
            let nonce = Uuid::now_v7();
            let paths = SwapPaths {
                current: current.clone(),
                staging: grant.prepared.staging_directory.clone(),
                rollback: parent.join(format!("workspace-pre-restore-{nonce}")),
                failed: parent.join(format!("workspace-failed-restore-{nonce}")),
            };
            let install = install_preflighted_restore(
                grant.prepared.clone(),
                &paths,
                &LocalWorkspaceHealth,
            );
            let resume = supervisor.resume_after_maintenance();
            install.map_err(|error| {
                JobFailure::recoverable("RESTORE_INSTALL_FAILED", error.to_string())
            })?;
            resume.map_err(|failure| {
                JobFailure::recoverable("RESTORE_REOPEN_FAILED", failure.message)
            })?;
            Ok(json!({
                "workspaceId": grant.prepared.manifest.workspace_id,
                "rollbackDisplayName": paths.rollback.file_name().and_then(|name| name.to_str()).unwrap_or("Pre-restore workspace")
            }))
        },
    );
    let id = match submitted {
        Ok(id) => id,
        Err(error) => {
            application.register_restore(backup);
            return Err(map_maintenance_submit_error(error));
        }
    };
    job_snapshot(application.jobs().get(&id))
}

#[tauri::command]
pub(crate) fn discard_workspace_restore(
    application: State<'_, LocalWorkspaceApplication>,
    restore_id: String,
) -> CommandResult<()> {
    let grant = application.discard_restore(&restore_id).ok_or_else(|| {
        CommandErrorDto::recoverable(
            "restore_preflight_expired",
            "This restore preflight has already been discarded.",
            SuggestedAction::Restore,
        )
    })?;
    fs::remove_dir_all(grant.prepared.staging_directory).map_err(|_| {
        CommandErrorDto::recoverable(
            "restore_staging_cleanup_failed",
            "Restore staging could not be cleaned up.",
            SuggestedAction::Retry,
        )
    })
}

#[tauri::command]
pub(crate) async fn select_data_directory(
    app: AppHandle,
    application: State<'_, LocalWorkspaceApplication>,
) -> CommandResult<Option<DirectorySelectionDto>> {
    let selection = blocking(move || {
        dialogs::select_data_directory_parent(&app).map_err(|message| {
            CommandErrorDto::recoverable(
                "data_directory_selection_failed",
                message,
                SuggestedAction::ChooseDirectory,
            )
        })
    })
    .await?;
    let Some(parent) = selection else {
        return Ok(None);
    };
    let probe = blocking(move || probe_directory(parent)).await?;
    let mut destination = probe.0.join("TestPapers Workspace");
    if destination.exists() {
        destination = probe
            .0
            .join(format!("TestPapers Workspace {}", now_micros()));
    }
    Ok(Some(
        application
            .register_directory(destination, probe.1, probe.2)
            .into(),
    ))
}

#[tauri::command]
pub(crate) fn migrate_workspace_data_directory(
    engine: State<'_, EngineSupervisor>,
    application: State<'_, LocalWorkspaceApplication>,
    selection_id: String,
) -> CommandResult<JobSummaryDto> {
    ensure_maintenance_available(&engine)?;
    let workspace_root = application.workspace_root().map_err(map_engine_failure)?;
    let selection = application.directory(&selection_id).ok_or_else(|| {
        CommandErrorDto::recoverable(
            "data_directory_selection_expired",
            "The selected data directory has expired; choose it again.",
            SuggestedAction::ChooseDirectory,
        )
    })?;
    let destination_parent = selection.path.parent().ok_or_else(|| {
        CommandErrorDto::recoverable(
            "data_directory_invalid",
            "The selected data directory has no valid parent folder.",
            SuggestedAction::ChooseDirectory,
        )
    })?;
    let available_bytes = fs2::available_space(destination_parent).map_err(|_| {
        CommandErrorDto::recoverable(
            "data_directory_space_unavailable",
            "Available space could not be determined for the selected directory.",
            SuggestedAction::ChooseDirectory,
        )
    })?;
    let required_bytes = workspace_tree_size(&workspace_root).map_err(|message| {
        CommandErrorDto::recoverable("workspace_size_failed", message, SuggestedAction::Retry)
    })?;
    let protected_backup = application.backup().destination_path;
    let plan = DataDirectoryPlan::inspect(
        &workspace_root,
        &selection.path,
        protected_backup.as_deref(),
        required_bytes,
        DestinationProbe {
            writable: selection.writable,
            available_bytes,
            same_volume: same_volume(&workspace_root, &selection.path),
        },
    )
    .map_err(|error| {
        CommandErrorDto::recoverable(
            "data_directory_preflight_failed",
            error.to_string(),
            SuggestedAction::ChooseDirectory,
        )
    })?;
    let activator = application.workspace_root_activator().ok_or_else(|| {
        CommandErrorDto::recoverable(
            "workspace_pointer_unavailable",
            "The workspace directory setting cannot be persisted.",
            SuggestedAction::ChooseDirectory,
        )
    })?;
    let supervisor = engine.inner().clone();
    let display_name = selection.display_name;
    let submitted = application.jobs().submit_maintenance(
        JobKind::DataDirectoryMigration,
        Duration::from_secs(30),
        move |context| {
            supervisor
                .pause_for_maintenance(Duration::from_secs(10))
                .map_err(|failure| {
                    JobFailure::recoverable("DATA_DIRECTORY_PAUSE_FAILED", failure.message)
                })?;
            if let Err(cancelled) = context.cancellation().checkpoint() {
                let _ = supervisor.resume_after_maintenance();
                return Err(cancelled);
            }
            context.commit_started();
            let migration = migrate_data_directory(&plan, &LocalWorkspaceHealth, &activator);
            let resume = supervisor.resume_after_maintenance();
            migration.map_err(|error| {
                JobFailure::recoverable("DATA_DIRECTORY_MIGRATION_FAILED", error.to_string())
            })?;
            resume.map_err(|failure| {
                JobFailure::recoverable("DATA_DIRECTORY_REOPEN_FAILED", failure.message)
            })?;
            Ok(json!({
                "destinationDisplayName": display_name,
                "sourceRetained": true,
                "byteSize": plan.required_bytes
            }))
        },
    );
    let id = submitted.map_err(map_maintenance_submit_error)?;
    job_snapshot(application.jobs().get(&id))
}

fn decrypt_restore_archive(
    archive: &[u8],
    unlock: &RestoreUnlockDto,
    current_workspace_id: Option<&str>,
) -> CommandResult<Vec<u8>> {
    let backend = AuditedAgeBackend::new();
    if let Some(recovery_key) = unlock
        .recovery_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let secret = SecretBytes::new(recovery_key.as_bytes().to_vec()).map_err(|error| {
            CommandErrorDto::recoverable(
                "restore_recovery_key_invalid",
                error.to_string(),
                SuggestedAction::Restore,
            )
        })?;
        return UnlockMaterial::Identity(&secret)
            .decrypt(archive, &backend)
            .map_err(|error| {
                CommandErrorDto::recoverable(
                    "restore_decryption_failed",
                    error.to_string(),
                    SuggestedAction::Restore,
                )
            });
    }
    if let Some(passphrase) = unlock
        .passphrase
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        let secret = SecretBytes::new(passphrase.as_bytes().to_vec()).map_err(|error| {
            CommandErrorDto::recoverable(
                "restore_passphrase_invalid",
                error.to_string(),
                SuggestedAction::Restore,
            )
        })?;
        return UnlockMaterial::Passphrase(&secret)
            .decrypt(archive, &backend)
            .map_err(|error| {
                CommandErrorDto::recoverable(
                    "restore_decryption_failed",
                    error.to_string(),
                    SuggestedAction::Restore,
                )
            });
    }
    let Some(current_workspace_id) = current_workspace_id else {
        return Err(CommandErrorDto::recoverable(
            "restore_unlock_required",
            "This encrypted backup requires its passphrase or exported age recovery key.",
            SuggestedAction::Restore,
        ));
    };
    let provider = KeyringAgeIdentityProvider::production();
    UnlockMaterial::Keychain {
        key_id: &backup_key_id(current_workspace_id),
        provider: &provider,
    }
    .decrypt(archive, &backend)
    .map_err(|_| {
        CommandErrorDto::recoverable(
            "restore_unlock_required",
            "This encrypted backup requires its passphrase or exported age recovery key.",
            SuggestedAction::Restore,
        )
    })
}

struct SqliteRestorePreflight;

impl DatabasePreflight for SqliteRestorePreflight {
    fn schema_version(&self, database: &Path) -> Result<u32, String> {
        let connection = rusqlite::Connection::open(database).map_err(|error| error.to_string())?;
        connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| error.to_string())
    }

    fn migrate_in_place(&self, _database: &Path, from: u32, to: u32) -> Result<(), String> {
        if from == to {
            Ok(())
        } else {
            Err(format!(
                "no supported restore migration path exists from schema {from} to {to}"
            ))
        }
    }

    fn validate(&self, database: &Path) -> Result<(), String> {
        validate_workspace_database(database, None)
    }
}

struct LocalWorkspaceHealth;

impl WorkspaceHealth for LocalWorkspaceHealth {
    fn validate_workspace(&self, workspace: &Path) -> Result<(), String> {
        let identity: serde_json::Value = serde_json::from_slice(
            &fs::read(workspace.join("workspace.v1.json")).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        let workspace_id = identity
            .get("workspaceId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "workspace identity has no workspaceId".to_owned())?;
        let principal_id = identity
            .get("localPrincipalId")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "workspace identity has no localPrincipalId".to_owned())?;
        for id in [workspace_id, principal_id] {
            let parsed = Uuid::parse_str(id).map_err(|_| "workspace identity is invalid")?;
            if parsed.get_version() != Some(Version::SortRand) {
                return Err("workspace identity is not UUIDv7".into());
            }
        }
        validate_workspace_database(
            &workspace.join("workspace.sqlite3"),
            Some((workspace_id, principal_id)),
        )
    }
}

fn validate_workspace_database(
    database: &Path,
    expected_identity: Option<(&str, &str)>,
) -> Result<(), String> {
    let connection = rusqlite::Connection::open(database).map_err(|error| error.to_string())?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(|error| error.to_string())?;
    let quick: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if quick != "ok" {
        return Err("SQLite quick_check failed".into());
    }
    let foreign_key_errors: i64 = connection
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .map_err(|error| error.to_string())?;
    if foreign_key_errors != 0 {
        return Err("SQLite foreign-key validation failed".into());
    }
    let schema: u32 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    if schema != LATEST_SCHEMA_VERSION {
        return Err(format!("unsupported workspace schema {schema}"));
    }
    if let Some((workspace_id, principal_id)) = expected_identity {
        let stored_workspace: String = connection
            .query_row(
                "SELECT value FROM workspace_meta WHERE key = 'workspace_id'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        let stored_principal: String = connection
            .query_row(
                "SELECT value FROM workspace_meta WHERE key = 'local_principal_id'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if stored_workspace != workspace_id || stored_principal != principal_id {
            return Err("workspace identity and database metadata do not match".into());
        }
    }
    Ok(())
}

fn workspace_tree_size(root: &Path) -> Result<u64, String> {
    let mut total = 0_u64;
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        for entry in entries {
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink() {
                return Err(
                    "The workspace contains a symbolic link and cannot be migrated.".into(),
                );
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                total = total
                    .checked_add(metadata.len())
                    .ok_or_else(|| "Workspace size overflowed.".to_owned())?;
            } else {
                return Err("The workspace contains an unsupported file type.".into());
            }
        }
    }
    Ok(total)
}

fn same_volume(left: &Path, right: &Path) -> bool {
    left.components().next() == right.components().next()
}

fn probe_directory(path: PathBuf) -> CommandResult<(PathBuf, bool, Option<u64>)> {
    if !path.is_dir() {
        return Err(CommandErrorDto::recoverable(
            "directory_unavailable",
            "The selected folder is unavailable.",
            SuggestedAction::ChooseDirectory,
        ));
    }
    let probe = path.join(format!(".testpapers-write-probe-{}", Uuid::now_v7()));
    let writable = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
        .and_then(|file| file.sync_all())
        .is_ok();
    let _ = fs::remove_file(probe);
    let available = fs2::available_space(&path).ok();
    Ok((path, writable, available))
}

fn backup_key_id(workspace_id: &str) -> String {
    format!("workspace-{workspace_id}")
}

fn create_workspace_archive(
    workspace: &WorkspaceRuntime,
    app_version: &str,
    kind: BackupKind,
    context: &JobContext,
) -> Result<Vec<u8>, JobFailure> {
    let staging = JobStaging::create(&workspace.root, "backup", &context.id())?;
    context.update_progress("snapshotting", 0, Some(3));
    context.cancellation().checkpoint()?;
    let database =
        WorkspaceDatabaseSnapshot::new(Arc::clone(&workspace.store), workspace.root.clone());
    let request = BackupCreateRequest::with_defaults(
        staging.path().to_path_buf(),
        workspace.identity.workspace_id.to_string(),
        app_version.to_owned(),
        LATEST_SCHEMA_VERSION,
        now_micros(),
        kind,
    );
    let archive = create_consistent_backup(&request, &database)
        .map_err(|error| JobFailure::recoverable("BACKUP_SNAPSHOT_FAILED", error.to_string()))?;
    context.update_progress("archiving", 1, Some(3));
    Ok(archive)
}

struct WorkspaceDatabaseSnapshot {
    store: Arc<LocalDataStore>,
    workspace_root: PathBuf,
    inventory: std::sync::Mutex<Option<BackupInventory>>,
}

impl WorkspaceDatabaseSnapshot {
    fn new(store: Arc<LocalDataStore>, workspace_root: PathBuf) -> Self {
        Self {
            store,
            workspace_root,
            inventory: std::sync::Mutex::new(None),
        }
    }
}

impl ConsistentDatabaseSnapshot for WorkspaceDatabaseSnapshot {
    fn snapshot_to(&self, destination: &Path) -> Result<(), String> {
        let inventory = self
            .store
            .snapshot_to(destination)
            .map_err(|error| error.to_string())?;
        *self
            .inventory
            .lock()
            .map_err(|_| "backup inventory lock is unavailable".to_owned())? = Some(inventory);
        Ok(())
    }

    fn inventory(
        &self,
        _snapshot_database: &Path,
    ) -> Result<(BTreeMap<String, u64>, Vec<BackupPayloadSource>), String> {
        let inventory = self
            .inventory
            .lock()
            .map_err(|_| "backup inventory lock is unavailable".to_owned())?
            .clone()
            .ok_or_else(|| "database snapshot inventory is unavailable".to_owned())?;
        let mut payloads = inventory
            .blobs
            .into_iter()
            .map(|blob| BackupPayloadSource {
                archive_path: blob.archive_relative_path,
                source_path: blob.source_path,
                role: BackupFileRole::Blob,
            })
            .collect::<Vec<_>>();
        let identity = self.workspace_root.join("workspace.v1.json");
        if !identity.is_file() {
            return Err("workspace identity metadata is missing".into());
        }
        payloads.push(BackupPayloadSource {
            archive_path: "workspace.v1.json".into(),
            source_path: identity,
            role: BackupFileRole::WorkspaceMetadata,
        });
        collect_template_payloads(&self.workspace_root.join("templates"), &mut payloads)?;
        Ok((inventory.live_entity_counts, payloads))
    }
}

fn collect_template_payloads(
    root: &Path,
    payloads: &mut Vec<BackupPayloadSource>,
) -> Result<(), String> {
    if !root.exists() {
        return Ok(());
    }
    let canonical_root = root.canonicalize().map_err(|error| error.to_string())?;
    let mut pending = vec![canonical_root.clone()];
    while let Some(directory) = pending.pop() {
        let mut entries = fs::read_dir(&directory)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let metadata = fs::symlink_metadata(entry.path()).map_err(|error| error.to_string())?;
            if metadata.file_type().is_symlink() {
                return Err("template inventory does not follow symbolic links".into());
            }
            if metadata.is_dir() {
                pending.push(entry.path());
            } else if metadata.is_file() {
                let canonical = entry
                    .path()
                    .canonicalize()
                    .map_err(|error| error.to_string())?;
                if !canonical.starts_with(&canonical_root) {
                    return Err("template inventory escaped its root".into());
                }
                let relative = canonical
                    .strip_prefix(&canonical_root)
                    .map_err(|error| error.to_string())?
                    .components()
                    .map(|component| component.as_os_str().to_str())
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| "template filename is not valid UTF-8".to_owned())?
                    .join("/");
                payloads.push(BackupPayloadSource {
                    archive_path: format!("templates/{relative}"),
                    source_path: canonical,
                    role: BackupFileRole::Template,
                });
            } else {
                return Err("template inventory contains an unsupported file".into());
            }
        }
    }
    Ok(())
}

struct JobGenerationObserver(JobContext);

impl GenerationObserver for JobGenerationObserver {
    fn is_cancelled(&self) -> bool {
        self.0.cancellation().is_cancelled()
    }

    fn progress(&self, completed_generations: usize, total_generations: usize) {
        self.0.update_progress(
            "optimizing",
            completed_generations as u64,
            Some(total_generations as u64),
        );
    }
}

fn load_generation_candidates(
    store: &LocalDataStore,
    request: &GenerationRequest,
    context: &JobContext,
) -> Result<Vec<QuestionSnapshot>, JobFailure> {
    let mut candidates = Vec::new();
    let mut cursor = None;
    loop {
        context.cancellation().checkpoint()?;
        let page = store
            .search_questions(QuestionSearch {
                query: None,
                subjects: request.subjects.clone(),
                tags: Vec::new(),
                types: Vec::new(),
                difficulties: Vec::new(),
                deleted: DeletedFilter::Exclude,
                cursor,
                page_size: Some(100),
            })
            .map_err(|error| job_local_data_failure("QUESTION_SEARCH_FAILED", error))?;
        for question in page.items {
            candidates.push(local_question_snapshot(store, question)?);
        }
        context.update_progress("loadingCandidates", candidates.len() as u64, None);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(candidates)
}

fn local_question_snapshot(
    store: &LocalDataStore,
    question: QuestionRecord,
) -> Result<QuestionSnapshot, JobFailure> {
    let attachments = store
        .list_question_attachments(&question.id, false)
        .map_err(|error| job_local_data_failure("ATTACHMENT_LOOKUP_FAILED", error))?
        .into_iter()
        .map(|attachment| AttachmentSnapshot {
            id: attachment.id,
            blob_hash: attachment.blob_hash,
            media_type: attachment.media_type,
            filename: attachment.file_name,
            caption: attachment.caption,
        })
        .collect();
    let answer = match question.content.answer {
        serde_json::Value::String(value) => AnswerSnapshot::Text(value),
        serde_json::Value::Array(values) => AnswerSnapshot::Multiple(
            values
                .into_iter()
                .map(|value| {
                    value.as_str().map(str::to_owned).ok_or_else(|| {
                        JobFailure::fatal(
                            "QUESTION_ANSWER_INVALID",
                            "A question answer is not a string array.",
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        _ => {
            return Err(JobFailure::fatal(
                "QUESTION_ANSWER_INVALID",
                "A question answer is not a supported local value.",
            ))
        }
    };
    Ok(QuestionSnapshot {
        id: question.id,
        version: u64::try_from(question.version).map_err(|_| {
            JobFailure::fatal("QUESTION_VERSION_INVALID", "A question version is invalid.")
        })?,
        content_hash: question.content_hash,
        question_type: match question.content.question_type {
            crate::local_data::QuestionType::SingleChoice => PaperQuestionType::SingleChoice,
            crate::local_data::QuestionType::MultipleChoice => PaperQuestionType::MultipleChoice,
            crate::local_data::QuestionType::TrueFalse => PaperQuestionType::TrueFalse,
            crate::local_data::QuestionType::Blank => PaperQuestionType::Blank,
            crate::local_data::QuestionType::ShortAnswer => PaperQuestionType::ShortAnswer,
            crate::local_data::QuestionType::Essay => PaperQuestionType::Essay,
        },
        subjects: question.content.subjects,
        difficulty: match question.content.difficulty {
            crate::local_data::Difficulty::Easy => PaperDifficulty::Easy,
            crate::local_data::Difficulty::Medium => PaperDifficulty::Medium,
            crate::local_data::Difficulty::Hard => PaperDifficulty::Hard,
        },
        tags: question.content.tags,
        text: question.content.text,
        options: question.content.options,
        answer,
        has_latex: question.content.has_latex,
        source: question.content.source,
        essay_blank_space: question
            .content
            .essay_blank_space
            .map(|space| {
                u8::try_from(space.lines)
                    .map(|lines| EssayBlankSpace { lines })
                    .map_err(|_| {
                        JobFailure::fatal(
                            "QUESTION_LAYOUT_INVALID",
                            "Essay blank-space lines exceed the paper format.",
                        )
                    })
            })
            .transpose()?,
        score_weight: question.content.score_weight,
        attachments,
    })
}

fn map_generation_error(error: GenerationError) -> JobFailure {
    match error {
        GenerationError::Cancelled => JobFailure::Cancelled,
        GenerationError::Invalid(message) => {
            JobFailure::recoverable("GENERATION_INPUT_INVALID", message)
        }
        GenerationError::DuplicateCandidate(_) => JobFailure::fatal(
            "GENERATION_CANDIDATE_DUPLICATE",
            "The local candidate set contains a duplicate stable question identifier.",
        ),
        GenerationError::Insufficient(diagnostics) => JobFailure::recoverable(
            "GENERATION_INSUFFICIENT_QUESTIONS",
            serde_json::to_string(&diagnostics)
                .unwrap_or_else(|_| "Not enough matching questions.".into()),
        ),
    }
}

fn job_local_data_failure(code: &'static str, error: LocalDataError) -> JobFailure {
    let mapped = map_local_data_error(error);
    JobFailure::recoverable(code, mapped.safe_message())
}

struct StoreAttachmentSource(Arc<LocalDataStore>);

impl AttachmentSource for StoreAttachmentSource {
    fn load_blob(&self, sha256: &str) -> Result<Vec<u8>, String> {
        let inventory = self
            .0
            .backup_inventory()
            .map_err(|error| error.to_string())?;
        let source = inventory
            .blobs
            .into_iter()
            .find(|blob| blob.blob_hash == sha256)
            .ok_or_else(|| format!("attachment blob is unavailable: {sha256}"))?;
        fs::read(source.source_path).map_err(|error| error.to_string())
    }
}

fn build_pdf_artifact(
    paper: &PaperSnapshot,
    options: ExportOptions,
    attachments: &dyn AttachmentSource,
    workspace_root: &Path,
    resource_root: &Path,
    context: &JobContext,
) -> Result<ExportArtifact, JobFailure> {
    let tex = build_tex(paper, options, attachments)
        .map_err(|error| JobFailure::recoverable("PDF_TEX_FAILED", error.to_string()))?;
    let staging = JobStaging::create(workspace_root, "pdf-export", &context.id())?;
    let tex_name = "paper.tex";
    fs::write(staging.path().join(tex_name), &tex.bytes)
        .map_err(|_| JobFailure::recoverable("PDF_STAGING_FAILED", "PDF staging failed."))?;
    for companion in &tex.companions {
        let destination = safe_companion_path(staging.path(), &companion.relative_path)
            .map_err(|message| JobFailure::fatal("PDF_COMPANION_INVALID", message))?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                JobFailure::recoverable("PDF_STAGING_FAILED", "PDF staging failed.")
            })?;
        }
        fs::write(destination, &companion.bytes)
            .map_err(|_| JobFailure::recoverable("PDF_STAGING_FAILED", "PDF staging failed."))?;
    }
    let runner = configured_tectonic(resource_root)?;
    let pdf_path = runner
        .compile(staging.path(), tex_name, context.cancellation())
        .map_err(|error| JobFailure::recoverable("PDF_ENGINE_UNAVAILABLE", error.to_string()))?;
    let bytes = fs::read(pdf_path).map_err(|_| {
        JobFailure::recoverable("PDF_READ_FAILED", "The generated PDF is unreadable.")
    })?;
    Ok(ExportArtifact {
        filename: export_suggested_name(&paper.title, ExportFormat::Pdf),
        media_type: "application/pdf",
        bytes,
        companions: Vec::new(),
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TectonicRelease {
    binary_sha256: String,
    bundle_sha256: String,
}

fn configured_tectonic(resource_root: &Path) -> Result<BundledTectonic, JobFailure> {
    use crate::workspace_features::hash::Sha256Digest;

    let directory = resource_root.join("tectonic");
    let release: TectonicRelease =
        serde_json::from_slice(&fs::read(directory.join("release.v1.json")).map_err(|_| {
            JobFailure::recoverable(
                "PDF_ENGINE_UNAVAILABLE",
                "The bundled offline PDF engine is not installed.",
            )
        })?)
        .map_err(|_| {
            JobFailure::fatal(
                "PDF_ENGINE_MANIFEST_INVALID",
                "The bundled PDF engine manifest is invalid.",
            )
        })?;
    let binary_name = if cfg!(windows) {
        "tectonic.exe"
    } else {
        "tectonic"
    };
    let mut runner = BundledTectonic::new(
        directory.join(binary_name),
        directory.join("testpapers-bundle.ttb"),
    );
    runner.expected_binary_sha256 =
        Some(Sha256Digest::from_hex(&release.binary_sha256).map_err(|_| {
            JobFailure::fatal(
                "PDF_ENGINE_MANIFEST_INVALID",
                "The bundled PDF engine checksum is invalid.",
            )
        })?);
    runner.expected_bundle_sha256 =
        Some(Sha256Digest::from_hex(&release.bundle_sha256).map_err(|_| {
            JobFailure::fatal(
                "PDF_ENGINE_MANIFEST_INVALID",
                "The bundled PDF bundle checksum is invalid.",
            )
        })?);
    Ok(runner)
}

fn publish_export_artifact(artifact: &ExportArtifact, target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "The export destination has no parent folder.".to_owned())?;
    if !parent.is_dir() || target.file_name().is_none() {
        return Err("The export destination is unavailable.".into());
    }
    for companion in &artifact.companions {
        let destination = safe_companion_path(parent, &companion.relative_path)?;
        if let Some(directory) = destination.parent() {
            fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        }
        publish_atomic(&companion.bytes, &destination)?;
    }
    publish_atomic(&artifact.bytes, target)
}

fn publish_atomic(bytes: &[u8], target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| "The destination has no parent folder.".to_owned())?;
    let temporary = parent.join(format!(
        ".{}.{}.partial",
        target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("testpapers-export"),
        Uuid::now_v7()
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        if target.exists() {
            fs::remove_file(target).map_err(|error| error.to_string())?;
        }
        fs::rename(&temporary, target).map_err(|error| error.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn safe_companion_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("An export companion path is unsafe.".into());
    }
    Ok(root.join(path))
}

fn export_suggested_name(title: &str, format: ExportFormat) -> String {
    let extension = match format {
        ExportFormat::Docx => "docx",
        ExportFormat::Tex => "tex",
        ExportFormat::Pdf => "pdf",
    };
    let cleaned = title
        .chars()
        .map(|character| {
            if matches!(
                character,
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            ) || character.is_control()
            {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let basename = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let basename = if basename.is_empty() {
        "examination-paper".into()
    } else {
        basename.chars().take(80).collect::<String>()
    };
    format!("{basename}.{extension}")
}

fn now_micros() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
        .min(i64::MAX as u128) as i64
}

struct JobStaging(PathBuf);

impl JobStaging {
    fn create(root: &Path, kind: &str, id: &JobId) -> Result<Self, JobFailure> {
        let path = root.join(".jobs").join(format!("{kind}-{}", id.0));
        fs::create_dir_all(path.parent().expect("job staging has a parent")).map_err(|_| {
            JobFailure::recoverable("JOB_STAGING_FAILED", "Job staging is unavailable.")
        })?;
        fs::create_dir(&path).map_err(|_| {
            JobFailure::recoverable("JOB_STAGING_FAILED", "Job staging is unavailable.")
        })?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for JobStaging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tauri::command]
pub(crate) fn get_job(
    application: State<'_, LocalWorkspaceApplication>,
    id: String,
) -> CommandResult<JobSummaryDto> {
    let id = parse_job_id(&id)?;
    job_snapshot(application.jobs().get(&id))
}

#[tauri::command]
pub(crate) fn cancel_job(
    application: State<'_, LocalWorkspaceApplication>,
    id: String,
) -> CommandResult<JobSummaryDto> {
    let id = parse_job_id(&id)?;
    application.jobs().cancel(&id).map_err(map_cancel_error)?;
    job_snapshot(application.jobs().get(&id))
}

fn parse_job_id(value: &str) -> CommandResult<JobId> {
    let id = Uuid::parse_str(value).map_err(|_| {
        CommandErrorDto::fatal(
            "invalid_job_id",
            "The Local Engine job identifier is invalid.",
            SuggestedAction::ContactSupport,
        )
    })?;
    if id.get_version() != Some(Version::SortRand) {
        return Err(CommandErrorDto::fatal(
            "invalid_job_id",
            "The Local Engine job identifier is invalid.",
            SuggestedAction::ContactSupport,
        ));
    }
    Ok(JobId(id))
}

fn job_snapshot(snapshot: Option<JobSnapshot>) -> CommandResult<JobSummaryDto> {
    snapshot.map(Into::into).ok_or_else(|| {
        CommandErrorDto::recoverable(
            "job_not_found",
            "The Local Engine job is no longer available.",
            SuggestedAction::Retry,
        )
    })
}

fn map_submit_error(error: SubmitError) -> CommandErrorDto {
    let (code, message) = match error {
        SubmitError::Maintenance => (
            "workspace_maintenance",
            "The workspace is in exclusive maintenance mode.",
        ),
        SubmitError::ShuttingDown => (
            "local_engine_stopping",
            "The Local Engine is stopping and cannot accept another job.",
        ),
        SubmitError::QueueUnavailable => (
            "job_queue_unavailable",
            "The Local Engine background queue is unavailable.",
        ),
        SubmitError::ExclusiveMaintenanceRequired => (
            "exclusive_maintenance_required",
            "This operation requires exclusive workspace maintenance.",
        ),
    };
    CommandErrorDto::recoverable(code, message, SuggestedAction::Retry)
}

fn map_maintenance_submit_error(error: MaintenanceSubmitError) -> CommandErrorDto {
    let (code, message, action) = match error {
        MaintenanceSubmitError::RegularQueueKind => (
            "maintenance_job_kind_invalid",
            "This operation is not an exclusive workspace maintenance job.",
            SuggestedAction::ContactSupport,
        ),
        MaintenanceSubmitError::Maintenance(MaintenanceError::AlreadyActive) => (
            "workspace_maintenance",
            "Another exclusive workspace maintenance operation is already active.",
            SuggestedAction::Retry,
        ),
        MaintenanceSubmitError::Maintenance(MaintenanceError::ShuttingDown) => (
            "local_engine_stopping",
            "The Local Engine is stopping and cannot start workspace maintenance.",
            SuggestedAction::RestartApp,
        ),
        MaintenanceSubmitError::Maintenance(MaintenanceError::TimedOut) => (
            "maintenance_wait_timeout",
            "Background jobs did not stop before the maintenance deadline.",
            SuggestedAction::Retry,
        ),
        MaintenanceSubmitError::WorkerUnavailable => (
            "maintenance_worker_unavailable",
            "The Local Engine maintenance worker is unavailable.",
            SuggestedAction::Retry,
        ),
    };
    CommandErrorDto::recoverable(code, message, action)
}

fn map_cancel_error(error: CancelError) -> CommandErrorDto {
    let (code, message) = match error {
        CancelError::UnknownJob => ("job_not_found", "The Local Engine job was not found."),
        CancelError::AlreadyFinished => {
            ("job_finished", "The Local Engine job has already finished.")
        }
        CancelError::CommitStarted => (
            "job_commit_started",
            "The job has reached its commit point and can no longer be cancelled.",
        ),
    };
    CommandErrorDto::recoverable(code, message, SuggestedAction::Retry)
}

impl std::fmt::Display for CommandErrorDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Local Engine command failed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RetentionSnapshot {
        metadata: PathBuf,
    }

    impl ConsistentDatabaseSnapshot for RetentionSnapshot {
        fn snapshot_to(&self, destination: &Path) -> Result<(), String> {
            fs::write(destination, b"sqlite").map_err(|error| error.to_string())
        }

        fn inventory(
            &self,
            _snapshot_database: &Path,
        ) -> Result<(BTreeMap<String, u64>, Vec<BackupPayloadSource>), String> {
            Ok((
                BTreeMap::new(),
                vec![BackupPayloadSource {
                    archive_path: "workspace.v1.json".into(),
                    source_path: self.metadata.clone(),
                    role: BackupFileRole::WorkspaceMetadata,
                }],
            ))
        }
    }

    fn retention_archive(
        root: &Path,
        workspace_id: &str,
        created_at_micros: i64,
        kind: BackupKind,
        nonce: &str,
    ) -> Vec<u8> {
        let staging = root.join(format!("staging-{nonce}"));
        fs::create_dir(&staging).unwrap();
        let metadata = root.join(format!("identity-{nonce}.json"));
        fs::write(&metadata, b"{}").unwrap();
        create_consistent_backup(
            &BackupCreateRequest::with_defaults(
                staging,
                workspace_id.into(),
                "1.0.0".into(),
                1,
                created_at_micros,
                kind,
            ),
            &RetentionSnapshot { metadata },
        )
        .unwrap()
    }

    #[test]
    fn accepts_only_uuid_v7_job_ids() {
        assert!(parse_job_id(&Uuid::now_v7().to_string()).is_ok());
        assert!(parse_job_id("550e8400-e29b-41d4-a716-446655440000").is_err());
        assert!(parse_job_id("not-a-job").is_err());
    }

    #[test]
    fn unsafe_local_paths_are_never_serialized_to_the_webview() {
        let error = map_local_data_error(LocalDataError::UnsafePath(
            r"C:\Teachers\Alice\questions.csv".into(),
        ));
        let value = serde_json::to_string(&error).unwrap();
        assert!(!value.contains("Teachers"));
        assert!(!value.contains("questions.csv"));
    }

    #[test]
    fn question_image_types_are_allowlisted_by_extension() {
        assert_eq!(
            question_image_media_type(Path::new("diagram.PNG")),
            Some("image/png")
        );
        assert_eq!(
            question_image_media_type(Path::new("photo.jpeg")),
            Some("image/jpeg")
        );
        assert_eq!(question_image_media_type(Path::new("vector.svg")), None);
        assert_eq!(question_image_media_type(Path::new("image.png.exe")), None);
    }

    #[test]
    fn question_image_content_must_match_the_declared_type() {
        let mut png = std::io::Cursor::new(b"\x89PNG\r\n\x1a\ncontent".to_vec());
        assert!(verify_question_image_signature(&mut png, "image/png").unwrap());
        assert_eq!(png.position(), 0);

        let mut renamed = std::io::Cursor::new(b"not an image".to_vec());
        assert!(!verify_question_image_signature(&mut renamed, "image/png").unwrap());
    }

    #[test]
    fn retention_deletes_only_expired_verified_automatic_backups() {
        const DAY: i64 = 24 * 60 * 60 * 1_000_000;
        let temporary = tempfile::tempdir().unwrap();
        let destination = temporary.path().join("backups");
        fs::create_dir(&destination).unwrap();
        let workspace_id = "018f0000-0000-7000-8000-000000000001";
        let expired = destination.join("TestPapers-Automatic-expired.tpbackup");
        let recent = destination.join("TestPapers-Automatic-recent.tpbackup");
        let manual = destination.join("TestPapers-Automatic-manual.tpbackup");
        let invalid = destination.join("TestPapers-Automatic-invalid.tpbackup");
        fs::write(
            &expired,
            retention_archive(
                temporary.path(),
                workspace_id,
                DAY,
                BackupKind::Automatic,
                "expired",
            ),
        )
        .unwrap();
        fs::write(
            &recent,
            retention_archive(
                temporary.path(),
                workspace_id,
                20 * DAY,
                BackupKind::Automatic,
                "recent",
            ),
        )
        .unwrap();
        fs::write(
            &manual,
            retention_archive(
                temporary.path(),
                workspace_id,
                DAY,
                BackupKind::Manual,
                "manual",
            ),
        )
        .unwrap();
        fs::write(&invalid, b"not a backup").unwrap();

        prune_automatic_backups(&destination, workspace_id, 30, 40 * DAY);

        assert!(!expired.exists());
        assert!(recent.exists());
        assert!(manual.exists());
        assert!(invalid.exists());
    }
}
