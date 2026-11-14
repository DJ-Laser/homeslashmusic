use std::sync::Arc;

use smol::Executor;

use crate::database::{DatabaseHandle, store::DatabaseStore};

#[derive(Debug, Clone)]
pub struct JobContext {
    executor: Arc<Executor<'static>>,
    handle: DatabaseHandle,
}

impl JobContext {
    pub fn new(executor: Arc<Executor<'static>>, handle: DatabaseHandle) -> Self {
        Self { executor, handle }
    }

    /// Spawn a new job that runs in parallel to the current job.
    /// Once the job completes the commit phase, the furure will finish.
    /// Dropping this future will cancel the job. Use `spawn_detatched` to keep it running
    pub fn spawn<F>(&self, fut: F) -> impl Future<Output = F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send,
    {
        self.executor.spawn(fut)
    }

    /// Spawn a new job that runs in parallel to the current job.
    /// The job will run in the background
    pub fn spawn_detatched<F>(&self, fut: F)
    where
        F: Future + Send + 'static,
        F::Output: Send,
    {
        self.executor.spawn(fut).detach();
    }

    /// Synchronously read from the database
    pub fn read_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseStore) -> R,
    {
        self.handle.read_db(f)
    }

    /// Synchronously write to the database
    pub fn write_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DatabaseStore) -> R,
    {
        self.handle.write_db(f)
    }
}
