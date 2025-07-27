use kc::{download_from_kemono_url, start_kemono_ingest_loop};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use shutdown_utils::ShutdownCoordinator;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::watch;

const KC_LINKS_DIR: &str = "./exchange/kc-links";

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Starting Kemono/Coomer Ingester...");

    // Create kc directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(KC_LINKS_DIR) {
        tracing::error!("Failed to create kc directory: {}", e);
        return;
    }

    // Create shutdown coordinator
    let mut shutdown_coordinator = ShutdownCoordinator::new();
    let shutdown_rx = shutdown_coordinator.subscribe();

    // Spawn periodic artist ingestion loop
    let shutdown_rx_clone = shutdown_rx.clone();
    let artist_loop_task = tokio::spawn(async move {
        start_kemono_ingest_loop(shutdown_rx_clone).await;
    });

    // Spawn URL file monitoring task
    let shutdown_rx_clone = shutdown_rx.clone();
    let file_monitor_task = tokio::spawn(async move {
        monitor_kemono_links(shutdown_rx_clone).await;
    });

    // Add tasks to coordinator
    shutdown_coordinator.add_task(artist_loop_task);
    shutdown_coordinator.add_task(file_monitor_task);

    tracing::info!("Kemono/Coomer Ingester is running. Press Ctrl+C to stop.");

    // Wait for shutdown with 15 second timeout
    let graceful = shutdown_coordinator.wait_for_shutdown(15).await;
    
    if !graceful {
        tracing::error!("Forced shutdown due to timeout");
        std::process::exit(1);
    }

    tracing::info!("Kemono/Coomer Ingester shut down gracefully");
}

async fn monitor_kemono_links(shutdown_rx: watch::Receiver<bool>) {
    tracing::info!("Starting file monitor for kemono links...");

    let (tx, rx) = mpsc::channel();

    // Create a watcher
    let mut watcher = match RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        },
        Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("Failed to create file watcher: {}", e);
            return;
        }
    };

    // Watch the kc directory
    if let Err(e) = watcher.watch(Path::new(KC_LINKS_DIR), RecursiveMode::NonRecursive) {
        tracing::error!("Failed to watch kc directory: {}", e);
        return;
    }

    // Process existing files on startup
    if let Err(e) = process_existing_files().await {
        tracing::error!("Error processing existing files: {}", e);
    }

    loop {
        // Check for shutdown signal
        if *shutdown_rx.borrow() {
            tracing::info!("File monitor received shutdown signal");
            break;
        }

        // Check for file system events
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                if let Err(e) = handle_file_event(event).await {
                    tracing::error!("Error handling file event: {}", e);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Normal timeout, continue loop
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                tracing::error!("File watcher disconnected");
                break;
            }
        }
    }

    tracing::info!("File monitor stopped");
}

async fn process_existing_files() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Processing existing kemono URL files...");
    let entries = fs::read_dir(KC_LINKS_DIR)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_file() && path.extension().map_or(false, |ext| ext == "txt") {
            if let Err(e) = process_kemono_url_file(&path).await {
                tracing::error!("Error processing existing file {:?}: {}", path, e);
            }
        }
    }

    Ok(())
}

async fn handle_file_event(event: Event) -> Result<(), Box<dyn std::error::Error>> {
    use notify::EventKind;

    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => {
            for path in event.paths {
                if path.is_file() && path.extension().map_or(false, |ext| ext == "txt") {
                    // Small delay to ensure file is fully written
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    if let Err(e) = process_kemono_url_file(&path).await {
                        tracing::error!("Error processing file {:?}: {}", path, e);
                    }
                }
            }
        }
        _ => {
            // Ignore other event types
        }
    }

    Ok(())
}

async fn process_kemono_url_file(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Processing kemono URL file: {:?}", path);

    // Read URL from file
    let url = fs::read_to_string(path)?.trim().to_string();

    if url.is_empty() {
        tracing::info!("Empty URL file, skipping: {:?}", path);
        fs::remove_file(path)?;
        return Ok(());
    }

    // Validate URL format
    if !url.starts_with("https://kemono.cr/") && !url.starts_with("https://coomer.st/") {
        tracing::warn!("Invalid kemono/coomer URL format: {}", url);
        fs::remove_file(path)?;
        return Ok(());
    }

    tracing::info!("Processing kemono/coomer URL: {}", url);

    // Download content using kemono library
    match download_from_kemono_url(&url).await {
        Ok(download_path) => {
            tracing::info!("Successfully downloaded to: {}", download_path);
            // Delete the processed URL file
            fs::remove_file(path)?;
        }
        Err(e) => {
            tracing::error!("Failed to download from kemono URL {}: {}", url, e);
            // Still delete the file to avoid reprocessing
            fs::remove_file(path)?;
        }
    }

    Ok(())
}
