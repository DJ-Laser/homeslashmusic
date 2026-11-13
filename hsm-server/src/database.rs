use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
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
                sources::local_files::load_tracks_from_path(ctx.clone(), path).await;

                //ctx.read_db(|db| println!("{db:?}"));
            })
            .await;
    }
}
