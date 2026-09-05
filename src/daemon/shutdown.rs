use std::time::Duration;

use tokio::sync::watch;

/// Graceful shutdown coordinator
#[allow(dead_code)]
pub struct Shutdown {
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
}

#[allow(dead_code)]
impl Shutdown {
    pub fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        Self {
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Signal shutdown
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    /// Wait for shutdown signal
    pub async fn wait(&mut self) {
        let _ = self.shutdown_rx.changed().await;
    }

    /// Wait for shutdown with timeout
    pub async fn wait_timeout(&mut self, timeout: Duration) -> bool {
        tokio::time::timeout(timeout, self.shutdown_rx.changed())
            .await
            .is_ok()
    }

    /// Check if shutdown has been requested
    pub fn is_shutdown(&self) -> bool {
        *self.shutdown_rx.borrow()
    }
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(unix)]
#[allow(dead_code)]
pub async fn setup_signal_handlers(shutdown: std::sync::Arc<Shutdown>) -> anyhow::Result<()> {
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;

    tokio::spawn(async move {
        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM, shutting down gracefully...");
            }
            _ = sigint.recv() => {
                tracing::info!("Received SIGINT, shutting down gracefully...");
            }
            _ = sighup.recv() => {
                tracing::info!("Received SIGHUP, shutting down gracefully...");
            }
        }
        shutdown.shutdown();
    });

    Ok(())
}
