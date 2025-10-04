// examples/basic_usage.rs
// Run with: cargo run --example basic_usage

use anyhow;
use rusty_sftp::{
    CancellationToken, FileTransferOperationResult, SftpClientConfig, SftpSessionManager,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Connect to SSH server
    let mut session_manager: SftpSessionManager = SftpSessionManager::connect(
        "example.com",                           // hostname
        "your_username",                         // SSH username
        PathBuf::from("/tmp/ssh_control"),       // control directory
        PathBuf::from("/home/user/.ssh/id_rsa"), // path to SSH private key
    )
    .await
    .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;

    println!("✅ Connected to SSH server");

    // 2. Create SFTP client with custom configuration
    let client = session_manager
        .create_sftp_client(SftpClientConfig {
            io_size: 65_536, // 64KB buffer size
            concurrency: 10, // 10 concurrent operations
        })
        .await?;

    println!("✅ SFTP client created");

    // 3. Create cancellation token for operations
    let cancel_token = CancellationToken::new();

    // 4. List files in remote directory
    println!("\n📂 Listing files in /remote/directory...");
    let files = client
        .ls("/remote/directory".to_string(), cancel_token.clone())
        .await?;

    for file in &files {
        println!("  - {:?} ({} bytes)", file.path, file.size.unwrap_or(0));
    }
    println!("Found {} files", files.len());

    // 5. Upload a file
    println!("\n⬆️  Uploading file...");
    let upload_result = client
        .put(
            "/local/path/document.pdf".to_string(),
            "/remote/path/document.pdf".to_string(),
            cancel_token.clone(),
        )
        .await?;

    match upload_result {
        FileTransferOperationResult::Completed(progress) => {
            println!(
                "✅ Upload completed: {} bytes transferred",
                progress.file_size
            );
        }
        FileTransferOperationResult::Cancelled {
            src_file,
            dest_file,
        } => {
            println!("❌ Upload cancelled: {} -> {}", src_file, dest_file);
        }
        FileTransferOperationResult::InProgress(progress) => {
            println!(
                "⏳ Upload in progress: {:.2}%",
                progress.percentage_progress
            );
        }
    }

    // 6. Download a file
    println!("\n⬇️  Downloading file...");
    let download_result = client
        .get(
            "/remote/path/config.json".to_string(),
            "/local/path/config.json".to_string(),
            cancel_token.clone(),
        )
        .await?;

    match download_result {
        FileTransferOperationResult::Completed(progress) => {
            println!(
                "✅ Download completed: {} bytes transferred",
                progress.file_size
            );
        }
        FileTransferOperationResult::Cancelled {
            src_file,
            dest_file,
        } => {
            println!("❌ Download cancelled: {} -> {}", src_file, dest_file);
        }
        FileTransferOperationResult::InProgress(progress) => {
            println!(
                "⏳ Download in progress: {:.2}%",
                progress.percentage_progress
            );
        }
    }

    // 7. Cleanup
    println!("\n🧹 Cleaning up...");
    client.close().await?;
    session_manager.close().await?;

    println!("✅ All done!");

    Ok(())
}
