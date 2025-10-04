use anyhow::anyhow;
use futures::stream::{FuturesUnordered, StreamExt};
use std::collections::HashMap;
use std::time::Instant;
use tokio::fs;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::client::SftpClient;
use crate::types::{FileTransferOperationResult, FileTransferProgress};

/// Uploads a local file to the remote server
///
/// This function implements concurrent upload with ordered writes:
/// 1. Reads chunks from the local file
/// 2. Sends chunks to a writer task via MPSC channel with sequence numbers
/// 3. Writer task maintains ordering using a buffer map
/// 4. Supports graceful cancellation at any point
///
/// # Arguments
///
/// * `client` - The SFTP client instance
/// * `local_path` - Path to the local file to upload
/// * `remote_path` - Destination path on the remote server
/// * `cancel_token` - Token for cancelling the upload operation
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
/// - The local file cannot be opened or read
/// - The remote file cannot be created or written to
/// - There's a channel communication error
/// - An SFTP error occurs during transfer
pub async fn put(
    client: &SftpClient,
    local_path: String,
    remote_path: String,
    cancel_token: CancellationToken,
) -> Result<FileTransferOperationResult, anyhow::Error> {
    let upload_time = Instant::now();
    let mut is_cancelled: bool = false;
    let mut local_file = fs::File::open(local_path.clone()).await?;
    info!("Local file opened: {:?}", local_path);
    let local_file_size = local_file.metadata().await?.len();

    let (tx, mut rx) = mpsc::channel::<(usize, Vec<u8>)>(client.config.concurrency);

    let mut buffer_idx = 0;
    let mut tasks = FuturesUnordered::new();

    let mut remote_file = client.sftp.create(remote_path.clone()).await.map_err(|err| {
        info!(
            "Failed to open file: {:?} ERROR: {:?}",
            remote_path.clone(),
            err
        );
        io::Error::new(io::ErrorKind::Other, format!("SFTP error: {:?}", err))
    })?;
    info!("Remote file created path: {:?}", remote_path.clone());

    let stop_transmission = CancellationToken::new();
    let stop_transmission_child = stop_transmission.child_token();

    // Spawn a task to handle ordered writing
    let write_handle = tokio::spawn(async move {
        let mut current_idx = 0;
        let mut buffer_map = HashMap::new();

        while let Some((idx, buffer)) = rx.recv().await {
            if stop_transmission_child.is_cancelled() {
                warn!("transmission cancelled by reader task");
                continue;
            }
            buffer_map.insert(idx, buffer);
            while let Some(buffer) = buffer_map.remove(&current_idx) {
                remote_file.write_all(&buffer).await.map_err(|e| {
                    io::Error::new(io::ErrorKind::Other, format!("SFTP write error: {:?}", e))
                })?;
                let _bytes_written = buffer.len() as u64;
                current_idx += 1;
            }
        }
        Ok::<(), anyhow::Error>(())
    });

    // Task to read from local file
    let mut upload_error: Option<anyhow::Error> = None;
    loop {
        let mut buffer = vec![0; client.config.io_size];
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("Upload operation cancelled by user");
                is_cancelled = true;
                stop_transmission.cancel();
                break;
            }
            read_result = local_file.read(&mut buffer[..]) => {
                let bytes_read = match read_result {
                    Ok(0) => {
                        info!("Upload: End of file reached");
                        break;
                    },
                    Ok(n) => n,
                    Err(e) => {
                        error!("Error reading local file: {:?}", e);
                        upload_error = Some(anyhow!("Failed to read from remote file: {e}"));
                        break;
                    }
                };
                let tx = tx.clone();
                let buffer = buffer[..bytes_read].to_vec();
                tasks.push(tokio::spawn(async move {
                    tx.send((buffer_idx, buffer))
                        .await
                        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "Failed to send buffer"))
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
        warn!("Waiting tasks to finish");
        task??;
    }
    write_handle.await??;

    if let Some(err) = upload_error {
        return Err(err);
    }

    let time_taken = upload_time.elapsed();
    info!(
        "File {:?} uploaded. Time taken {:?}",
        local_path, time_taken,
    );

    if is_cancelled {
        Ok(FileTransferOperationResult::Cancelled {
            src_file: local_path.clone(),
            dest_file: remote_path.clone(),
        })
    } else {
        Ok(FileTransferOperationResult::Completed(
            FileTransferProgress {
                src_file: local_path.clone(),
                dest_file: remote_path.clone(),
                file_size: local_file_size,
                percentage_progress: 100.0_f64,
            },
        ))
    }
}
