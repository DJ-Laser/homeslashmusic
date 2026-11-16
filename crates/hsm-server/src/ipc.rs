use std::{
    fs,
    mem::DropGuard,
    path::{Path, PathBuf},
};

use log::error;
use smol::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::unix::{UnixListener, UnixStream},
    stream::StreamExt,
};
use thiserror::Error;

use crate::{AppHandle, ipc::IpcServerError::ListenerClosed};

#[derive(Debug, Error)]
pub enum IpcServerError {
    #[error("Failed to check socket file: {0}")]
    CheckSocketFileFailed(#[source] io::Error),

    #[error(
        "The homeslashmusic socket file was already present, is another `hsm-server` instance running?"
    )]
    SocketInUse,

    #[error("Failed to create ipc socket: {0}")]
    FailedToCreateSocket(#[source] io::Error),

    #[error("Listener unexpectedly closed")]
    ListenerClosed,
}

fn is_socket_in_use(socket_path: &Path) -> Result<bool, IpcServerError> {
    let socket_in_use = fs::exists(socket_path).map_err(IpcServerError::CheckSocketFileFailed)?;
    Ok(socket_in_use)
}

fn cleanup_socket(socket_path: &Path) {
    let _ = fs::remove_file(socket_path);
}

pub async fn run_ipc_server(handle: AppHandle) -> Result<(), IpcServerError> {
    let socket_path = PathBuf::from(hsm_ipc::socket_path());
    if is_socket_in_use(&socket_path)? {
        return Err(IpcServerError::SocketInUse);
    }

    let listener = DropGuard::new(
        UnixListener::bind(&socket_path).map_err(IpcServerError::FailedToCreateSocket)?,
        |_| cleanup_socket(&socket_path),
    );

    while let Some(stream) = listener.incoming().next().await {
        let stream_handle = handle.clone();
        handle
            .executor()
            .spawn(async {
                let res = if let Ok(stream) = stream {
                    handle_stream(stream_handle, stream).await
                } else {
                    stream.map(|_| ())
                };

                if let Err(error) = res {
                    error!("failed to connect to ipc client: {error}");
                }
            })
            .detach();
    }

    Err(ListenerClosed)
}

async fn handle_stream(handle: AppHandle, stream: UnixStream) -> io::Result<()> {
    let mut request_data = String::new();
    let mut stream_reader = BufReader::new(stream);
    stream_reader.read_line(&mut request_data).await?;

    let reply_data = handle.send_json(request_data).await;

    let mut stream = stream_reader.into_inner();
    stream.write_all(reply_data.as_bytes()).await?;

    Ok(())
}
