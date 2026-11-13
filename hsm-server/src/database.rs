use std::sync::{Arc, RwLock};

use smol::Executor;

use crate::database::store::DatabaseStore;

mod jobs;
mod sources;
mod store;

pub struct MusicDatabase {
    executor: Arc<Executor<'static>>,
    store: Arc<RwLock<DatabaseStore>>,
}

impl MusicDatabase {
    pub fn start_update() {}
}
