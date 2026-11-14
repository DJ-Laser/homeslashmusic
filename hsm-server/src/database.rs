#![allow(dead_code)]

use std::{
    io::Write as _,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

use smol::Executor;

use crate::database::{jobs::JobContext, store::DatabaseStore};

mod jobs;
mod sources;
mod store;

#[derive(Debug)]
pub struct MusicDatabase {
    executor: Arc<Executor<'static>>,
    db: Arc<RwLock<DatabaseStore>>,
}

impl MusicDatabase {
    pub fn new_empty(executor: Arc<Executor<'static>>) -> Self {
        Self {
            executor,
            db: Arc::default(),
        }
    }

    fn ctx(&self) -> JobContext {
        JobContext::new(self.executor.clone(), self.db.clone())
    }

    pub async fn start_update(&self, path: PathBuf) {
        let ctx = self.ctx();

        self.executor
            .spawn(async move {
                print!("Loading local files...");
                let _ = std::io::stdout().flush();
                sources::local_files::load_all_from_path(ctx.clone(), path).await;
                println!("Done!");

                print!("Waiting 10 seconds for api requests...");
                let _ = std::io::stdout().flush();

                smol::Timer::after(Duration::from_secs(10)).await;
                println!("Done!");

                ctx.read_db(DatabaseStore::print_contents);
            })
            .await;
    }
}
