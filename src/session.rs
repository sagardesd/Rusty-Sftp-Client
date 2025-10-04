use anyhow::anyhow;
use openssh;
use openssh_sftp_client::{Sftp, SftpOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use crate::client::SftpClient;
use crate::types::SftpClientConfig;
use crate::utils::check_connection;

/// Manager for SSH sessions that creates SFTP clients
#[derive(Debug, Default)]
pub struct SftpSessionManager {
    pub session: Option<Arc<openssh::Session>>,
}

impl SftpSessionManager {
    /// Establishes a new SSH connection to the remote host
    ///
    /// # Arguments
    ///
    /// * `host` - The hostname or IP address of the remote server
    /// * `username` - The username for SSH authentication
    /// * `control_dir` - Directory for SSH control sockets
    /// * `ssh_key_path` - Path to the private SSH key file
    ///
    /// # Returns
    ///
    /// Returns a new `SftpSessionManager` with an active SSH session
    ///
    /// # Example
    ///
    /// ```ignore
    /// let manager = SftpSessionManager::connect(
    ///     "example.com",
    ///     "user",
    ///     PathBuf::from("/tmp/ssh_control"),
    ///     PathBuf::from("/home/user/.ssh/id_rsa"),
    /// ).await?;
    /// ```
    pub async fn connect(
        host: &str,
        username: &str,
        control_dir: PathBuf,
        ssh_key_path: PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("Connecting to {:?}:{:?}", username, host);
        let session = openssh::SessionBuilder::default()
            .control_directory(&control_dir)
            .keyfile(&ssh_key_path)
            .known_hosts_check(openssh::KnownHosts::Accept)
            .connect_timeout(Duration::from_secs(60))
            .connect(format!("ssh://{}@{}", username, host))
            .await?;
        Ok(Self {
            session: Some(Arc::new(session)),
        })
    }

    /// Creates a new SFTP client from the managed SSH session
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the SFTP client (buffer size, concurrency)
    ///
    /// # Returns
    ///
    /// Returns a new `SftpClient` instance that can perform SFTP operations
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The SSH session is not connected
    /// - The session check fails (connection is dead)
    /// - SFTP subsystem cannot be initialized
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = manager.create_sftp_client(SftpClientConfig {
    ///     io_size: 32_768,
    ///     concurrency: 8,
    /// }).await?;
    /// ```
    pub async fn create_sftp_client(
        &self,
        config: SftpClientConfig,
    ) -> Result<SftpClient, anyhow::Error> {
        debug!("Creating sftp client from session");
        let session = self
            .session
            .as_ref()
            .ok_or(anyhow!("SSH session not connected"))?;
        session
            .check()
            .await
            .map_err(|_| anyhow!("sftp session is already closed"))?;

        let sftp = Sftp::from_clonable_session_with_check_connection(
            session.clone(),
            SftpOptions::default(),
            check_connection, /* if the ssh connection is dropped this sftp client can notice and fail the ongoing operation */
        )
        .await?;
        debug!("sftp client created successfully");
        Ok(SftpClient::new(sftp, config))
    }

    /// Closes the SSH session if no SFTP clients are using it
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the session was successfully closed, or an error if:
    /// - Some SFTP clients still hold references to the session
    /// - The session was already closed
    /// - There was an error closing the session
    ///
    /// # Note
    ///
    /// This method uses `Arc::try_unwrap` to ensure all SFTP clients have been
    /// dropped before closing the session. If any clients still exist, the
    /// session will not be closed and an error will be returned.
    pub async fn close(&mut self) -> Result<(), anyhow::Error> {
        if let Some(session) = self.session.take() {
            match Arc::try_unwrap(session) {
                Ok(session) => {
                    info!("No sftp client is using the session anymore so can close the session");
                    session.close().await?;
                    Ok(())
                }
                Err(session) => {
                    // Put it back if we couldn't close it
                    error!(
                        "Some sftp client still has the session instance so could not close session"
                    );
                    self.session = Some(session);
                    Err(anyhow!("failed to close ssh session"))
                }
            }
        } else {
            error!("Session not found");
            Err(anyhow!("failed to close ssh session"))
        }
    }

    /// Checks if the SSH session is still active
    ///
    /// # Returns
    ///
    /// Returns `true` if the session is active and responsive, `false` otherwise.
    /// If the session is found to be dead, it will be removed from the manager.
    pub async fn connected(&mut self) -> bool {
        match self.session.as_mut() {
            Some(session) => {
                if session.check().await.is_ok() {
                    true
                } else {
                    warn!("Underlying ssh session is dead so setting sftp status to disconnected");
                    self.session = None;
                    false
                }
            }
            None => false,
        }
    }
}
