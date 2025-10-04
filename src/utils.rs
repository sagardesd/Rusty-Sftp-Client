use std::time::Duration;

/// Routine to check the underlying SSH connection is active or not for the SFTP client.
/// This function runs in a loop, checking the connection every 10 seconds.
/// It will continue indefinitely until the connection fails.
pub fn check_connection<'session>(
    session: &'session openssh::Session,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<(), openssh::Error>> + Send + Sync + 'session>,
> {
    Box::pin(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            session.check().await?;
        }
        #[allow(unreachable_code)]
        Ok(())
    })
}
