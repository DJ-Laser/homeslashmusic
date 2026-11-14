#![allow(dead_code)]

use std::{
    io::Write as _,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

use smol::{
    Executor, Task,
    channel::{self, Sender},
};

use crate::database::{
    jobs::JobContext,
    store::{DatabaseStore, fetch_incomplete_mbids},
};

mod jobs;
mod sources;
mod store;

#[derive(Debug)]
struct MusicDatabase {
    db: RwLock<DatabaseStore>,
    mbid_wake_tx: Sender<()>,
}

#[derive(Debug, Clone)]
pub struct DatabaseHandle(Arc<MusicDatabase>);

impl DatabaseHandle {
    fn ctx(&self, executor: Arc<Executor<'static>>) -> JobContext {
        JobContext::new(executor, self.clone())
    }

    fn read_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&DatabaseStore) -> R,
    {
        f(&self.0.db.read().unwrap())
    }

    fn write_db<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut DatabaseStore) -> R,
    {
        let db = &mut self.0.db.write().unwrap();
        let ret = f(db);
        if db.has_incomplete_mbids() {
            let _ = self.0.mbid_wake_tx.try_send(());
        }

        ret
    }

    pub fn run_job<F, R>(
        &self,
        executor: &Arc<Executor<'static>>,
        job: impl FnOnce(JobContext) -> F,
    ) -> Task<R>
    where
        F: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        executor.spawn(job(self.ctx(executor.clone())))
    }
}

fn start_database(executor: &Arc<Executor<'static>>, store: DatabaseStore) -> DatabaseHandle {
    let (mbid_wake_tx, mbid_wake_rx) = channel::bounded(1);
    let handle = DatabaseHandle(Arc::new(MusicDatabase {
        db: RwLock::new(store),
        mbid_wake_tx,
    }));

    handle
        .run_job(executor, |ctx| fetch_incomplete_mbids(ctx, mbid_wake_rx))
        .detach();

    handle
}

pub fn init_blank_database(executor: &Arc<Executor<'static>>) -> DatabaseHandle {
    start_database(executor, DatabaseStore::default())
}

pub async fn start_update(ctx: JobContext, path: PathBuf) {
    println!("Starting database update");
    println!();

    print!("Loading local files...");
    let _ = std::io::stdout().flush();
    sources::local_files::load_all_from_path(ctx.clone(), path).await;
    println!("Done!");

    print!("Waiting for api requests");
    let _ = std::io::stdout().flush();

    loop {
        smol::Timer::after(Duration::from_secs(1)).await;
        print!(".");
        let _ = std::io::stdout().flush();

        if ctx.read_db(|db| !db.has_incomplete_mbids()) {
            break;
        }
    }

    println!("Done!");

    ctx.read_db(DatabaseStore::print_contents);
}
