// examples/advanced_usage.rs
// Run with: cargo run --example advanced_usage

use anyhow;
use rusty_sftp::{
    CancellationToken, FileTransferOperationResult, SftpClientConfig, SftpSessionManager,
};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Multiple file uploads with progress tracking
    example_batch_upload().await?;

    // Example 2: Download with timeout and cancellation
    example_download_with_timeout().await?;

    // Example 3: Reusing session for multiple clients
    example_multiple_clients().await?;

    // Example 4: Error handling and retry logic
    example_with_retry().await?;

    Ok(())
}

/// Example 1: Upload multiple files in parallel
async fn example_batch_upload() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 1: Batch Upload ===");

    let mut manager: SftpSessionManager = SftpSessionManager::connect(
        "example.com",
        "user",
        PathBuf::from("/tmp/ssh_control"),
        PathBuf::from("/home/user/.ssh/id_rsa"),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;

    let client = manager
        .create_sftp_client(SftpClientConfig {
            io_size: 65_536,
            concurrency: 10,
        })
        .await?;

    let files_to_upload = vec![
        ("/local/file1.txt", "/remote/file1.txt"),
        ("/local/file2.txt", "/remote/file2.txt"),
        ("/local/file3.txt", "/remote/file3.txt"),
    ];

    let mut handles = vec![];

    for (local, remote) in files_to_upload {
        let client_clone = &client;
        let cancel_token = CancellationToken::new();

        let handle = tokio::spawn(async move {
            let result = client_clone
                .put(local.to_string(), remote.to_string(), cancel_token)
                .await;

            match result {
                Ok(FileTransferOperationResult::Completed(progress)) => {
                    println!("✅ Uploaded {}: {} bytes", local, progress.file_size);
                }
                Ok(FileTransferOperationResult::Cancelled { .. }) => {
                    println!("❌ Cancelled {}", local);
                }
                Err(e) => {
                    println!("❌ Failed {}: {}", local, e);
                }
                _ => {}
            }
        });

        handles.push(handle);
    }

    // Wait for all uploads to complete
    for handle in handles {
        handle.await?;
    }

    client.close().await?;
    manager.close().await?;

    Ok(())
}

/// Example 2: Download with timeout
async fn example_download_with_timeout() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 2: Download with Timeout ===");

    let mut manager: SftpSessionManager = SftpSessionManager::connect(
        "example.com",
        "user",
        PathBuf::from("/tmp/ssh_control"),
        PathBuf::from("/home/user/.ssh/id_rsa"),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;

    let client = manager
        .create_sftp_client(SftpClientConfig::default())
        .await?;

    let cancel_token = CancellationToken::new();
    let cancel_token_clone = cancel_token.clone();

    // Spawn a task to cancel after timeout
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        cancel_token_clone.cancel();
        println!("⏰ Timeout reached, cancelling download...");
    });

    // Try to download with 30 second timeout
    match timeout(
        Duration::from_secs(30),
        client.get(
            "/remote/large_file.bin".to_string(),
            "/local/large_file.bin".to_string(),
            cancel_token,
        ),
    )
    .await
    {
        Ok(Ok(FileTransferOperationResult::Completed(progress))) => {
            println!("✅ Downloaded {} bytes", progress.file_size);
        }
        Ok(Ok(FileTransferOperationResult::Cancelled { .. })) => {
            println!("❌ Download was cancelled");
        }
        Ok(Err(e)) => {
            println!("❌ Download failed: {}", e);
        }
        Err(_) => {
            println!("❌ Download timed out");
        }
        _ => {}
    }

    client.close().await?;
    manager.close().await?;

    Ok(())
}

/// Example 3: Create multiple SFTP clients from one session
async fn example_multiple_clients() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 3: Multiple Clients ===");

    let mut manager: SftpSessionManager = SftpSessionManager::connect(
        "example.com",
        "user",
        PathBuf::from("/tmp/ssh_control"),
        PathBuf::from("/home/user/.ssh/id_rsa"),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;

    // Create multiple clients with different configurations
    let upload_client = manager
        .create_sftp_client(SftpClientConfig {
            io_size: 131_072, // 128KB for uploads
            concurrency: 16,
        })
        .await?;

    let download_client = manager
        .create_sftp_client(SftpClientConfig {
            io_size: 65_536, // 64KB for downloads
            concurrency: 8,
        })
        .await?;

    let cancel_token = CancellationToken::new();

    // Use clients concurrently
    let upload_handle = tokio::spawn(async move {
        upload_client
            .put(
                "/local/upload.dat".to_string(),
                "/remote/upload.dat".to_string(),
                cancel_token.clone(),
            )
            .await
    });

    let download_handle = tokio::spawn(async move {
        download_client
            .get(
                "/remote/download.dat".to_string(),
                "/local/download.dat".to_string(),
                cancel_token.clone(),
            )
            .await
    });

    // Wait for both operations
    let (upload_result, download_result) = tokio::join!(upload_handle, download_handle);

    match upload_result {
        Ok(Ok(FileTransferOperationResult::Completed(_))) => {
            println!("✅ Upload completed");
        }
        _ => println!("❌ Upload failed or cancelled"),
    }

    match download_result {
        Ok(Ok(FileTransferOperationResult::Completed(_))) => {
            println!("✅ Download completed");
        }
        _ => println!("❌ Download failed or cancelled"),
    }

    manager.close().await?;

    Ok(())
}

/// Example 4: Retry logic for failed operations
async fn example_with_retry() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 4: Retry Logic ===");

    let mut manager: SftpSessionManager = SftpSessionManager::connect(
        "example.com",
        "user",
        PathBuf::from("/tmp/ssh_control"),
        PathBuf::from("/home/user/.ssh/id_rsa"),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;

    let client = manager
        .create_sftp_client(SftpClientConfig::default())
        .await?;

    let max_retries = 3;
    let mut attempt = 0;

    loop {
        attempt += 1;
        println!("Attempt {}/{}", attempt, max_retries);

        let cancel_token = CancellationToken::new();
        let result = client
            .put(
                "/local/important.dat".to_string(),
                "/remote/important.dat".to_string(),
                cancel_token,
            )
            .await;

        match result {
            Ok(FileTransferOperationResult::Completed(progress)) => {
                println!("✅ Upload succeeded: {} bytes", progress.file_size);
                break;
            }
            Ok(FileTransferOperationResult::Cancelled { .. }) => {
                println!("❌ Upload was cancelled, not retrying");
                break;
            }
            Err(e) => {
                if attempt >= max_retries {
                    println!("❌ Upload failed after {} attempts: {}", max_retries, e);
                    break;
                } else {
                    println!("⚠️  Upload failed: {}, retrying...", e);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
            _ => {}
        }
    }

    client.close().await?;
    manager.close().await?;

    Ok(())
}

/// Example 5: Check connection status
async fn example_connection_check() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 5: Connection Check ===");

    let mut manager: SftpSessionManager = SftpSessionManager::connect(
        "example.com",
        "user",
        PathBuf::from("/tmp/ssh_control"),
        PathBuf::from("/home/user/.ssh/id_rsa"),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;

    // Periodically check connection
    for i in 1..=5 {
        println!(
            "Check {}: Connection is {}",
            i,
            if manager.connected().await {
                "alive ✅"
            } else {
                "dead ❌"
            }
        );
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    manager.close().await?;

    Ok(())
}

/// Example 6: List directory recursively
async fn example_recursive_list() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Example 6: Recursive Directory Listing ===");

    let mut manager: SftpSessionManager = SftpSessionManager::connect(
        "example.com",
        "user",
        PathBuf::from("/tmp/ssh_control"),
        PathBuf::from("/home/user/.ssh/id_rsa"),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Connection failed: {}", e))?;

    let client = manager
        .create_sftp_client(SftpClientConfig::default())
        .await?;

    async fn list_recursive(
        client: &rusty_sftp::SftpClient,
        path: &str,
        depth: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let cancel_token = CancellationToken::new();
        let files = client.ls(path.to_string(), cancel_token).await?;

        for file in files {
            let indent = "  ".repeat(depth);
            println!(
                "{}{:?} ({} bytes)",
                indent,
                file.path,
                file.size.unwrap_or(0)
            );

            // If you want to recurse into directories, you would do it here
            // Note: Current implementation only lists regular files
        }

        Ok(())
    }

    list_recursive(&client, "/remote/directory", 0).await?;

    client.close().await?;
    manager.close().await?;

    Ok(())
}
