use std::path::PathBuf;
use std::sync::Arc;

/// Metadata information for a file
#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub size: Option<u64>,
    pub file_type: FileType,
    pub last_accessed_at: Option<std::time::SystemTime>,
    pub last_modified_at: Option<std::time::SystemTime>,
}

/// Type of file (regular file or directory)
#[derive(Debug, Clone)]
pub enum FileType {
    Regular,
    Directory,
}

/// Configuration for SFTP client operations
#[derive(Debug)]
pub struct SftpClientConfig {
    /// Buffer size for read/write operations in bytes
    pub io_size: usize,
    /// Number of concurrent operations allowed
    pub concurrency: usize,
}

impl SftpClientConfig {
    /// Creates a new configuration with default values
    /// - io_size: 65536 (64KB)
    /// - concurrency: 8
    pub fn default() -> Self {
        Self {
            io_size: 65536,
            concurrency: 8,
        }
    }

    /// Creates a new configuration with custom values
    pub fn new(io_size: usize, concurrency: usize) -> Self {
        Self {
            io_size,
            concurrency,
        }
    }
}

/// Result of a file transfer operation
#[derive(Debug, Clone)]
pub enum FileTransferOperationResult {
    /// Transfer completed successfully
    Completed(FileTransferProgress),
    /// Transfer was cancelled by user
    Cancelled { src_file: String, dest_file: String },
    /// Transfer is currently in progress
    InProgress(FileTransferProgress),
}

/// Progress information for an ongoing or completed file transfer
#[derive(Debug, Clone)]
pub struct FileTransferProgress {
    /// Source file path
    pub src_file: String,
    /// Destination file path
    pub dest_file: String,
    /// Total size of the file in bytes
    pub file_size: u64,
    /// Percentage of transfer completed (0.0 to 100.0)
    pub percentage_progress: f64,
}

/// Internal configuration wrapper with Arc for shared ownership
#[derive(Debug, Clone)]
pub(crate) struct SftpClientConfigArc {
    pub(crate) inner: Arc<SftpClientConfig>,
}

impl From<SftpClientConfig> for SftpClientConfigArc {
    fn from(config: SftpClientConfig) -> Self {
        Self {
            inner: Arc::new(config),
        }
    }
}

impl std::ops::Deref for SftpClientConfigArc {
    type Target = SftpClientConfig;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
