use anyhow::anyhow;
use futures::stream::StreamExt;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::client::SftpClient;
use crate::types::{FileMetadata, FileType};

/// Lists the contents of a remote directory
///
/// # Arguments
///
/// * `client` - The SFTP client instance
/// * `remote_dir` - Path to the remote directory
/// * `cancel_token` - Token for cancelling the operation
///
/// # Returns
///
/// Returns a vector of `FileMetadata` for all regular files in the directory.
/// Note: This implementation currently only includes regular files, not directories.
///
/// # Errors
///
/// Returns an error if:
/// - The remote directory cannot be opened
/// - There's an error reading directory entries
/// - The operation is cancelled by the user
pub async fn ls(
    client: &SftpClient,
    remote_dir: String,
    cancel_token: CancellationToken,
) -> Result<Vec<FileMetadata>, anyhow::Error> {
    let mut file_list = Vec::new();
    let dir = client
        .sftp
        .fs()
        .open_dir(remote_dir.clone())
        .await
        .map_err(|e| anyhow!("Failed to open remote dir: {}", e))?;

    let dir_stream = dir.read_dir();
    futures::pin_mut!(dir_stream);
    let mut error: Option<anyhow::Error> = None;

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                info!("ls operation cancelled by user");
                break;
            }
            entry_opt = dir_stream.next() => {
                match entry_opt {
                    Some(Ok(entry)) => {
                        let file_type = entry.file_type();
                        let file_name = entry.filename().file_name();
                        if let (Some(file_type), Some(file_name)) = (file_type, file_name) {
                            // Currently only listing regular files
                            if file_type.is_file() {
                                let metadata = entry.metadata();
                                file_list.push(
                                    FileMetadata {
                                        path: PathBuf::from(remote_dir.clone()).join(file_name.to_string_lossy().as_ref()),
                                        file_type: if file_type.is_dir() {
                                            FileType::Directory
                                        } else {
                                            FileType::Regular
                                        },
                                        size: metadata.len(),
                                        last_accessed_at: metadata.accessed().map(|t| t.as_system_time()),
                                        last_modified_at: metadata.modified().map(|t| t.as_system_time())
                                    }
                                );
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error = Some(anyhow!("Failed to ls to remote directory: {e}"));
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    if let Some(e) = error {
        Err(e)
    } else {
        Ok(file_list)
    }
}
