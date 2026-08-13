use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    sync::{mpsc, Arc, Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    domain::{EngineFailure, EngineState, SuggestedAction},
    infrastructure::workspace::{WorkspaceBootstrap, WorkspaceIdentity, WorkspaceLease},
    local_data::{LocalDataError, LocalDataStore, StoreConfig},
};

const DEFAULT_RESTART_DELAYS: [Duration; 3] = [
    Duration::from_millis(250),
    Duration::from_secs(1),
    Duration::from_secs(4),
];

type StateListener = Arc<dyn Fn(EngineSnapshot) + Send + Sync>;
type WorkspaceOpener = Arc<dyn Fn() -> Result<WorkspaceLease, EngineFailure> + Send + Sync>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EngineSnapshot {
    pub(crate) state: EngineState,
    pub(crate) generation: u64,
    pub(crate) workspace_id: Option<String>,
    pub(crate) database_available: bool,
    pub(crate) maintenance_mode: bool,
    pub(crate) last_error: Option<EngineFailure>,
}

#[derive(Clone)]
pub(crate) struct WorkspaceRuntime {
    #[allow(dead_code)] // Export and backup services consume the workspace root in CLE-27/28.
    pub(crate) root: PathBuf,
    pub(crate) identity: WorkspaceIdentity,
    pub(crate) store: Arc<LocalDataStore>,
}

struct WorkerTask {
    handle: JoinHandle<()>,
    finished: mpsc::Receiver<()>,
}

struct RuntimeState {
    snapshot: EngineSnapshot,
    workspace: Option<WorkspaceRuntime>,
    stop_requested: bool,
    maintenance_reopen: bool,
    worker_active: bool,
    worker: Option<WorkerTask>,
}

struct SupervisorInner {
    opener: WorkspaceOpener,
    listener: StateListener,
    restart_delays: Vec<Duration>,
    runtime: Mutex<RuntimeState>,
    maintenance_operation: Mutex<()>,
    wake_worker: Condvar,
}

#[derive(Clone)]
pub(crate) struct EngineSupervisor {
    inner: Arc<SupervisorInner>,
}

impl EngineSupervisor {
    pub(crate) fn new(bootstrap: WorkspaceBootstrap, listener: StateListener) -> Self {
        let opener: WorkspaceOpener = Arc::new(move || bootstrap.open());
        Self::with_opener(opener, listener, DEFAULT_RESTART_DELAYS.to_vec())
    }

    fn with_opener(
        opener: WorkspaceOpener,
        listener: StateListener,
        restart_delays: Vec<Duration>,
    ) -> Self {
        Self {
            inner: Arc::new(SupervisorInner {
                opener,
                listener,
                restart_delays,
                runtime: Mutex::new(RuntimeState {
                    snapshot: EngineSnapshot {
                        state: EngineState::Starting,
                        generation: 0,
                        workspace_id: None,
                        database_available: false,
                        maintenance_mode: false,
                        last_error: None,
                    },
                    workspace: None,
                    stop_requested: false,
                    maintenance_reopen: false,
                    worker_active: false,
                    worker: None,
                }),
                maintenance_operation: Mutex::new(()),
                wake_worker: Condvar::new(),
            }),
        }
    }

    pub(crate) fn start(&self) -> EngineSnapshot {
        lock_runtime(&self.inner).maintenance_reopen = false;
        match self.spawn_worker(EngineState::Starting) {
            Ok(snapshot) => snapshot,
            Err(_) => self.snapshot(),
        }
    }

    pub(crate) fn retry(&self) -> Result<EngineSnapshot, EngineFailure> {
        let snapshot = self.snapshot();
        if snapshot.state != EngineState::Degraded {
            return Err(EngineFailure::retry_not_available(snapshot.state));
        }
        if let Some(failure) = snapshot.last_error.filter(|failure| !failure.recoverable) {
            return Err(failure);
        }
        lock_runtime(&self.inner).maintenance_reopen = false;
        self.spawn_worker(EngineState::Recovering)
    }

    pub(crate) fn snapshot(&self) -> EngineSnapshot {
        lock_runtime(&self.inner).snapshot.clone()
    }

    #[allow(dead_code)] // CLE-25 consumes the initialized workspace behind this lifecycle boundary.
    pub(crate) fn workspace(&self) -> Option<WorkspaceRuntime> {
        let runtime = lock_runtime(&self.inner);
        if runtime.snapshot.state != EngineState::Ready
            || !runtime.snapshot.database_available
            || runtime.snapshot.maintenance_mode
        {
            return None;
        }
        runtime.workspace.clone()
    }

    #[allow(dead_code)] // CLE-28 restore and directory swaps call this through their coordinator.
    pub(crate) fn pause_for_maintenance(
        &self,
        timeout: Duration,
    ) -> Result<EngineSnapshot, EngineFailure> {
        let _operation = self
            .inner
            .maintenance_operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let deadline = Instant::now() + timeout;
        let snapshot = {
            let mut runtime = lock_runtime(&self.inner);
            if runtime.snapshot.state == EngineState::Stopping {
                return Err(EngineFailure::new(
                    "maintenance_not_available",
                    "Workspace maintenance cannot start while the Local Engine is stopping",
                    false,
                    SuggestedAction::RestartApp,
                ));
            }
            runtime.snapshot.maintenance_mode = true;
            runtime.snapshot.database_available = false;
            runtime.stop_requested = true;
            runtime.maintenance_reopen = false;
            self.inner.wake_worker.notify_all();
            runtime.snapshot.clone()
        };
        notify(&self.inner, snapshot.clone());

        let worker = loop {
            let mut runtime = lock_runtime(&self.inner);
            if let Some(worker) = runtime.worker.take() {
                break Some(worker);
            }
            if !runtime.worker_active {
                break None;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                drop(runtime);
                return Err(self.maintenance_timeout());
            }
            let (next, wait) = self
                .inner
                .wake_worker
                .wait_timeout(runtime, remaining)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime = next;
            if wait.timed_out() && runtime.worker.is_none() && runtime.worker_active {
                drop(runtime);
                return Err(self.maintenance_timeout());
            }
        };

        if let Some(worker) = worker {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match worker.finished.recv_timeout(remaining) {
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = worker.handle.join();
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let mut runtime = lock_runtime(&self.inner);
                    runtime.worker = Some(worker);
                    drop(runtime);
                    return Err(self.maintenance_timeout());
                }
            }
        }

        // A worker panic releases its stack-held workspace lease, but it may leave the
        // managed store behind. Close that final handle before declaring maintenance safe.
        let orphaned_workspace = {
            let mut runtime = lock_runtime(&self.inner);
            runtime.worker_active = false;
            runtime.workspace.take()
        };
        if let Some(workspace) = orphaned_workspace {
            while Arc::strong_count(&workspace.store) > 1 {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    lock_runtime(&self.inner).workspace = Some(workspace);
                    return Err(self.maintenance_timeout());
                }
                thread::sleep(remaining.min(Duration::from_millis(5)));
            }
            let _ = workspace.store.checkpoint();
            drop(workspace);
        }

        let snapshot = {
            let mut runtime = lock_runtime(&self.inner);
            if runtime
                .snapshot
                .last_error
                .as_ref()
                .is_some_and(|failure| failure.code == "maintenance_pause_timeout")
            {
                runtime.snapshot.last_error = None;
            }
            runtime.snapshot.clone()
        };
        Ok(snapshot)
    }

    #[allow(dead_code)] // CLE-28 resumes after its atomic restore or directory swap.
    pub(crate) fn resume_after_maintenance(&self) -> Result<EngineSnapshot, EngineFailure> {
        let _operation = self
            .inner
            .maintenance_operation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let snapshot = self.snapshot();
        if snapshot.state == EngineState::Stopping {
            return Err(EngineFailure::new(
                "maintenance_not_available",
                "The Local Engine cannot resume while it is stopping",
                false,
                SuggestedAction::RestartApp,
            ));
        }
        if !snapshot.maintenance_mode {
            return Err(EngineFailure::new(
                "maintenance_not_active",
                "Workspace maintenance is not active",
                false,
                SuggestedAction::ContactSupport,
            ));
        }
        if lock_runtime(&self.inner).worker_active {
            return Err(EngineFailure::new(
                "maintenance_release_pending",
                "The Local Engine is still releasing the workspace",
                true,
                SuggestedAction::Retry,
            ));
        }
        lock_runtime(&self.inner).maintenance_reopen = true;
        self.spawn_worker(EngineState::Recovering)
    }

    fn maintenance_timeout(&self) -> EngineFailure {
        let failure = EngineFailure::new(
            "maintenance_pause_timeout",
            "The Local Engine could not release the workspace before the maintenance timeout",
            true,
            SuggestedAction::Retry,
        );
        let snapshot = {
            let mut runtime = lock_runtime(&self.inner);
            runtime.snapshot.last_error = Some(failure.clone());
            runtime.snapshot.clone()
        };
        notify(&self.inner, snapshot);
        failure
    }

    pub(crate) fn shutdown(&self, timeout: Duration) -> bool {
        let (worker, snapshot, changed) = {
            let mut runtime = lock_runtime(&self.inner);
            let changed = runtime.snapshot.state != EngineState::Stopping;
            runtime.snapshot.state = EngineState::Stopping;
            runtime.snapshot.database_available = false;
            runtime.snapshot.maintenance_mode = false;
            runtime.stop_requested = true;
            runtime.maintenance_reopen = false;
            self.inner.wake_worker.notify_all();
            (runtime.worker.take(), runtime.snapshot.clone(), changed)
        };

        if changed {
            notify(&self.inner, snapshot);
        }

        if let Some(worker) = worker {
            if worker.finished.recv_timeout(timeout).is_ok() {
                let _ = worker.handle.join();
            }
        }
        changed
    }

    fn spawn_worker(&self, state: EngineState) -> Result<EngineSnapshot, EngineFailure> {
        let previous = {
            let mut runtime = lock_runtime(&self.inner);
            if runtime.worker_active {
                return Err(EngineFailure::retry_not_available(runtime.snapshot.state));
            }
            if runtime.snapshot.state == EngineState::Stopping {
                return Err(EngineFailure::retry_not_available(EngineState::Stopping));
            }
            runtime.worker.take()
        };
        if let Some(previous) = previous {
            let _ = previous.handle.join();
        }

        let snapshot = {
            let mut runtime = lock_runtime(&self.inner);
            runtime.snapshot.state = state;
            runtime.snapshot.database_available = false;
            runtime.snapshot.last_error = None;
            runtime.workspace = None;
            runtime.stop_requested = false;
            runtime.worker_active = true;
            runtime.snapshot.clone()
        };
        notify(&self.inner, snapshot.clone());

        let inner = Arc::clone(&self.inner);
        let (finished_tx, finished_rx) = mpsc::sync_channel(1);
        let handle = match thread::Builder::new()
            .name("testpapers-local-engine".into())
            .spawn(move || {
                run_worker(&inner);
                let _ = finished_tx.send(());
            }) {
            Ok(handle) => handle,
            Err(_) => {
                let failure = EngineFailure::new(
                    "engine_worker_unavailable",
                    "The Local Engine worker could not be started",
                    true,
                    SuggestedAction::RestartApp,
                );
                let degraded = {
                    let mut runtime = lock_runtime(&self.inner);
                    runtime.worker_active = false;
                    runtime.snapshot.state = EngineState::Degraded;
                    if runtime.maintenance_reopen {
                        runtime.snapshot.maintenance_mode = false;
                    }
                    runtime.maintenance_reopen = false;
                    runtime.snapshot.last_error = Some(failure.clone());
                    self.inner.wake_worker.notify_all();
                    runtime.snapshot.clone()
                };
                notify(&self.inner, degraded);
                return Err(failure);
            }
        };
        lock_runtime(&self.inner).worker = Some(WorkerTask {
            handle,
            finished: finished_rx,
        });
        self.inner.wake_worker.notify_all();
        Ok(snapshot)
    }
}

fn run_worker(inner: &Arc<SupervisorInner>) {
    let attempts = inner.restart_delays.len() + 1;
    let mut final_failure = None;

    for attempt in 0..attempts {
        if attempt > 0 && wait_or_stopping(inner, inner.restart_delays[attempt - 1]) {
            finish_worker(inner);
            return;
        }
        if stopping(inner) {
            finish_worker(inner);
            return;
        }

        let opened = catch_unwind(AssertUnwindSafe(|| {
            let lease = (inner.opener)()?;
            let workspace = open_local_data(&lease)?;
            Ok((lease, workspace))
        }))
        .unwrap_or_else(|_| {
            Err(EngineFailure::new(
                "engine_worker_panicked",
                "The Local Engine stopped unexpectedly while starting",
                true,
                SuggestedAction::Retry,
            ))
        });

        match opened {
            Ok((lease, workspace)) => {
                serve_until_stopped(inner, lease, workspace);
                return;
            }
            Err(failure) => {
                let retryable = failure.recoverable;
                final_failure = Some(failure.clone());
                let snapshot = {
                    let mut runtime = lock_runtime(inner);
                    runtime.snapshot.last_error = Some(failure);
                    runtime.snapshot.clone()
                };
                notify(inner, snapshot);
                if !retryable {
                    break;
                }
            }
        }
    }

    let snapshot = {
        let mut runtime = lock_runtime(inner);
        runtime.worker_active = false;
        runtime.snapshot.state = EngineState::Degraded;
        runtime.snapshot.database_available = false;
        if runtime.maintenance_reopen {
            runtime.snapshot.maintenance_mode = false;
        }
        runtime.maintenance_reopen = false;
        runtime.snapshot.last_error = final_failure;
        inner.wake_worker.notify_all();
        runtime.snapshot.clone()
    };
    notify(inner, snapshot);
}

fn open_local_data(lease: &WorkspaceLease) -> Result<WorkspaceRuntime, EngineFailure> {
    let root = lease.root().to_path_buf();
    let identity = lease.identity().clone();
    let config = StoreConfig {
        database_path: root.join("workspace.sqlite3"),
        blob_root: root.join("blobs"),
        workspace_id: identity.workspace_id.to_string(),
        local_principal_id: identity.local_principal_id.to_string(),
    };
    let store = LocalDataStore::open(config).map_err(map_database_error)?;
    store.verify_integrity().map_err(map_database_error)?;
    Ok(WorkspaceRuntime {
        root,
        identity,
        store: Arc::new(store),
    })
}

fn map_database_error(error: LocalDataError) -> EngineFailure {
    match error {
        LocalDataError::UnsupportedSchema { .. } => EngineFailure::new(
            "database_schema_too_new",
            "This workspace was created by a newer version; upgrade TestPapers Desktop to open it",
            false,
            SuggestedAction::RestartApp,
        ),
        LocalDataError::Corrupt(_)
        | LocalDataError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase,
                ..
            },
            _,
        )) => EngineFailure::new(
            "database_corrupt",
            "The local database failed its integrity checks",
            true,
            SuggestedAction::Restore,
        ),
        LocalDataError::WorkspaceMismatch { .. } => EngineFailure::new(
            "workspace_identity_mismatch",
            "The local database does not belong to this workspace",
            true,
            SuggestedAction::Restore,
        ),
        LocalDataError::UnsafePath(_) => EngineFailure::new(
            "database_path_unsafe",
            "The selected workspace directory is not safe to use",
            true,
            SuggestedAction::ChooseDirectory,
        ),
        _ => EngineFailure::new(
            "database_initialization_failed",
            "The Local Engine could not initialize its database",
            true,
            SuggestedAction::Retry,
        ),
    }
}

fn serve_until_stopped(
    inner: &Arc<SupervisorInner>,
    lease: WorkspaceLease,
    workspace: WorkspaceRuntime,
) {
    let snapshot = {
        let mut runtime = lock_runtime(inner);
        if runtime.stop_requested {
            drop(runtime);
            drop(workspace);
            drop(lease);
            runtime = lock_runtime(inner);
            runtime.worker_active = false;
            inner.wake_worker.notify_all();
            return;
        }
        runtime.snapshot.state = EngineState::Ready;
        runtime.snapshot.generation = runtime.snapshot.generation.saturating_add(1);
        runtime.snapshot.workspace_id = Some(workspace.identity.workspace_id.to_string());
        runtime.snapshot.database_available = true;
        runtime.snapshot.maintenance_mode = false;
        runtime.maintenance_reopen = false;
        runtime.snapshot.last_error = None;
        runtime.workspace = Some(workspace);
        runtime.snapshot.clone()
    };
    notify(inner, snapshot);

    let runtime = lock_runtime(inner);
    let mut runtime = inner
        .wake_worker
        .wait_while(runtime, |runtime| !runtime.stop_requested)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let workspace = runtime.workspace.take();
    drop(runtime);

    if let Some(workspace) = workspace {
        while Arc::strong_count(&workspace.store) > 1 {
            thread::sleep(Duration::from_millis(5));
        }
        let _ = workspace.store.checkpoint();
        drop(workspace);
    }

    drop(lease);
    runtime = lock_runtime(inner);
    runtime.worker_active = false;
    inner.wake_worker.notify_all();
    drop(runtime);
}

fn wait_or_stopping(inner: &Arc<SupervisorInner>, duration: Duration) -> bool {
    let runtime = lock_runtime(inner);
    let (runtime, _) = inner
        .wake_worker
        .wait_timeout_while(runtime, duration, |runtime| !runtime.stop_requested)
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    runtime.stop_requested
}

fn stopping(inner: &Arc<SupervisorInner>) -> bool {
    lock_runtime(inner).stop_requested
}

fn finish_worker(inner: &Arc<SupervisorInner>) {
    let mut runtime = lock_runtime(inner);
    runtime.worker_active = false;
    runtime.workspace = None;
    inner.wake_worker.notify_all();
}

fn lock_runtime(inner: &SupervisorInner) -> MutexGuard<'_, RuntimeState> {
    inner
        .runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn notify(inner: &SupervisorInner, snapshot: EngineSnapshot) {
    let listener = Arc::clone(&inner.listener);
    let _ = catch_unwind(AssertUnwindSafe(move || listener(snapshot)));
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Mutex,
        },
        time::{Duration, Instant},
    };

    use super::*;

    fn listener(events: Arc<Mutex<Vec<EngineSnapshot>>>) -> StateListener {
        Arc::new(move |snapshot| {
            events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(snapshot);
        })
    }

    fn wait_for(supervisor: &EngineSupervisor, expected: EngineState) -> EngineSnapshot {
        // Full-suite Windows runs start several real SQLite-backed engines concurrently. Keep the
        // assertion bounded while allowing migration and antivirus I/O contention to settle.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let snapshot = supervisor.snapshot();
            if snapshot.state == expected {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "engine did not reach {expected:?}"
            );
            // Leave CPU for the supervisor's real worker when readiness tests run concurrently.
            thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn start_reaches_ready_with_a_stable_workspace_and_generation() {
        let directory = tempfile::tempdir().unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let supervisor = EngineSupervisor::with_opener(
            Arc::new({
                let bootstrap = WorkspaceBootstrap::at(directory.path().join("workspace"));
                move || bootstrap.open()
            }),
            listener(Arc::clone(&events)),
            Vec::new(),
        );

        assert_eq!(supervisor.start().state, EngineState::Starting);
        let ready = wait_for(&supervisor, EngineState::Ready);
        assert_eq!(ready.generation, 1);
        assert!(ready.workspace_id.is_some());
        assert!(ready.database_available);
        let workspace = supervisor.workspace().unwrap();
        assert_eq!(
            workspace.store.workspace_id(),
            workspace.identity.workspace_id.to_string()
        );
        assert!(events
            .lock()
            .unwrap()
            .iter()
            .any(|event| event.state == EngineState::Ready));
        drop(workspace);
        assert!(supervisor.shutdown(Duration::from_secs(1)));
        assert!(!supervisor.shutdown(Duration::from_secs(1)));
    }

    #[test]
    fn panic_is_contained_and_automatic_restarts_are_bounded() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let supervisor = EngineSupervisor::with_opener(
            Arc::new({
                let attempts = Arc::clone(&attempts);
                move || -> Result<WorkspaceLease, EngineFailure> {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    panic!("test worker panic")
                }
            }),
            Arc::new(|_| {}),
            vec![Duration::ZERO, Duration::ZERO, Duration::ZERO],
        );

        supervisor.start();
        let degraded = wait_for(&supervisor, EngineState::Degraded);
        assert_eq!(attempts.load(Ordering::SeqCst), 4);
        assert_eq!(degraded.last_error.unwrap().code, "engine_worker_panicked");
    }

    #[test]
    fn manual_retry_resets_the_budget_after_a_workspace_lock_is_released() {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap = WorkspaceBootstrap::at(directory.path().join("workspace"));
        let blocker = bootstrap.open().unwrap();
        let supervisor = EngineSupervisor::with_opener(
            Arc::new({
                let bootstrap = bootstrap.clone();
                move || bootstrap.open()
            }),
            Arc::new(|_| {}),
            Vec::new(),
        );

        supervisor.start();
        let degraded = wait_for(&supervisor, EngineState::Degraded);
        assert_eq!(degraded.last_error.unwrap().code, "workspace_locked");
        drop(blocker);

        assert_eq!(supervisor.retry().unwrap().state, EngineState::Recovering);
        let ready = wait_for(&supervisor, EngineState::Ready);
        assert_eq!(ready.generation, 1);
        supervisor.shutdown(Duration::from_secs(1));
    }

    #[test]
    fn shutdown_interrupts_restart_backoff_and_releases_the_lease() {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap = WorkspaceBootstrap::at(directory.path().join("workspace"));
        let supervisor = EngineSupervisor::new(bootstrap.clone(), Arc::new(|_| {}));
        supervisor.start();
        wait_for(&supervisor, EngineState::Ready);

        assert!(supervisor.shutdown(Duration::from_secs(1)));
        assert_eq!(supervisor.snapshot().state, EngineState::Stopping);
        assert!(bootstrap.open().is_ok());
    }

    #[test]
    fn maintenance_pause_releases_windows_file_handles_and_resume_reopens() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("workspace");
        let rollback = directory.path().join("workspace.rollback");
        let bootstrap = WorkspaceBootstrap::at(root.clone());
        let events = Arc::new(Mutex::new(Vec::new()));
        let supervisor = EngineSupervisor::new(bootstrap.clone(), listener(Arc::clone(&events)));
        supervisor.start();
        let initial = wait_for(&supervisor, EngineState::Ready);

        let paused = supervisor
            .pause_for_maintenance(Duration::from_secs(1))
            .unwrap();
        assert_eq!(paused.state, EngineState::Ready);
        assert!(paused.maintenance_mode);
        assert!(!paused.database_available);
        assert!(supervisor.workspace().is_none());
        assert!(bootstrap.open().is_ok());

        std::fs::rename(&root, &rollback).unwrap();
        std::fs::rename(&rollback, &root).unwrap();

        let recovering = supervisor.resume_after_maintenance().unwrap();
        assert_eq!(recovering.state, EngineState::Recovering);
        assert!(recovering.maintenance_mode);
        assert!(!recovering.database_available);
        let reopened = wait_for(&supervisor, EngineState::Ready);
        assert_eq!(reopened.generation, initial.generation + 1);
        assert_eq!(reopened.workspace_id, initial.workspace_id);
        assert!(!reopened.maintenance_mode);
        assert!(reopened.database_available);

        let events = events.lock().unwrap();
        assert!(events.iter().any(|event| event.maintenance_mode));
        assert!(events.iter().any(|event| {
            event.state == EngineState::Ready
                && event.generation == reopened.generation
                && !event.maintenance_mode
        }));
        drop(events);
        supervisor.shutdown(Duration::from_secs(1));
    }

    #[test]
    fn maintenance_timeout_keeps_commands_gated_until_store_users_release() {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap = WorkspaceBootstrap::at(directory.path().join("workspace"));
        let supervisor = EngineSupervisor::new(bootstrap.clone(), Arc::new(|_| {}));
        supervisor.start();
        wait_for(&supervisor, EngineState::Ready);
        let outstanding_command = supervisor.workspace().unwrap();

        let error = supervisor
            .pause_for_maintenance(Duration::from_millis(20))
            .unwrap_err();
        assert_eq!(error.code, "maintenance_pause_timeout");
        let timed_out = supervisor.snapshot();
        assert!(timed_out.maintenance_mode);
        assert!(!timed_out.database_available);
        assert!(supervisor.workspace().is_none());
        assert_eq!(bootstrap.open().unwrap_err().code, "workspace_locked");

        drop(outstanding_command);
        let paused = supervisor
            .pause_for_maintenance(Duration::from_secs(1))
            .unwrap();
        assert!(paused.maintenance_mode);
        assert!(paused.last_error.is_none());
        assert!(bootstrap.open().is_ok());

        supervisor.resume_after_maintenance().unwrap();
        wait_for(&supervisor, EngineState::Ready);
        supervisor.shutdown(Duration::from_secs(1));
    }

    #[test]
    fn failed_maintenance_reopen_clears_maintenance_and_degrades() {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap = WorkspaceBootstrap::at(directory.path().join("workspace"));
        let opens = Arc::new(AtomicUsize::new(0));
        let supervisor = EngineSupervisor::with_opener(
            Arc::new({
                let bootstrap = bootstrap.clone();
                let opens = Arc::clone(&opens);
                move || {
                    if opens.fetch_add(1, Ordering::SeqCst) == 0 {
                        bootstrap.open()
                    } else {
                        Err(EngineFailure::new(
                            "replacement_workspace_invalid",
                            "The replacement workspace is invalid",
                            false,
                            SuggestedAction::Restore,
                        ))
                    }
                }
            }),
            Arc::new(|_| {}),
            Vec::new(),
        );
        supervisor.start();
        wait_for(&supervisor, EngineState::Ready);
        supervisor
            .pause_for_maintenance(Duration::from_secs(1))
            .unwrap();

        let recovering = supervisor.resume_after_maintenance().unwrap();
        assert!(recovering.maintenance_mode);
        let degraded = wait_for(&supervisor, EngineState::Degraded);
        assert!(!degraded.maintenance_mode);
        assert!(!degraded.database_available);
        assert_eq!(
            degraded.last_error.unwrap().code,
            "replacement_workspace_invalid"
        );
        supervisor.shutdown(Duration::from_secs(1));
    }

    #[test]
    fn shutdown_from_maintenance_is_final_and_does_not_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let bootstrap = WorkspaceBootstrap::at(directory.path().join("workspace"));
        let supervisor = EngineSupervisor::new(bootstrap.clone(), Arc::new(|_| {}));
        supervisor.start();
        wait_for(&supervisor, EngineState::Ready);
        supervisor
            .pause_for_maintenance(Duration::from_secs(1))
            .unwrap();

        assert!(supervisor.shutdown(Duration::from_secs(1)));
        let stopped = supervisor.snapshot();
        assert_eq!(stopped.state, EngineState::Stopping);
        assert!(!stopped.maintenance_mode);
        assert!(!stopped.database_available);
        assert!(supervisor.resume_after_maintenance().is_err());
        assert!(bootstrap.open().is_ok());
    }
}
