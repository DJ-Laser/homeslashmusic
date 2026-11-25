use std::{
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

use crc::Crc;
use futures_concurrency::future::Race;
use log::{error, info};
use smol::{
    Executor, Task, Timer,
    channel::{self},
};
use thiserror::Error;

use crate::database::{DatabaseHandle, store::DatabaseStore};

const DB_MAGIC: &[u8; 5] = b"HSMDB";
const DB_VERSION: u32 = 1;
const HEADER_LEN: usize = 9;
const CRC_ALGORITHM: crc::Algorithm<u32> = crc::CRC_32_ISO_HDLC;

#[derive(Debug, Error)]
pub enum LoadDbError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("Invalid database file header (not a HSMDB file)")]
    InvalidMagic,

    #[error("Unsupported database schema version {found} (expected {expected})")]
    UnsupportedVersion { found: u32, expected: u32 },

    #[error("Failed to decode database: {0}")]
    Deserialize(#[from] postcard::Error),
}

pub async fn load_db(path: PathBuf) -> Result<Option<DatabaseStore>, LoadDbError> {
    let store = smol::unblock(move || {
        if !fs::exists(&path)? {
            return Ok(None);
        }

        let bytes = fs::read(path)?;

        if bytes.len() < HEADER_LEN || &bytes[0..5] != DB_MAGIC {
            return Err(LoadDbError::InvalidMagic);
        }

        let version =
            u32::from_le_bytes(bytes[5..9].try_into().expect("Sliced a 4-wide byte slice"));
        if version != DB_VERSION {
            return Err(LoadDbError::UnsupportedVersion {
                found: version,
                expected: DB_VERSION,
            });
        }

        let crc = Crc::<u32>::new(&CRC_ALGORITHM);
        let store: DatabaseStore = postcard::from_bytes_crc32(&bytes[HEADER_LEN..], crc.digest())?;
        Ok(Some(store))
    })
    .await?;

    Ok(store)
}

#[derive(Debug, Error)]

pub enum SaveDbError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error("Failed to serialize database: {0}")]
    Serialize(#[from] postcard::Error),
}

pub fn save_db_file(path: PathBuf, payload: &[u8]) -> Result<(), SaveDbError> {
    let tmp_path = path.with_added_extension(".tmp");

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Use a closure to stop on error without returning from the outer function
    let res = (|| {
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(DB_MAGIC)?;
            file.write_all(&DB_VERSION.to_le_bytes())?;
            file.write_all(payload)?;
            file.sync_all()?;
        }

        fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if res.is_err() {
        let _ = fs::remove_file(tmp_path);
    }

    res
}

pub async fn save_db(handle: &DatabaseHandle, path: PathBuf) -> Result<(), SaveDbError> {
    let crc = Crc::<u32>::new(&CRC_ALGORITHM);

    let payload = handle.read_db(|db| postcard::to_allocvec_crc32(db, crc.digest()))?;

    smol::unblock(move || save_db_file(path, &payload)).await
}

async fn save_db_debounced(handle: DatabaseHandle, waker: channel::Receiver<()>) {
    async fn save_handle_err(handle: &DatabaseHandle) {
        let save_path = handle.read_config(|config| config.database_file_path.clone());

        if let Err(error) = save_db(handle, save_path.clone()).await {
            error!("Error saving the database: {error}");
        } else {
            info!("Successfully saved database to {}", save_path.display());
        }
    }

    'main: loop {
        if waker.recv().await.is_err() {
            break 'main;
        }

        let (debounce_delay, max_delay) =
            handle.read_config(|config| (config.autosave_debounce(), config.autosave_max_delay()));

        let first_dirty_at = Instant::now();

        loop {
            if max_delay.saturating_sub(first_dirty_at.elapsed()).is_zero() {
                break;
            }

            let write_occurred = (
                async {
                    Timer::after(debounce_delay).await;
                    Ok(false)
                },
                async { waker.recv().await.map(|()| true) },
            )
                .race()
                .await;

            // If no write happened, or the channel was dropped, save the file
            match write_occurred {
                Ok(false) => break,
                Ok(true) => (),
                Err(_) => break 'main,
            }
        }

        save_handle_err(&handle).await;
    }

    save_handle_err(&handle).await;
}

#[derive(Debug)]
pub struct AutosaveTask {
    wake_tx: channel::Sender<()>,
    task: Task<()>,
}

impl AutosaveTask {
    pub async fn stop(self) {
        let Self { wake_tx, task } = self;
        wake_tx.close();
        task.await;
    }
}

pub fn start_autosave_task(
    executor: &Arc<Executor<'static>>,
    handle: DatabaseHandle,
    channel: (channel::Sender<()>, channel::Receiver<()>),
) -> AutosaveTask {
    let task = executor.spawn(save_db_debounced(handle, channel.1));

    AutosaveTask {
        wake_tx: channel.0,
        task,
    }
}
