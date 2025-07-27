use chrono::Utc;
use kc::KEMONO_COOMER_REGEX;
use regex::Regex;
use std::collections::VecDeque;
use std::fs::{create_dir_all, rename, File};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use teloxide::{
    prelude::*,
    types::{MediaAnimation, MediaDocument, MediaKind, MediaPhoto, MediaVideo, MessageKind},
};
use tokio::time::{sleep, Duration, Instant};
use tracing::{error, info};

struct Attachment {
    file_id: String,
    original_path: String,
}

pub async fn handle_telegram_photos() {
    info!("Starting telegram bot...");

    let bot = Bot::from_env();
    let attachments: Arc<Mutex<Option<(VecDeque<Attachment>, Instant, UserId)>>> =
        Arc::new(Mutex::new(None));

    teloxide::repl(bot.clone(), move |bot: Bot, msg: Message| {
        let bot = bot.clone();
        let attachments = Arc::clone(&attachments);

        async move {
            if let MessageKind::Common(msg_common) = &msg.kind {
                if let Some(user) = &msg.from {
                    let user_id = user.id;

                    // Rule for Kemono/Coomer URL
                    if let Some(text) = &msg.text() {
                        let site_regex = Regex::new(KEMONO_COOMER_REGEX).unwrap();
                        if let Some(url) = site_regex.find(text) {
                            let url_str = url.as_str().to_string();
                            tokio::spawn(async move {
                                match kc::download_from_kemono_url(&url_str).await {
                                    Ok(_) => {}
                                    Err(e) => {
                                        error!(
                                            "Failed to download from Kemono/Coomer URL: {e}"
                                        );
                                    }
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
                        MediaKind::Animation(MediaAnimation { animation, .. }) => {
                            Some(Attachment {
                                file_id: animation.file.id.to_string(),
                                original_path: animation.file.id.to_string(),
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
                                                format!("exchange/messages_tmp/{}_{}", user_id, timestamp);
                                            let folder_final =
                                                format!("exchange/messages/{}_{}", user_id, timestamp);

                                            if let Err(e) = create_dir_all(&folder_tmp) {
                                                error!("Failed to create tmp dir: {}", e);
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
                                                    error!("Download failed: {}", e);
                                                }
                                            }

                                            if let Err(e) = rename(&folder_tmp, &folder_final) {
                                                error!("Failed to move directory: {}", e);
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
