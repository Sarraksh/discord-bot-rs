use serde::Deserialize;
use serenity::builder::{CreateAttachment, CreateMessage};
use serenity::http::Http;
use serenity::model::id::ChannelId;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};

const MAX_ATTACHMENTS: usize = 10;
const MAX_TOTAL_BYTES: u64 = 200 * 1024 * 1024; // 200 MB

#[derive(Debug, Deserialize)]
struct UrlEntry {
    url: String,
    name: Option<String>,
}

pub async fn watch_and_send_discord_folders(discord_token: String, channel_id: u64) {
    let http = Http::new(&discord_token);
    let channel = ChannelId::new(channel_id);

    loop {
        let entries = match fs::read_dir("messages") {
            Ok(entries) => entries,
            Err(_) => {
                sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Ok(metadata) = entry.metadata() {
                if let Ok(created) = metadata.created() {
                    if created.elapsed().unwrap_or(Duration::ZERO) < Duration::from_secs(5) {
                        continue;
                    }
                }
            }

            let mut media_files = vec![];
            let mut url_json_files = vec![];
            let mut title_text: Option<String> = None;
            // let mut post_text: Option<String> = None;

            if let Ok(files) = fs::read_dir(&path) {
                for file in files.flatten() {
                    let fpath = file.path();
                    let ext = fpath
                        .extension()
                        .and_then(OsStr::to_str)
                        .unwrap_or("")
                        .to_lowercase();

                    match ext.as_str() {
                        "jpg" | "jpeg" | "png" | "gif" | "mp4" | "mov" | "webp" => {
                            media_files.push(fpath);
                        }
                        "txt" => {
                            let name = fpath
                                .file_stem()
                                .and_then(OsStr::to_str)
                                .unwrap_or("")
                                .to_lowercase();
                            let content = fs::read_to_string(&fpath).unwrap_or_default();
                            match name.as_str() {
                                "title" => title_text = Some(content.trim().to_string()),
                                // "post" => post_text = Some(content.trim().to_string()),
                                _ => {}
                            }
                        }
                        "json" => {
                            if fpath
                                .file_name()
                                .and_then(OsStr::to_str)
                                .unwrap_or("")
                                .ends_with(".url.json")
                            {
                                url_json_files.push(fpath);
                            }
                        }
                        _ => {}
                    }
                }
            }

            media_files.sort();
            url_json_files.sort();

            let mut url_lines = vec![];
            let total_links = url_json_files.len();

            for (i, path) in url_json_files.iter().enumerate() {
                let entry = match fs::read_to_string(path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<UrlEntry>(&s).ok())
                {
                    Some(e) => e,
                    None => {
                        eprintln!("Invalid or unreadable .url.json file: {:?}", path);
                        continue;
                    }
                };

                let label = if let Some(name) = &entry.name {
                    format!("[{:2}/{:2}] [{}]({})", i + 1, total_links, name, entry.url)
                } else {
                    format!("[{:2}/{:2}] {}", i + 1, total_links, entry.url)
                };

                url_lines.push(label);
            }

            // Construct the text part
            let mut message_content = String::new();
            if let Some(title) = &title_text {
                let cleaned_title = clean_text_field(title);
                if !cleaned_title.is_empty() {
                    message_content.push_str(title);
                    message_content.push('\n');
                }
            }
            if !url_lines.is_empty() {
                message_content.push_str(&url_lines.join("\n"));
            }

            // Trim to Discord max message length
            let trimmed_message = message_content.chars().take(2000).collect::<String>();

            let mut has_text = !trimmed_message.is_empty();

            // Chunk logic: by max 10 files AND under 200 MB
            let mut current_chunk = Vec::new();
            let mut current_total_size = 0u64;

            for path in &media_files {
                let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

                let would_exceed = current_chunk.len() >= MAX_ATTACHMENTS
                    || current_total_size + size > MAX_TOTAL_BYTES;

                if would_exceed {
                    send_file_chunk(&channel, &http, &current_chunk, has_text, &trimmed_message)
                        .await;
                    current_chunk.clear();
                    current_total_size = 0;
                    has_text = false; // only first chunk gets text
                }

                current_chunk.push(path.clone());
                current_total_size += size;
            }

            if !current_chunk.is_empty() {
                send_file_chunk(&channel, &http, &current_chunk, has_text, &trimmed_message).await;
            }

            // If no files, send text-only
            if media_files.is_empty() && has_text {
                let msg = CreateMessage::new().content(trimmed_message.clone());
                if let Err(e) = channel.send_message(&http, msg).await {
                    eprintln!("Failed to send text-only message: {}", e);
                }
            }

            // Cleanup
            if let Err(e) = fs::remove_dir_all(&path) {
                eprintln!("Failed to delete folder {:?}: {}", path, e);
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
}

async fn send_file_chunk(
    channel: &ChannelId,
    http: &Http,
    chunk: &[PathBuf],
    include_text: bool,
    message_content: &str,
) {
    let mut attachments = Vec::new();
    for path in chunk {
        match CreateAttachment::path(path).await {
            Ok(att) => attachments.push(att),
            Err(e) => {
                eprintln!("Failed to create attachment from {:?}: {}", path, e);
            }
        }
    }

    let mut msg = CreateMessage::default();
    if include_text {
        msg = msg.content(message_content.to_string());
    }

    if let Err(e) = channel.send_files(http, attachments, msg).await {
        eprintln!("Failed to send media files: {:?}", e);
    }
}

fn clean_text_field(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
}
