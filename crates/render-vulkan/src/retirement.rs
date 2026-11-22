//! Deferred deletion for producers that detach with work still on a queue.
use crate::VulkanError;
use ash::vk;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Mutex, OnceLock, mpsc},
    task::Poll,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const POLL_INTERVAL: Duration = Duration::from_millis(2);
const RETIREMENT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ProducerRetirementReport {
    pub registered_jobs: usize,
    pub joined_jobs: usize,
    pub completed_submissions: usize,
    pub unconfirmed_submissions: usize,
    pub failed_jobs: usize,
}

#[derive(Default)]
struct Outcomes {
    report: ProducerRetirementReport,
    errors: Vec<String>,
}

impl Outcomes {
    fn failed(&mut self, error: String) {
        self.report.unconfirmed_submissions += 1;
        self.report.failed_jobs += 1;
        if self.errors.len() < 8 {
            self.errors.push(error);
        }
    }

    fn merge(&mut self, other: Self) {
        self.report.registered_jobs += other.report.registered_jobs;
        self.report.joined_jobs += other.report.joined_jobs;
        self.report.completed_submissions += other.report.completed_submissions;
        self.report.unconfirmed_submissions += other.report.unconfirmed_submissions;
        self.report.failed_jobs += other.report.failed_jobs;
        self.errors
            .extend(other.errors.into_iter().take(8 - self.errors.len()));
    }
}

struct Deletion {
    queued: Instant,
    poll: Box<dyn FnMut() -> Poll<Result<(), VulkanError>> + Send>,
}

struct Worker {
    enqueue: mpsc::Sender<Deletion>,
    join: JoinHandle<Outcomes>,
}

impl Worker {
    fn start() -> std::io::Result<Self> {
        let (enqueue, queue) = mpsc::channel();
        let join = thread::Builder::new()
            .name("gpu-producer-deletion".into())
            .spawn(move || drain(queue))?;
        Ok(Self { enqueue, join })
    }
}

#[derive(Default)]
struct Registry {
    worker: Option<Worker>,
    outcomes: Outcomes,
}

fn registry() -> &'static Mutex<Registry> {
    static QUEUE: OnceLock<Mutex<Registry>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(Registry::default()))
}

fn drain(queue: mpsc::Receiver<Deletion>) -> Outcomes {
    let mut outcomes = Outcomes::default();
    let mut pending = Vec::<Deletion>::new();
    let mut disconnected = false;
    while !disconnected || !pending.is_empty() {
        if disconnected {
            thread::sleep(POLL_INTERVAL);
        } else {
            let received = if pending.is_empty() {
                queue
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
            } else {
                queue.recv_timeout(POLL_INTERVAL)
            };
            match received {
                Ok(deletion) => pending.push(deletion),
                Err(mpsc::RecvTimeoutError::Disconnected) => disconnected = true,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
        pending.retain_mut(|deletion| {
            let result = catch_unwind(AssertUnwindSafe(|| (deletion.poll)()));
            let error = match result {
                Ok(Poll::Ready(Ok(()))) => {
                    outcomes.report.completed_submissions += 1;
                    None
                }
                Ok(Poll::Ready(Err(error))) => Some(error.to_string()),
                Ok(Poll::Pending) if deletion.queued.elapsed() < RETIREMENT_TIMEOUT => {
                    return true;
                }
                Ok(Poll::Pending) => Some(
                    VulkanError::ProducerUnconfirmed {
                        operation: "producer retirement",
                        source: vk::Result::TIMEOUT,
                    }
                    .to_string(),
                ),
                Err(_) => Some("GPU producer deletion panicked; resources remain pinned".into()),
            };
            outcomes.report.joined_jobs += 1;
            if let Some(error) = error {
                outcomes.failed(error);
            }
            false
        });
    }
    outcomes
}

pub(crate) fn retire(poll: impl FnMut() -> Poll<Result<(), VulkanError>> + Send + 'static) {
    // The closure owns a ManuallyDrop bundle. Dropping a failed or timed-out
    // deletion must keep all resources and their device pinned.
    let mut state = registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.outcomes.report.registered_jobs += 1;
    if state.worker.is_none() {
        match Worker::start() {
            Ok(worker) => state.worker = Some(worker),
            Err(error) => {
                state
                    .outcomes
                    .failed(format!("GPU producer deletion could not start: {error}"));
                return;
            }
        }
    }
    let enqueue = &state.worker.as_ref().unwrap().enqueue;
    if enqueue
        .send(Deletion {
            queued: Instant::now(),
            poll: Box::new(poll),
        })
        .is_err()
    {
        state
            .outcomes
            .failed("GPU producer deletion queue closed; resources remain pinned".into());
    }
}

/// Application shutdown only, after all renderer owners have been dropped.
pub fn finish_producer_retirements() -> (ProducerRetirementReport, Vec<String>) {
    let mut result = Outcomes::default();
    loop {
        let batch = std::mem::take(
            &mut *registry()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        result.merge(batch.outcomes);
        let Some(Worker { enqueue, join }) = batch.worker else {
            return (result.report, result.errors);
        };
        drop(enqueue);
        match join.join() {
            Ok(completed) => result.merge(completed),
            Err(_) => result.failed("GPU producer deletion worker panicked".into()),
        }
    }
}
