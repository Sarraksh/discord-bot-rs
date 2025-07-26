use chrono::Utc;
use regex::Regex;
use std::collections::VecDeque;
use std::fs::{create_dir_all, rename, File};
use std::io::Write;
use std::sync::{Arc, Mutex};
use teloxide::{
    prelude::*,
    types::{MediaAnimation, MediaDocument, MediaKind, MediaPhoto, MediaVideo, MessageKind},
};
use tokio::time::{sleep, Duration, Instant};
use uuid::Uuid;

struct Attachment {
    file_id: String,
    original_path: String,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting telegram bot...");

    let bot = Bot::from_env();
    let attachments: Arc<Mutex<Option<(VecDeque<Attachment>, Instant, UserId)>>> =
        Arc::new(Mutex::new(None));

    teloxide::repl(bot.clone(), move |bot: Bot, msg: Message| {
        let bot = bot.clone();
        let attachments = Arc::clone(&attachments);

        async move {
            if let MessageKind::Common(msg_common) = &msg.kind {
                if let Some(user) = &msg.from() {
                    let user_id = user.id;

                    // Rule for Kemono URL - save to file instead of processing directly
                    if let Some(text) = &msg.text() {
                        let kemono_regex =
                            Regex::new(r"https://kemono\.su/[^/]+/user/\d+/post/\d+").unwrap();
                        if let Some(url) = kemono_regex.find(text) {
                            let url_str = url.as_str().to_string();
                            tokio::spawn(async move {
                                if let Err(e) = save_kemono_url_to_file(&url_str, "telegram").await
                                {
                                    log::error!("Failed to save Kemono URL to file: {e}");
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
                                file_id: p.file.id.clone(),
                                original_path: p.file.id.clone(),
                            }),
                        MediaKind::Document(MediaDocument { document, .. }) => Some(Attachment {
                            file_id: document.file.id.clone(),
                            original_path: document.file.id.clone(),
                        }),
                        MediaKind::Video(MediaVideo { video, .. }) => Some(Attachment {
                            file_id: video.file.id.clone(),
                            original_path: video.file.id.clone(),
                        }),
                        MediaKind::Animation(MediaAnimation { animation, .. }) => {
                            Some(Attachment {
                                file_id: animation.file.id.clone(),
                                original_path: animation.file.id.clone(),
                            })
                        }
                        _ => None,
                    };

                    if let Some(attachment) = extract {
                        let mut store = attachments.lock().unwrap();
                        let now = Instant::now();

                        match &mut *store {
                            Some((queue, last_seen, uid)) if *uid == user_id => {
                                *last_seen = now;
                                queue.push_back(attachment);
                            }
                            _ => {
                                let mut queue = VecDeque::new();
                                queue.push_back(attachment);
                                *store = Some((queue, now, user_id));

                                let bot_clone = bot.clone();
                                let attachments_clone = Arc::clone(&attachments);
                                tokio::spawn(async move {
                                    loop {
                                        sleep(Duration::from_secs(1)).await;

                                        let expired = {
                                            let store = attachments_clone.lock().unwrap();
                                            store.as_ref().is_some_and(|(_, last_seen, _)| {
                                                last_seen.elapsed() >= Duration::from_secs(3)
                                            })
                                        };

                                        if expired {
                                            let (queue, _, user_id) =
                                                attachments_clone.lock().unwrap().take().unwrap();
                                            let timestamp = Utc::now().format("%Y%m%dT%H%M%S");
                                            let folder_tmp =
                                                format!("messages_tmp/{}_{}", user_id, timestamp);
                                            let folder_final =
                                                format!("messages/{}_{}", user_id, timestamp);

                                            if let Err(e) = create_dir_all(&folder_tmp) {
                                                log::error!("Failed to create tmp dir: {}", e);
                                                return;
                                            }

                                            for item in queue {
                                                if let Err(e) = download_and_save_file(
                                                    &bot_clone,
                                                    &item.file_id,
                                                    &folder_tmp,
                                                )
                                                .await
                                                {
                                                    log::error!("Download failed: {}", e);
                                                }
                                            }

                                            if let Err(e) = rename(&folder_tmp, &folder_final) {
                                                log::error!("Failed to move directory: {}", e);
                                            }

                                            break;
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
            Ok(())
        }
    })
    .await;
}

async fn download_and_save_file(bot: &Bot, file_id: &str, folder: &str) -> Result<(), String> {
    let file = bot
        .get_file(file_id)
        .send()
        .await
        .map_err(|e| format!("get_file failed: {e}"))?;
    let token = std::env::var("TELOXIDE_TOKEN").map_err(|e| format!("env error: {e}"))?;
    let url = format!("https://api.telegram.org/file/bot{token}/{}", file.path);
    println!("Fetching file from URL: {url}");

    let content = reqwest::get(&url)
        .await
        .map_err(|e| format!("request failed: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("bytes failed: {e}"))?;
    let filename = format!("{folder}/{}", file.path.replace('/', "_"));
    println!("Saving file to: {}", filename);
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
    create_dir_all("./kemono-links")?;

    // Generate filename with timestamp, source, and UUID
    let timestamp = Utc::now().timestamp();
    let uuid = Uuid::new_v4();
    let filename = format!(
        "./kemono-links/{}_{}_{}_{}.txt",
        timestamp, source, uuid, "url"
    );

    // Write URL to file
    let mut file = File::create(&filename)?;
    file.write_all(url.as_bytes())?;

    log::info!("Saved Kemono URL to file: {}", filename);
    Ok(())
}
