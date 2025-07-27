use tokio::signal;
use tokio::sync::watch;
use std::time::Duration;

/// Creates a shutdown signal that listens for SIGINT, SIGTERM, and manual triggers
/// Returns (sender, receiver) where sender can manually trigger shutdown
pub fn create_shutdown_signal() -> (watch::Sender<bool>, watch::Receiver<bool>) {
    let (tx, rx) = watch::channel(false);
    
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                tracing::info!("Received SIGINT (Ctrl+C), initiating graceful shutdown");
            },
            _ = terminate => {
                tracing::info!("Received SIGTERM, initiating graceful shutdown");
            },
        }

        if let Err(_) = tx_clone.send(true) {
            tracing::error!("Failed to send shutdown signal - receiver may have been dropped");
        }
    });

    (tx, rx)
}

/// Waits for shutdown with a timeout, returns true if shutdown completed gracefully
pub async fn wait_for_shutdown_with_timeout(
    shutdown_future: impl std::future::Future<Output = ()>,
    timeout_secs: u64,
) -> bool {
    let timeout = Duration::from_secs(timeout_secs);
    
    match tokio::time::timeout(timeout, shutdown_future).await {
        Ok(_) => {
            tracing::info!("Graceful shutdown completed successfully");
            true
        }
        Err(_) => {
            tracing::error!("Shutdown timeout after {} seconds", timeout_secs);
            false
        }
    }
}

/// Helper to wait for multiple tasks to complete gracefully
pub async fn wait_for_tasks(tasks: Vec<tokio::task::JoinHandle<()>>) {
    for (i, task) in tasks.into_iter().enumerate() {
        if let Err(e) = task.await {
            tracing::warn!("Task {} failed during shutdown: {}", i, e);
        }
    }
}

/// Unified shutdown coordinator for applications with multiple async tasks
pub struct ShutdownCoordinator {
    shutdown_tx: watch::Sender<bool>,
    shutdown_rx: watch::Receiver<bool>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl ShutdownCoordinator {
    pub fn new() -> Self {
        let (shutdown_tx, shutdown_rx) = create_shutdown_signal();
        Self {
            shutdown_tx,
            shutdown_rx,
            tasks: Vec::new(),
        }
    }

    /// Get a receiver for monitoring shutdown signals
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.shutdown_rx.clone()
    }

    /// Manually trigger shutdown
    pub fn trigger_shutdown(&self) {
        if let Err(_) = self.shutdown_tx.send(true) {
            tracing::error!("Failed to trigger shutdown - receiver may have been dropped");
        }
    }

    /// Add a task to be monitored for completion during shutdown
    pub fn add_task(&mut self, task: tokio::task::JoinHandle<()>) {
        self.tasks.push(task);
    }

    /// Wait for shutdown signal and coordinate graceful shutdown
    pub async fn wait_for_shutdown(self, timeout_secs: u64) -> bool {
        let mut shutdown_rx = self.shutdown_rx.clone();
        
        // Wait for shutdown signal
        if let Err(e) = shutdown_rx.changed().await {
            tracing::error!("Error waiting for shutdown signal: {}", e);
            return false;
        }

        tracing::info!("Shutdown signal received, coordinating graceful shutdown...");

        // Trigger shutdown for all components
        self.trigger_shutdown();

        // Wait for all tasks to complete
        let shutdown_future = async {
            wait_for_tasks(self.tasks).await;
            tracing::info!("All tasks completed");
        };

        wait_for_shutdown_with_timeout(shutdown_future, timeout_secs).await
    }
}
