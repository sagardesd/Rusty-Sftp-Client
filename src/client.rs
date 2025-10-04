use openssh_sftp_client::Sftp;
use tokio_util::sync::CancellationToken;

use crate::operations::{download, list, upload};
use crate::types::{FileMetadata, FileTransferOperationResult, SftpClientConfig, SftpClientConfigArc};

/// SFTP client for performing file operations on a remote server
#[derive(Debug)]
pub struct SftpClient {
    pub(crate) sftp: Sftp,
    pub(crate) config: SftpClientConfigArc,
}

impl SftpClient {
    /// Creates a new SFTP client instance (internal use)
    pub(crate) fn new(sftp: Sftp, config: SftpClientConfig) -> Self {
        Self {
            sftp,
            config: config.into(),
        }
    }

    /// Closes the SFTP client and releases resources
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful close, or an error if the close operation fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// client.close().await?;
    /// ```
    pub async fn close(self) -> Result<(), anyhow::Error> {
        self.sftp.close().await?;
        Ok(())
    }

    /// Lists the contents of a remote directory
    ///
    /// # Arguments
    ///
    /// * `remote_dir` - Path to the remote directory
    /// * `cancel_token` - Token for cancelling the operation
    ///
    /// # Returns
    ///
    /// Returns a vector of `FileMetadata` for files in the directory
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cancel_token = CancellationToken::new();
    /// let files = client.ls("/remote/path".to_string(), cancel_token).await?;
    /// for file in files {
    ///     println!("{:?}: {} bytes", file.path, file.size.unwrap_or(0));
    /// }
    /// ```
    pub async fn ls(
        &self,
        remote_dir: String,
        cancel_token: CancellationToken,
    ) -> Result<Vec<FileMetadata>, anyhow::Error> {
        list::ls(self, remote_dir, cancel_token).await
    }

    /// Uploads a local file to the remote server
    ///
    /// # Arguments
    ///
    /// * `local_path` - Path to the local file
    /// * `remote_path` - Destination path on the remote server
    /// * `cancel_token` - Token for cancelling the upload
    ///
    /// # Returns
    ///
    /// Returns a `FileTransferOperationResult` indicating completion or cancellation
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cancel_token = CancellationToken::new();
    /// let result = client.put(
    ///     "/local/file.txt".to_string(),
    ///     "/remote/file.txt".to_string(),
    ///     cancel_token,
    /// ).await?;
    /// ```
    pub async fn put(
        &self,
        local_path: String,
        remote_path: String,
        cancel_token: CancellationToken,
    ) -> Result<FileTransferOperationResult, anyhow::Error> {
        upload::put(self, local_path, remote_path, cancel_token).await
    }

    /// Downloads a file from the remote server to local storage
    ///
    /// # Arguments
    ///
    /// * `remote_path` - Path to the remote file
    /// * `local_path` - Local destination path
    /// * `cancel_token` - Token for cancelling the download
    ///
    /// # Returns
    ///
    /// Returns a `FileTransferOperationResult` indicating completion or cancellation
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cancel_token = CancellationToken::new();
    /// let result = client.get(
    ///     "/remote/file.txt".to_string(),
    ///     "/local/file.txt".to_string(),
    ///     cancel_token,
    /// ).await?;
    /// ```
    pub async fn get(
        &self,
        remote_path: String,
        local_path: String,
        cancel_token: CancellationToken,
    ) -> Result<FileTransferOperationResult, anyhow::Error> {
        download::get(self, remote_path, local_path, cancel_token).await
    }
}
