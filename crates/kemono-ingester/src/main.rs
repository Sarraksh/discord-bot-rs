use kc::{download_from_kemono_url, start_kemono_ingest_loop};
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::watch;

const KC_LINKS_DIR: &str = "./exchange/kc-links";

#[tokio::main]
async fn main() {
    println!("Starting Kemono/Coomer Ingester...");

    // Create kc directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(KC_LINKS_DIR) {
        eprintln!("Failed to create kc directory: {}", e);
        return;
    }

    // Set up graceful shutdown
    let (shutdown_tx, shutdown_rx) = watch::channel::<bool>(false);

    // Spawn periodic artist ingestion loop
    let shutdown_rx_clone = shutdown_rx.clone();
    let artist_loop_handle = tokio::spawn(async move {
        start_kemono_ingest_loop(shutdown_rx_clone).await;
    });

    // Spawn URL file monitoring task
    let shutdown_rx_clone = shutdown_rx.clone();
    let file_monitor_handle = tokio::spawn(async move {
        monitor_kemono_links(shutdown_rx_clone).await;
    });

    // Set up Ctrl+C handler
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        println!("Received Ctrl+C, shutting down kemono/coomer ingester...");
        shutdown_tx.send(true).ok();
    });

    // Wait for tasks to complete
    let _ = tokio::try_join!(artist_loop_handle, file_monitor_handle);
    println!("Kemono/Coomer Ingester shut down gracefully.");
}

async fn monitor_kemono_links(shutdown_rx: watch::Receiver<bool>) {
    println!("Starting kc file monitor...");

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
            eprintln!("Failed to create file watcher: {}", e);
            return;
        }
    };

    // Watch the kc directory
    if let Err(e) = watcher.watch(Path::new(KC_LINKS_DIR), RecursiveMode::NonRecursive) {
        eprintln!("Failed to watch kc directory: {}", e);
        return;
    }

    // Process existing files on startup
    if let Err(e) = process_existing_files().await {
        eprintln!("Error processing existing files: {}", e);
    }

    loop {
        // Check for shutdown signal
        if *shutdown_rx.borrow() {
            break;
        }

        // Check for file system events
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                if let Err(e) = handle_file_event(event).await {
                    eprintln!("Error handling file event: {}", e);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Normal timeout, continue loop
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("File watcher disconnected");
                break;
            }
        }
    }

    println!("File monitor stopped.");
}

async fn process_existing_files() -> Result<(), Box<dyn std::error::Error>> {
    let entries = fs::read_dir(KC_LINKS_DIR)?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().map_or(false, |ext| ext == "txt") {
            if let Err(e) = process_kemono_url_file(&path).await {
                eprintln!("Error processing existing file {:?}: {}", path, e);
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
                        eprintln!("Error processing file {:?}: {}", path, e);
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
    println!("Processing kemono URL file: {:?}", path);

    // Read URL from file
    let url = fs::read_to_string(path)?.trim().to_string();

    if url.is_empty() {
        println!("Empty URL file, skipping: {:?}", path);
        fs::remove_file(path)?;
        return Ok(());
    }

    // Validate URL format
    if !url.starts_with("https://kemono.cr/") && !url.starts_with("https://coomer.st/") {
        eprintln!("Invalid kemono/coomer URL format: {}", url);
        fs::remove_file(path)?;
        return Ok(());
    }

    println!("Processing kemono/coomer URL: {}", url);

    // Download content using kemono library
    match download_from_kemono_url(&url).await {
        Ok(download_path) => {
            println!("Successfully downloaded to: {}", download_path);
            // Delete the processed URL file
            fs::remove_file(path)?;
        }
        Err(e) => {
            eprintln!("Failed to download from kemono URL {}: {}", url, e);
            // Still delete the file to avoid reprocessing
            fs::remove_file(path)?;
        }
    }

    Ok(())
}
