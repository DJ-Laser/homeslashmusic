//! Jobs are asynchronous operations for updating the database state
//! Jobs have a chain of dependencies

use std::sync::{Arc, RwLock};

use smol::Executor;

use crate::database::store::DatabaseStore;

#[derive(Debug, Clone)]
pub struct JobContext {
    executor: Arc<Executor<'static>>,
    db: Arc<RwLock<DatabaseStore>>,
}

impl JobContext {
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
        f(&self.db.read().unwrap())
    }

    /// Synchronously write to the database
    pub fn write_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DatabaseStore) -> R,
    {
        f(&mut self.db.write().unwrap())
    }
}
