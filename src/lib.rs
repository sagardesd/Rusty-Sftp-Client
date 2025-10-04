// Module declarations
mod client;
mod operations;
mod session;
mod types;
mod utils;

// Public API exports
pub use client::SftpClient;
pub use session::SftpSessionManager;
pub use types::{
    FileMetadata, FileTransferOperationResult, FileTransferProgress, FileType, SftpClientConfig,
};

// Re-export commonly used external types for convenience
pub use tokio_util::sync::CancellationToken;
