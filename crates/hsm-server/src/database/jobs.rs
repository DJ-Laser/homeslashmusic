use std::sync::Arc;

use smol::Executor;

use crate::{
    config::database::DatabaseConfig,
    database::{DatabaseHandle, store::DatabaseStore},
};

#[derive(Debug, Clone)]
pub struct JobContext {
    executor: Arc<Executor<'static>>,
    handle: DatabaseHandle,
}

impl JobContext {
    pub fn new(executor: Arc<Executor<'static>>, handle: DatabaseHandle) -> Self {
        Self { executor, handle }
    }

    /// Spawn a new task that runs in parallel to the current task.
    /// Dropping this future will cancel it.
    pub fn spawn<F>(&self, fut: F) -> impl Future<Output = F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send,
    {
        self.executor.spawn(fut)
    }

    /// Synchronously read from the database
    pub fn read_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseStore) -> R,
    {
        self.handle.read_db(f)
    }

    /// Synchronously read from the database
    pub fn read_config<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseConfig) -> R,
    {
        self.handle.read_config(|config| f(config))
    }

    /// Synchronously write to the database
    pub fn write_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DatabaseStore) -> R,
    {
        self.handle.write_state(|state| f(&mut state.store))
    }
}
