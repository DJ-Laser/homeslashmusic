use crate::cli::{Command, QueueCommand, SearchQuery};
use crate::ipc::send_request;
use hsm_ipc::{InsertPosition, LoopMode, QueuedTrack, SeekOffset, requests};

fn try_load_tracks(
    position: InsertPosition,
    search_query: &SearchQuery,
) -> Result<(), crate::Error> {
    let search_results = send_request(requests::Search(search_query.string()))?;

    let Some(insert_item) = search_results
        .into_iter()
        .find_map(|(search_result, _)| search_result.try_into().ok())
    else {
        eprintln!("No results found for \"{}\"", search_query.string());
        return Ok(());
    };

    send_request(requests::LoadTracks(insert_item, position))?;

    Ok(())
}

fn handle_queue_command(command: QueueCommand) -> Result<(), crate::Error> {
    match command {
        QueueCommand::Clear => send_request(requests::ClearQueue)?,
        QueueCommand::Replace { query } => {
            send_request(requests::ClearQueue)?;
            try_load_tracks(InsertPosition::Start, &query)?;
        }
        QueueCommand::Add { query } => try_load_tracks(InsertPosition::End, &query)?,
        QueueCommand::Next { query } => {
            try_load_tracks(InsertPosition::Next, &query)?;
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

pub fn handle_command(command: Command) -> Result<(), crate::Error> {
    match command {
        Command::Play { query } => {
            if let Some(query) = query {
                send_request(requests::ClearQueue)?;
                try_load_tracks(InsertPosition::Start, &query)?;
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

        Command::Queue { command, query } => {
            if let Some(command) = command {
                handle_queue_command(command)?;
            } else if let Some(query) = query {
                handle_queue_command(QueueCommand::Add { query })?;
            } else {
                print_queue()?;
            }
        }

        Command::Update => {
            send_request(requests::UpdateDatabase)?;
        }
    }

    Ok(())
}
