use chrono::Utc;
use kc::KEMONO_COOMER_REGEX;
use regex::Regex;
use shutdown_utils::ShutdownCoordinator;
use std::collections::VecDeque;
use std::fs::{create_dir_all, rename, File};
use std::io::Write;
use std::sync::{Arc, Mutex};
use teloxide::{
    prelude::*,
    types::{MediaAnimation, MediaDocument, MediaKind, MediaPhoto, MediaVideo, MessageKind},
};
use tokio::sync::watch;
use tokio::time::{sleep, Duration, Instant};
use tracing::{error, info};
use uuid::Uuid;

type AttachmentQueue = Arc<Mutex<Option<(VecDeque<Attachment>, Instant, UserId)>>>;

struct Attachment {
    file_id: String,
    #[allow(dead_code)]
    original_path: String,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Telegram bot...");

    // Create shutdown coordinator
    let mut shutdown_coordinator = ShutdownCoordinator::new();
    let shutdown_rx = shutdown_coordinator.subscribe();

    let bot = Bot::from_env();
    let attachments: AttachmentQueue = Arc::new(Mutex::new(None));

    // Spawn the main bot handler
    let bot_clone = bot.clone();
    let attachments_clone = attachments.clone();
    let shutdown_rx_clone = shutdown_rx.clone();
    
    let bot_task = tokio::spawn(async move {
        run_telegram_bot(bot_clone, attachments_clone, shutdown_rx_clone).await;
    });

    // Spawn attachment cleanup task
    let attachments_clone = attachments.clone();
    let shutdown_rx_clone = shutdown_rx.clone();
    let cleanup_task = tokio::spawn(async move {
        run_attachment_cleanup(attachments_clone, shutdown_rx_clone).await;
    });

    // Add tasks to coordinator
    shutdown_coordinator.add_task(bot_task);
    shutdown_coordinator.add_task(cleanup_task);

    info!("Telegram bot is running. Press Ctrl+C to stop.");

    // Wait for shutdown with 15 second timeout
    let graceful = shutdown_coordinator.wait_for_shutdown(15).await;
    
    if !graceful {
        error!("Forced shutdown due to timeout");
        std::process::exit(1);
    }

    info!("Telegram bot shut down gracefully");
}

async fn run_telegram_bot(_bot: Bot, attachments: AttachmentQueue, mut shutdown_rx: watch::Receiver<bool>) {
    let bot = Bot::from_env(); // Get a fresh bot instance for the repl
    
    tokio::select! {
        _ = shutdown_rx.changed() => {
            info!("Telegram bot received shutdown signal");
        }
        _ = teloxide::repl(bot, move |bot: Bot, msg: Message| {
            let attachments = attachments.clone();
            async move {
                handle_message(bot, msg, attachments).await
            }
        }) => {
            info!("Telegram repl exited");
        }
    }
}

async fn handle_message(_bot: Bot, msg: Message, attachments: AttachmentQueue) -> ResponseResult<()> {
    if let MessageKind::Common(msg_common) = &msg.kind {
        if let Some(user) = &msg.from {
            let user_id = user.id;

            // print message text
            if let Some(text) = &msg.text() {
                info!("Received message from user {}: {}", user_id, text);
            } else {
                info!("Received message from user {} with no text", user_id);
            }

            // Rule for Kemono/Coomer URL - save to file instead of processing directly
            if let Some(text) = &msg.text() {
                let kemono_regex = Regex::new(KEMONO_COOMER_REGEX).unwrap();
                if let Some(url) = kemono_regex.find(text) {
                    let url_str = url.as_str().to_string();
                    tokio::spawn(async move {
                        if let Err(e) = save_kemono_url_to_file(&url_str, "telegram").await {
                            error!("Failed to save Kemono URL to file: {e}");
                        }
                    });
                    return Ok(());
                }
            }

            let extract = match &msg_common.media_kind {
                MediaKind::Photo(MediaPhoto { photo, .. }) => photo
                    .iter()
                    .max_by_key(|p| p.file.size)
                    .map(|p| Attachment {
                        file_id: p.file.id.to_string(),
                        original_path: p.file.id.to_string(),
                    }),
                MediaKind::Document(MediaDocument { document, .. }) => Some(Attachment {
                    file_id: document.file.id.to_string(),
                    original_path: document.file.id.to_string(),
                }),
                MediaKind::Video(MediaVideo { video, .. }) => Some(Attachment {
                    file_id: video.file.id.to_string(),
                    original_path: video.file.id.to_string(),
                }),
                MediaKind::Animation(MediaAnimation { animation, .. }) => Some(Attachment {
                    file_id: animation.file.id.to_string(),
                    original_path: animation.file.id.to_string(),
                }),
                _ => None,
            };

            if let Some(attachment) = extract {
                let mut store = attachments.lock().unwrap();
                match store.as_mut() {
                    Some((queue, last_seen, stored_user_id)) if *stored_user_id == user_id => {
                        queue.push_back(attachment);
                        *last_seen = Instant::now();
                    }
                    _ => {
                        let mut queue = VecDeque::new();
                        queue.push_back(attachment);
                        *store = Some((queue, Instant::now(), user_id));
                    }
                }
            }
        }
    }
    Ok(())
}

async fn run_attachment_cleanup(attachments: AttachmentQueue, mut shutdown_rx: watch::Receiver<bool>) {
    let bot = Bot::from_env();
    
    loop {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                info!("Attachment cleanup received shutdown signal");
                break;
            }
            _ = sleep(Duration::from_secs(1)) => {
                let expired = {
                    let store = attachments.lock().unwrap();
                    store.as_ref().is_some_and(|(_, last_seen, _)| {
                        last_seen.elapsed() >= Duration::from_secs(3)
                    })
                };

                if expired {
                    let (queue, _, user_id) = attachments.lock().unwrap().take().unwrap();
                    let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
                    let folder_tmp = format!("exchange/messages_tmp/{}_{}", user_id, timestamp);
                    let folder_final = format!("exchange/messages/{}_{}", user_id, timestamp);

                    if let Err(e) = create_dir_all(&folder_tmp) {
                        error!("Failed to create tmp dir: {}", e);
                        continue;
                    }

                    for item in queue {
                        if let Err(e) = download_and_save_file(&bot, &item.file_id, &folder_tmp).await {
                            error!("Download failed: {}", e);
                        }
                    }

                    if let Err(e) = rename(&folder_tmp, &folder_final) {
                        error!("Failed to move directory: {}", e);
                    }
                }
            }
        }
    }
}

async fn download_and_save_file(bot: &Bot, file_id: &str, folder: &str) -> Result<(), String> {
    let file = bot
        .get_file(file_id.to_owned().into())
        .send()
        .await
        .map_err(|e| format!("get_file failed: {e}"))?;
    let token = std::env::var("TELOXIDE_TOKEN").map_err(|e| format!("env error: {e}"))?;
    let url = format!("https://api.telegram.org/file/bot{token}/{}", file.path);
    info!("Fetching file from URL: {url}");

    let content = reqwest::get(&url)
        .await
        .map_err(|e| format!("request failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("bytes failed: {e}"))?;
    let filename = format!("{folder}/{}", file.path.replace('/', "_"));
    info!("Saving file to: {}", filename);
    let mut file = File::create(&filename).map_err(|e| format!("file create failed: {e}"))?;
    file.write_all(&content)
        .map_err(|e| format!("write failed: {e}"))?;
    Ok(())
}

async fn save_kemono_url_to_file(
    url: &str,
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create kemono-links directory if it doesn't exist
    create_dir_all("./exchange/kc-links")?;

    // Generate filename with timestamp, source, and UUID
    let timestamp = Utc::now().timestamp();
    let uuid = Uuid::new_v4();
    let filename = format!(
        "./exchange/kc-links/{}_{}_{}_{}.txt",
        timestamp, source, uuid, "url"
    );

    // Write URL to file
    let mut file = File::create(&filename)?;
    file.write_all(url.as_bytes())?;

    info!("Saved Kemono URL to file: {}", filename);
    Ok(())
}
