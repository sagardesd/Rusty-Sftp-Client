use anyhow::anyhow;
use bytes::BytesMut;
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;
use tokio::fs;
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::client::SftpClient;
use crate::types::{FileTransferOperationResult, FileTransferProgress};

/// Downloads a file from the remote server to local storage
///
/// This function implements concurrent download with ordered writes:
/// 1. Reads chunks from the remote file concurrently
/// 2. Sends chunks to a writer task via MPSC channel with sequence numbers
/// 3. Writer task maintains ordering using a buffer map
/// 4. Automatically creates parent directories for the local file
/// 5. Supports graceful cancellation at any point
///
/// # Arguments
///
/// * `client` - The SFTP client instance
/// * `remote_path` - Path to the remote file to download
/// * `local_path` - Local destination path
/// * `cancel_token` - Token for cancelling the download operation
///
/// # Returns
///
/// Returns a `FileTransferOperationResult`:
/// - `Completed` with transfer progress if successful
/// - `Cancelled` if the operation was cancelled
///
/// # Errors
///
/// Returns an error if:
/// - The remote file cannot be opened or read
/// - Parent directories for the local file cannot be created
/// - The local file cannot be created or written to
/// - There's a channel communication error
/// - An SFTP error occurs during transfer
pub async fn get(
    client: &SftpClient,
    remote_path: String,
    local_path: String,
    cancel_token: CancellationToken,
) -> Result<FileTransferOperationResult, anyhow::Error> {
    let local_path_copy = local_path.clone();
    let download_time = Instant::now();
    let mut is_cancelled: bool = false;
    let mut remote_file = client.sftp.open(remote_path.clone()).await?;
    info!("Remote file opened: {:?}", remote_path);
    let remote_file_size = remote_file.metadata().await?.len();

    let (tx, mut rx) = mpsc::channel::<(usize, Vec<u8>)>(client.config.concurrency);

    let mut buffer_idx = 0;
    let mut tasks = FuturesUnordered::new();

    // Buffer map to ensure ordering in writing to the local file
    let mut buffer_map = HashMap::new();
    let mut current_idx = 0;

    // Writing to local file
    let write_handle: JoinHandle<Result<(), io::Error>> = tokio::spawn(async move {
        if let Some(parent) = Path::new(&local_path).parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("Failed to create parent directory: {:?}", e),
                )
            })?;
        }
        let mut local_file = fs::File::create(local_path.clone()).await?;
        info!("Local file created: {:?}", local_path);

        while let Some((idx, buffer)) = rx.recv().await {
            buffer_map.insert(idx, buffer);

            while let Some(buffer) = buffer_map.remove(&current_idx) {
                local_file.write_all(&buffer).await?;
                current_idx += 1;
            }
        }
        Ok(())
    });

    // Reading from Remote file
    let mut download_error: Option<anyhow::Error> = None;
    loop {
        let buffer = BytesMut::with_capacity(client.config.io_size);
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("Operation cancelled by user");
                is_cancelled = true;
                break;
            }
            read_result = remote_file.read(client.config.io_size as u32, buffer) => {
                let buf = match read_result {
                    Ok(Some(buf)) => buf,
                    Ok(None) => {
                        info!("End of remote file reached");
                        break;
                    }
                    Err(e) => {
                        error!("Error reading remote file: {:?}", e);
                        download_error = Some(anyhow!("Error reading from remote file: {e}"));
                        break;
                    }
                };
                let tx = tx.clone();
                let data = buf[..buf.len()].to_vec();
                tasks.push(tokio::spawn(async move {
                    tx.send((buffer_idx, data)).await.map_err(|_| {
                        io::Error::new(io::ErrorKind::BrokenPipe, "Failed to send buffer")
                    })
                }));
                buffer_idx += 1;
                if tasks.len() >= client.config.concurrency {
                    tasks.select_next_some().await??;
                }
            }
        }
    }

    drop(tx);
    while let Some(task) = tasks.next().await {
        task??;
    }
    write_handle.await??;

    // Intermediate remote file read error causing the read loop to terminate
    if let Some(err) = download_error {
        return Err(err);
    }

    let time_taken = download_time.elapsed();
    info!(
        "File {:?} downloaded. Time taken {:?}",
        remote_path, time_taken,
    );

    if is_cancelled {
        Ok(FileTransferOperationResult::Cancelled {
            src_file: remote_path.clone(),
            dest_file: local_path_copy.clone(),
        })
    } else {
        Ok(FileTransferOperationResult::Completed(
            FileTransferProgress {
                src_file: remote_path.clone(),
                dest_file: local_path_copy.clone(),
                file_size: remote_file_size.unwrap(),
                percentage_progress: 100.0_f64,
            },
        ))
    }
}
