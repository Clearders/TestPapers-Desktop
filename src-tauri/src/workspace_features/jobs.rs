//! FIFO, single-worker queues for import, generation, export and backup work.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(crate) const JOB_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QueueKind {
    Import,
    Generation,
    Export,
    Backup,
}

impl QueueKind {
    const ALL: [Self; 4] = [Self::Import, Self::Generation, Self::Export, Self::Backup];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum JobState {
    Queued,
    Running,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

impl JobState {
    fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(crate) struct JobId(pub(crate) Uuid);

impl JobId {
    fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum JobKind {
    #[serde(rename = "import")]
    Import,
    #[serde(rename = "generation")]
    Generation,
    #[serde(rename = "export")]
    Export,
    #[serde(rename = "backup")]
    Backup,
    #[serde(rename = "restore")]
    Restore,
    #[serde(rename = "dataDirectoryMigration")]
    DataDirectoryMigration,
}

impl JobKind {
    fn queue(self) -> Option<QueueKind> {
        match self {
            Self::Import => Some(QueueKind::Import),
            Self::Generation => Some(QueueKind::Generation),
            Self::Export => Some(QueueKind::Export),
            Self::Backup => Some(QueueKind::Backup),
            Self::Restore | Self::DataDirectoryMigration => None,
        }
    }

    fn requires_maintenance(self) -> bool {
        self.queue().is_none()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobError {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) recoverable: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JobSnapshot {
    pub(crate) schema_version: u32,
    pub(crate) id: JobId,
    pub(crate) queue: Option<QueueKind>,
    pub(crate) kind: JobKind,
    pub(crate) state: JobState,
    pub(crate) phase: String,
    pub(crate) completed_units: u64,
    pub(crate) total_units: Option<u64>,
    pub(crate) cancellable: bool,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<JobError>,
}

pub(crate) trait JobEventSink: Send + Sync + 'static {
    fn updated(&self, snapshot: JobSnapshot);
}

struct NoopJobEventSink;

impl JobEventSink for NoopJobEventSink {
    fn updated(&self, _snapshot: JobSnapshot) {}
}

#[derive(Clone, Debug)]
pub(crate) struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    pub(crate) fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub(crate) fn checkpoint(&self) -> Result<(), JobFailure> {
        if self.is_cancelled() {
            Err(JobFailure::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Clone)]
pub(crate) struct JobContext {
    id: JobId,
    token: CancellationToken,
    inner: Arc<CoordinatorInner>,
}

impl JobContext {
    pub(crate) fn id(&self) -> JobId {
        self.id.clone()
    }

    pub(crate) fn cancellation(&self) -> &CancellationToken {
        &self.token
    }

    pub(crate) fn update_progress(
        &self,
        phase: impl Into<String>,
        completed_units: u64,
        total_units: Option<u64>,
    ) {
        self.inner.mutate_job(self.id.clone(), |record| {
            record.snapshot.phase = phase.into();
            record.snapshot.completed_units = completed_units;
            record.snapshot.total_units = total_units;
        });
    }

    /// Marks the irreversible commit point. Cancellation requests made after this call are still
    /// recorded but do not misrepresent an already committed result as cancellable.
    pub(crate) fn commit_started(&self) {
        self.inner.mutate_job(self.id.clone(), |record| {
            record.snapshot.cancellable = false;
            record.snapshot.phase = "committing".into();
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum JobFailure {
    Cancelled,
    Failed(JobError),
}

impl JobFailure {
    pub(crate) fn recoverable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Failed(JobError {
            code: code.into(),
            message: message.into(),
            recoverable: true,
        })
    }

    pub(crate) fn fatal(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Failed(JobError {
            code: code.into(),
            message: message.into(),
            recoverable: false,
        })
    }
}

type JobWork = Box<dyn FnOnce(JobContext) -> Result<Value, JobFailure> + Send + 'static>;

struct QueuedJob {
    id: JobId,
    token: CancellationToken,
    work: JobWork,
}

enum QueueMessage {
    Run(QueuedJob),
    Shutdown,
}

struct JobRecord {
    snapshot: JobSnapshot,
    token: CancellationToken,
}

struct CoordinatorState {
    maintenance: bool,
    shutting_down: bool,
    jobs: BTreeMap<JobId, JobRecord>,
}

struct CoordinatorInner {
    state: Mutex<CoordinatorState>,
    changed: Condvar,
    event_sink: Arc<dyn JobEventSink>,
}

impl CoordinatorInner {
    fn mutate_job(&self, id: JobId, mutate: impl FnOnce(&mut JobRecord)) {
        let snapshot = {
            let mut state = self.state.lock().expect("job state lock poisoned");
            let Some(record) = state.jobs.get_mut(&id) else {
                return;
            };
            mutate(record);
            record.snapshot.clone()
        };
        self.changed.notify_all();
        self.event_sink.updated(snapshot);
    }

    fn all_terminal(&self) -> bool {
        self.state
            .lock()
            .expect("job state lock poisoned")
            .jobs
            .values()
            .all(|record| record.snapshot.state.terminal())
    }
}

pub(crate) struct JobCoordinator {
    inner: Arc<CoordinatorInner>,
    senders: BTreeMap<QueueKind, mpsc::Sender<QueueMessage>>,
    workers: Mutex<Vec<JoinHandle<()>>>,
}

impl JobCoordinator {
    pub(crate) fn new(event_sink: Option<Arc<dyn JobEventSink>>) -> Self {
        let inner = Arc::new(CoordinatorInner {
            state: Mutex::new(CoordinatorState {
                maintenance: false,
                shutting_down: false,
                jobs: BTreeMap::new(),
            }),
            changed: Condvar::new(),
            event_sink: event_sink.unwrap_or_else(|| Arc::new(NoopJobEventSink)),
        });
        let mut senders = BTreeMap::new();
        let mut workers = Vec::new();
        for queue in QueueKind::ALL {
            let (sender, receiver) = mpsc::channel();
            senders.insert(queue, sender);
            let worker_inner = Arc::clone(&inner);
            workers.push(
                thread::Builder::new()
                    .name(format!("testpapers-{queue:?}-worker").to_lowercase())
                    .spawn(move || worker_loop(receiver, worker_inner))
                    .expect("failed to start workspace job worker"),
            );
        }
        Self {
            inner,
            senders,
            workers: Mutex::new(workers),
        }
    }

    pub(crate) fn submit<F>(&self, kind: JobKind, work: F) -> Result<JobId, SubmitError>
    where
        F: FnOnce(JobContext) -> Result<Value, JobFailure> + Send + 'static,
    {
        let queue = kind
            .queue()
            .ok_or(SubmitError::ExclusiveMaintenanceRequired)?;
        let id = JobId::new();
        let token = CancellationToken::new();
        let snapshot = JobSnapshot {
            schema_version: JOB_SCHEMA_VERSION,
            id: id.clone(),
            queue: Some(queue),
            kind,
            state: JobState::Queued,
            phase: "queued".into(),
            completed_units: 0,
            total_units: None,
            cancellable: true,
            result: None,
            error: None,
        };
        {
            let mut state = self.inner.state.lock().expect("job state lock poisoned");
            if state.shutting_down {
                return Err(SubmitError::ShuttingDown);
            }
            if state.maintenance {
                return Err(SubmitError::Maintenance);
            }
            state.jobs.insert(
                id.clone(),
                JobRecord {
                    snapshot: snapshot.clone(),
                    token: token.clone(),
                },
            );
        }
        self.inner.event_sink.updated(snapshot);
        if self.senders[&queue]
            .send(QueueMessage::Run(QueuedJob {
                id: id.clone(),
                token,
                work: Box::new(work),
            }))
            .is_err()
        {
            self.inner.mutate_job(id.clone(), |record| {
                record.snapshot.state = JobState::Failed;
                record.snapshot.phase = "failed".into();
                record.snapshot.error = Some(JobError {
                    code: "QUEUE_UNAVAILABLE".into(),
                    message: "The background worker is unavailable.".into(),
                    recoverable: true,
                });
                record.snapshot.cancellable = false;
            });
            return Err(SubmitError::QueueUnavailable);
        }
        Ok(id)
    }

    /// Starts a restore or data-directory migration only after all regular queues have drained.
    /// The returned job runs on its own supervised thread and holds the maintenance lease until it
    /// reaches a terminal state.
    pub(crate) fn submit_maintenance<F>(
        &self,
        kind: JobKind,
        timeout: Duration,
        work: F,
    ) -> Result<JobId, MaintenanceSubmitError>
    where
        F: FnOnce(JobContext) -> Result<Value, JobFailure> + Send + 'static,
    {
        if !kind.requires_maintenance() {
            return Err(MaintenanceSubmitError::RegularQueueKind);
        }
        let lease = self
            .begin_maintenance(timeout)
            .map_err(MaintenanceSubmitError::Maintenance)?;
        let id = JobId::new();
        let token = CancellationToken::new();
        let snapshot = JobSnapshot {
            schema_version: JOB_SCHEMA_VERSION,
            id: id.clone(),
            queue: None,
            kind,
            state: JobState::Running,
            phase: "running".into(),
            completed_units: 0,
            total_units: None,
            cancellable: true,
            result: None,
            error: None,
        };
        {
            let mut state = self.inner.state.lock().expect("job state lock poisoned");
            state.jobs.insert(
                id.clone(),
                JobRecord {
                    snapshot: snapshot.clone(),
                    token: token.clone(),
                },
            );
        }
        self.inner.event_sink.updated(snapshot);
        let worker_inner = Arc::clone(&self.inner);
        let failure_inner = Arc::clone(&self.inner);
        let worker_id = id.clone();
        let worker_token = token.clone();
        let spawn = thread::Builder::new()
            .name(format!("testpapers-{kind:?}-worker").to_lowercase())
            .spawn(move || {
                let context = JobContext {
                    id: worker_id.clone(),
                    token: worker_token,
                    inner: Arc::clone(&worker_inner),
                };
                let result = work(context);
                finish_job(&worker_inner, worker_id, result);
                drop(lease);
            });
        match spawn {
            Ok(worker) => self
                .workers
                .lock()
                .expect("worker lock poisoned")
                .push(worker),
            Err(error) => {
                finish_job(
                    &failure_inner,
                    id.clone(),
                    Err(JobFailure::recoverable(
                        "MAINTENANCE_WORKER_UNAVAILABLE",
                        format!("The maintenance worker could not start: {error}"),
                    )),
                );
                return Err(MaintenanceSubmitError::WorkerUnavailable);
            }
        }
        Ok(id)
    }

    pub(crate) fn get(&self, id: &JobId) -> Option<JobSnapshot> {
        self.inner
            .state
            .lock()
            .expect("job state lock poisoned")
            .jobs
            .get(id)
            .map(|record| record.snapshot.clone())
    }

    pub(crate) fn cancel(&self, id: &JobId) -> Result<(), CancelError> {
        let token = {
            let state = self.inner.state.lock().expect("job state lock poisoned");
            let record = state.jobs.get(id).ok_or(CancelError::UnknownJob)?;
            if record.snapshot.state.terminal() {
                return Err(CancelError::AlreadyFinished);
            }
            if !record.snapshot.cancellable {
                return Err(CancelError::CommitStarted);
            }
            record.token.clone()
        };
        token.cancel();
        self.inner.mutate_job(id.clone(), |record| {
            record.snapshot.state = JobState::Cancelling;
            record.snapshot.phase = "cancelling".into();
        });
        Ok(())
    }

    pub(crate) fn begin_maintenance(
        &self,
        timeout: Duration,
    ) -> Result<MaintenanceLease, MaintenanceError> {
        let deadline = Instant::now() + timeout;
        let mut state = self.inner.state.lock().expect("job state lock poisoned");
        if state.shutting_down {
            return Err(MaintenanceError::ShuttingDown);
        }
        if state.maintenance {
            return Err(MaintenanceError::AlreadyActive);
        }
        state.maintenance = true;
        let mut cancelling = Vec::new();
        for record in state.jobs.values_mut() {
            if !record.snapshot.state.terminal() && record.snapshot.cancellable {
                record.token.cancel();
                record.snapshot.state = JobState::Cancelling;
                record.snapshot.phase = "cancelling".into();
                cancelling.push(record.snapshot.clone());
            }
        }
        drop(state);
        for snapshot in cancelling {
            self.inner.event_sink.updated(snapshot);
        }
        state = self.inner.state.lock().expect("job state lock poisoned");
        while state
            .jobs
            .values()
            .any(|record| !record.snapshot.state.terminal())
        {
            let now = Instant::now();
            if now >= deadline {
                state.maintenance = false;
                self.inner.changed.notify_all();
                return Err(MaintenanceError::TimedOut);
            }
            let (next, wait) = self
                .inner
                .changed
                .wait_timeout(state, deadline.saturating_duration_since(now))
                .expect("job state lock poisoned");
            state = next;
            if wait.timed_out()
                && state
                    .jobs
                    .values()
                    .any(|record| !record.snapshot.state.terminal())
            {
                state.maintenance = false;
                self.inner.changed.notify_all();
                return Err(MaintenanceError::TimedOut);
            }
        }
        drop(state);
        Ok(MaintenanceLease {
            inner: Arc::clone(&self.inner),
            active: true,
        })
    }
}

impl Drop for JobCoordinator {
    fn drop(&mut self) {
        {
            let mut state = self.inner.state.lock().expect("job state lock poisoned");
            state.shutting_down = true;
            for record in state.jobs.values() {
                if !record.snapshot.state.terminal() && record.snapshot.cancellable {
                    record.token.cancel();
                }
            }
        }
        for sender in self.senders.values() {
            let _ = sender.send(QueueMessage::Shutdown);
        }
        for worker in self.workers.lock().expect("worker lock poisoned").drain(..) {
            let _ = worker.join();
        }
    }
}

pub(crate) struct MaintenanceLease {
    inner: Arc<CoordinatorInner>,
    active: bool,
}

impl MaintenanceLease {
    pub(crate) fn release(mut self) {
        self.finish();
    }

    fn finish(&mut self) {
        if !self.active {
            return;
        }
        let mut state = self.inner.state.lock().expect("job state lock poisoned");
        state.maintenance = false;
        self.active = false;
        self.inner.changed.notify_all();
    }
}

impl Drop for MaintenanceLease {
    fn drop(&mut self) {
        self.finish();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SubmitError {
    Maintenance,
    ShuttingDown,
    QueueUnavailable,
    ExclusiveMaintenanceRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaintenanceSubmitError {
    RegularQueueKind,
    Maintenance(MaintenanceError),
    WorkerUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CancelError {
    UnknownJob,
    AlreadyFinished,
    CommitStarted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MaintenanceError {
    AlreadyActive,
    ShuttingDown,
    TimedOut,
}

impl fmt::Display for MaintenanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyActive => formatter.write_str("workspace maintenance is already active"),
            Self::ShuttingDown => formatter.write_str("the job coordinator is shutting down"),
            Self::TimedOut => {
                formatter.write_str("background jobs did not stop before the maintenance deadline")
            }
        }
    }
}

fn worker_loop(receiver: mpsc::Receiver<QueueMessage>, inner: Arc<CoordinatorInner>) {
    while let Ok(message) = receiver.recv() {
        let QueueMessage::Run(job) = message else {
            return;
        };
        let QueuedJob { id, token, work } = job;
        if token.is_cancelled() {
            finish_job(&inner, id, Err(JobFailure::Cancelled));
            continue;
        }
        inner.mutate_job(id.clone(), |record| {
            record.snapshot.state = JobState::Running;
            record.snapshot.phase = "running".into();
        });
        let context = JobContext {
            id: id.clone(),
            token,
            inner: Arc::clone(&inner),
        };
        let result = work(context);
        finish_job(&inner, id, result);
    }
}

fn finish_job(inner: &CoordinatorInner, id: JobId, result: Result<Value, JobFailure>) {
    inner.mutate_job(id, |record| {
        record.snapshot.cancellable = false;
        match result {
            Ok(value) => {
                record.snapshot.state = JobState::Completed;
                record.snapshot.phase = "completed".into();
                record.snapshot.result = Some(value);
            }
            Err(JobFailure::Cancelled) => {
                record.snapshot.state = JobState::Cancelled;
                record.snapshot.phase = "cancelled".into();
            }
            Err(JobFailure::Failed(error)) => {
                record.snapshot.state = JobState::Failed;
                record.snapshot.phase = "failed".into();
                record.snapshot.error = Some(error);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait_for_terminal(coordinator: &JobCoordinator, id: &JobId) -> JobSnapshot {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let snapshot = coordinator.get(id).unwrap();
            if snapshot.state.terminal() {
                return snapshot;
            }
            assert!(Instant::now() < deadline, "job did not finish");
            thread::yield_now();
        }
    }

    #[test]
    fn each_feature_queue_is_fifo_and_reports_progress() {
        let coordinator = JobCoordinator::new(None);
        let (sender, receiver) = mpsc::channel();
        let first_sender = sender.clone();
        let first = coordinator
            .submit(JobKind::Export, move |context| {
                context.update_progress("rendering", 1, Some(2));
                first_sender.send(1).unwrap();
                Ok(serde_json::json!({"file": "paper.docx"}))
            })
            .unwrap();
        let second = coordinator
            .submit(JobKind::Export, move |_| {
                sender.send(2).unwrap();
                Ok(Value::Null)
            })
            .unwrap();
        assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).unwrap(), 1);
        assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).unwrap(), 2);
        assert_eq!(
            wait_for_terminal(&coordinator, &first).state,
            JobState::Completed
        );
        assert_eq!(
            wait_for_terminal(&coordinator, &second).state,
            JobState::Completed
        );
    }

    #[test]
    fn maintenance_cancels_cooperative_work_and_blocks_submissions() {
        let coordinator = JobCoordinator::new(None);
        let job = coordinator
            .submit(JobKind::Generation, |context| loop {
                context.cancellation().checkpoint()?;
                thread::yield_now();
            })
            .unwrap();
        let lease = coordinator
            .begin_maintenance(Duration::from_secs(1))
            .unwrap();
        assert_eq!(
            wait_for_terminal(&coordinator, &job).state,
            JobState::Cancelled
        );
        assert_eq!(
            coordinator.submit(JobKind::Backup, |_| Ok(Value::Null)),
            Err(SubmitError::Maintenance)
        );
        lease.release();
        assert!(coordinator
            .submit(JobKind::Backup, |_| Ok(Value::Null))
            .is_ok());
    }

    #[test]
    fn commit_point_rejects_cancellation() {
        let coordinator = JobCoordinator::new(None);
        let (started, wait) = mpsc::channel();
        let id = coordinator
            .submit(JobKind::Import, move |context| {
                context.commit_started();
                started.send(()).unwrap();
                thread::sleep(Duration::from_millis(20));
                Ok(Value::Null)
            })
            .unwrap();
        wait.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(coordinator.cancel(&id), Err(CancelError::CommitStarted));
        assert_eq!(
            wait_for_terminal(&coordinator, &id).state,
            JobState::Completed
        );
    }

    #[test]
    fn maintenance_kinds_use_uuid_v7_and_the_exclusive_path() {
        let coordinator = JobCoordinator::new(None);
        assert_eq!(
            coordinator.submit(JobKind::Restore, |_| Ok(Value::Null)),
            Err(SubmitError::ExclusiveMaintenanceRequired)
        );
        let id = coordinator
            .submit_maintenance(
                JobKind::DataDirectoryMigration,
                Duration::from_secs(1),
                |_| Ok(Value::Null),
            )
            .unwrap();
        assert_eq!(id.0.get_version_num(), 7);
        assert_eq!(
            wait_for_terminal(&coordinator, &id).state,
            JobState::Completed
        );
    }

    #[test]
    fn job_snapshot_matches_the_typed_wire_contract() {
        let snapshot = JobSnapshot {
            schema_version: JOB_SCHEMA_VERSION,
            id: JobId(Uuid::parse_str("018f0000-0000-7000-8000-000000000001").unwrap()),
            queue: None,
            kind: JobKind::DataDirectoryMigration,
            state: JobState::Cancelling,
            phase: "cancelling".into(),
            completed_units: 2,
            total_units: Some(5),
            cancellable: true,
            result: None,
            error: None,
        };
        let value = serde_json::to_value(snapshot).unwrap();
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["id"], "018f0000-0000-7000-8000-000000000001");
        assert_eq!(value["kind"], "dataDirectoryMigration");
        assert_eq!(value["state"], "cancelling");
    }
}
