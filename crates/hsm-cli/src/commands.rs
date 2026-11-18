use std::path::{self, PathBuf};

use crate::cli::{Cli, Command, QueueCommand};
use crate::ipc::send_request;
use hsm_ipc::{InsertPosition, LoopMode, QueuedTrack, SeekOffset, requests};

fn try_load_tracks(position: InsertPosition, paths: Vec<PathBuf>) -> Result<(), crate::Error> {
    let mut absolute_paths = Vec::new();
    for path in paths {
        absolute_paths.push(
            path::absolute(path)
                .map_err(crate::Error::GetCurrentDirFailed)?
                .to_string_lossy()
                .into_owned(),
        );
    }

    let tracks = send_request(requests::LookupTracks(absolute_paths))?;

    let errors = send_request(requests::LoadTracks(tracks, position))?;

    for track_id in errors {
        eprintln!("Failed to load track {}", track_id.0);
    }

    Ok(())
}

fn handle_queue_command(command: QueueCommand) -> Result<(), crate::Error> {
    match command {
        QueueCommand::Clear => send_request(requests::ClearQueue)?,
        QueueCommand::Replace { tracks } => {
            send_request(requests::ClearQueue)?;
            try_load_tracks(InsertPosition::Start, tracks.paths)?;
        }
        QueueCommand::Add { tracks } => try_load_tracks(InsertPosition::End, tracks.paths)?,
        QueueCommand::Next { tracks } => {
            try_load_tracks(InsertPosition::Next, tracks.paths)?;
        }
    }

    Ok(())
}

fn print_queue() -> Result<(), crate::Error> {
    let tracks = send_request(requests::GetQueuedTracks)?;
    let current_track_id = send_request(requests::GetCurrentQueuedTrackId)?;

    for QueuedTrack { metadata, id, .. } in tracks {
        if let Some(current_id) = current_track_id
            && current_id == id
        {
            println!("> {}", metadata.title);
        } else {
            println!("| {}", metadata.title);
        }
    }

    Ok(())
}

pub fn handle_command(command: Cli) -> Result<(), crate::Error> {
    match command.command {
        Command::Play { tracks } => {
            if let Some(tracks) = tracks {
                send_request(requests::ClearQueue)?;
                try_load_tracks(InsertPosition::Start, tracks.paths)?;
            }

            send_request(requests::Play)?;
        }
        Command::Pause => send_request(requests::Pause)?,
        Command::PlayPause => send_request(requests::TogglePlayback)?,
        Command::Stop => send_request(requests::StopPlayback)?,

        Command::Next => send_request(requests::GoToNextTrack)?,
        Command::Previous => send_request(requests::GoToPreviousTrack)?,

        Command::Loop { loop_mode } => {
            if let Some(loop_mode) = loop_mode {
                send_request(requests::SetLoopMode(loop_mode.into()))?;
            } else {
                let loop_mode = send_request(requests::GetLoopMode)?;
                match loop_mode {
                    LoopMode::None => println!("Loop: none"),
                    LoopMode::Track => println!("Loop: track"),
                    LoopMode::Playlist => println!("Loop: playlist"),
                }
            }
        }
        Command::Shuffle { shuffle } => {
            if let Some(shuffle) = shuffle {
                send_request(requests::SetShuffle(shuffle.into()))?;
            } else {
                let shuffle = send_request(requests::GetShuffle)?;
                if shuffle {
                    println!("Shuffle: on");
                } else {
                    println!("Shuffle: off");
                }
            }
        }
        Command::Volume { volume } => {
            if let Some(volume) = volume {
                send_request(requests::SetVolume(volume))?;
            } else {
                let volume = send_request(requests::GetVolume)?;
                println!("Volume: {volume}");
            }
        }

        Command::Seek { seek_position } => match SeekOffset::try_from(seek_position) {
            Ok(seek_offset) => send_request(requests::SeekCurrent(seek_offset))?,
            Err(position) => {
                if let Some(track_id) = send_request(requests::GetCurrentQueuedTrackId)? {
                    send_request(requests::SeekTo(track_id, position))?;
                }
            }
        },

        Command::Queue { command, tracks } => {
            if let Some(command) = command {
                handle_queue_command(command)?;
            } else if let Some(tracks) = tracks {
                handle_queue_command(QueueCommand::Add { tracks })?;
            } else {
                print_queue()?;
            }
        }
    }

    Ok(())
}
