use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use tokio::time::Duration;
use tokio::{fs::File, io::AsyncWriteExt};
use uuid::Uuid;

/// A file or attachment
#[derive(Debug, Deserialize)]
struct FileEntry {
    name: String,
    path: String,
}

/// Main API structure
#[derive(Debug, Deserialize)]
struct PostData {
    post: Post,
}

#[derive(Debug, Deserialize)]
struct Post {
    attachments: Option<Vec<FileEntry>>,
    file: Option<FileEntry>,
    title: Option<String>,
    content: Option<String>,
}

const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "mp4", "mov", "gif", "webp"];
const MAX_FILE_SIZE: u64 = 50_000_000;
pub const KEMONO_COOMER_REGEX: &str =
    r"https://(kemono\.cr|coomer\.st)/[^/]+/user/[[:alnum:]_]+/post/\d+";

pub async fn download_post_files(
    domain: &str,
    service: &str,
    user_id: &str,
    post_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!(
        "https://{}/api/v1/{}/user/{}/post/{}",
        domain, service, user_id, post_id
    );

    let client = reqwest::Client::new();
    let post_data: PostData = client.get(&url).send().await?.json().await?;

    let mut files = Vec::new();
    if let Some(file) = post_data.post.file {
        files.push(file);
    }
    if let Some(mut attachments) = post_data.post.attachments {
        files.append(&mut attachments);
    }

    if files.is_empty() {
        println!("No supported attachments found.");
    }

    let uuid = Uuid::new_v4();
    let tmp_dir = format!("./messages_tmp/{}", uuid);
    let final_dir = format!("./messages/{}", uuid);
    fs::create_dir_all(&tmp_dir)?;

    // Save post content to post.txt
    let post_text_path = format!("{}/post.txt", tmp_dir);
    if let Some(text) = &post_data.post.content {
        fs::write(&post_text_path, text)?;
    }

    // Save post title to title.txt
    let title_text_path = format!("{}/title.txt", tmp_dir);
    if let Some(text) = &post_data.post.title {
        fs::write(&title_text_path, text)?;
    }

    let mut file_number = 0;
    for file in &files {
        file_number += 1;
        let ext = Path::new(&file.name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
            println!("Skipping unsupported file type: {}", file.name);
            continue;
        }

        let sanitized_name = sanitize_filename(&file.name);
        let save_path = format!("{}/{file_number:0>3}_{}", tmp_dir, sanitized_name);
        println!("Saving file: {}", save_path);
        if Path::new(&save_path).exists() {
            println!("Skipping already downloaded: {}", save_path);
            continue;
        }

        // TODO - fallback to file path without "data" ?
        let file_url = format!("https://{}/data{}", domain, file.path);
        println!("Checking file: {}", file_url);

        let head = client.head(&file_url).send().await?;
        let content_len = head
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());

        if let Some(size) = content_len {
            if size > MAX_FILE_SIZE {
                println!(
                    "File too large ({} bytes), saving JSON link: {}",
                    size, sanitized_name
                );
                let url_path = format!(
                    "{}/{}.url.json",
                    tmp_dir,
                    Path::new(&sanitized_name)
                        .file_stem()
                        .unwrap()
                        .to_string_lossy()
                );
                let url_entry = serde_json::json!({
                    "url": file_url,
                    "name": Path::new(&sanitized_name).file_stem().unwrap_or_default().to_string_lossy()
                });
                fs::write(&url_path, serde_json::to_string_pretty(&url_entry)?)?;
                continue;
            }
        }

        println!("Downloading: {}", file_url);
        let mut resp = client.get(&file_url).send().await?;
        let mut out = File::create(&save_path).await?;
        while let Some(chunk) = resp.chunk().await? {
            out.write_all(&chunk).await?;
        }
    }

    fs::create_dir_all("./messages")?;
    match fs::rename(&tmp_dir, &final_dir) {
        Ok(_) => {
            println!("Moved to: {}", final_dir);
            Ok(final_dir)
        }
        Err(e) => {
            eprintln!("Failed to move directory: {}", e);
            Err(Box::new(e))
        }
    }
}

/// Parses the Kemono or Coomer post URL and calls the API download function
pub async fn download_from_kemono_url(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Example: https://kemono.cr/patreon/user/82530106/post/128244687
    // Example: https://coomer.st/onlyfans/user/12345/post/67890
    let (domain, path) = if url.starts_with("https://kemono.cr/") {
        ("kemono.cr", url.trim_start_matches("https://kemono.cr/"))
    } else if url.starts_with("https://coomer.st/") {
        ("coomer.st", url.trim_start_matches("https://coomer.st/"))
    } else {
        return Err("Invalid URL format. Must be kemono.cr or coomer.st".into());
    };

    let parts: Vec<&str> = path.trim_end_matches('/').split('/').collect();

    if parts.len() != 5 || parts[1] != "user" || parts[3] != "post" {
        return Err("Invalid URL format.".into());
    }

    let service = parts[0];
    let user_id = parts[2];
    let post_id = parts[4];

    download_post_files(domain, service, user_id, post_id).await
}

/// Makes file system-safe names
fn sanitize_filename(name: &str) -> String {
    name.replace(['/', '\\', '?', '%', '*', ':', '|', '"', '<', '>'], "_")
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ArtistEntry {
    pub author_name: Option<String>,
    pub platform: String,
    pub user_id: String,
    pub domain: String,
    #[serde(default)]
    pub last_ingested: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostSummary {
    id: String,
    title: String,
}

pub async fn start_kemono_ingest_loop(mut shutdown_signal: tokio::sync::watch::Receiver<bool>) {
    let path = "./kemono-artists.json";
    let mut artists: Vec<ArtistEntry> = read_json(path).unwrap_or_default();

    loop {
        for artist in &mut artists {
            match fetch_and_ingest_posts(artist).await {
                Ok(Some(new_last_id)) => artist.last_ingested = Some(new_last_id),
                Ok(None) => (),
                Err(e) => eprintln!("Error for {}: {}", artist.user_id, e),
            }
        }

        if let Err(e) = write_json(path, &artists) {
            eprintln!("Failed to write JSON: {}", e);
        }

        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(3600)) => continue,
            _ = shutdown_signal.changed() => {
                println!("Kemono loop shutdown triggered.");
                break;
            }
        }
    }
}

async fn fetch_and_ingest_posts(
    artist: &ArtistEntry,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://{}/api/v1/{}/user/{}",
        artist.domain, artist.platform, artist.user_id
    );
    let posts: Vec<PostSummary> = reqwest::get(&url).await?.json().await?;
    // TODO - implement sorting by date for now rely to order from response

    if posts.is_empty() {
        return Ok(None);
    }

    let new_posts: Vec<_> = if let Some(last_id) = &artist.last_ingested {
        posts.iter().take_while(|p| p.id != *last_id).collect()
    } else {
        posts.iter().take(10).collect()
    };

    if new_posts.is_empty() {
        return Ok(None);
    }

    for post in new_posts.iter().rev() {
        let post_url = format!(
            "https://{}/{}/user/{}/post/{}",
            artist.domain, artist.platform, artist.user_id, post.id
        );
        println!("Ingesting: {}", post_url);
        let _ = download_from_kemono_url(&post_url).await;
    }

    Ok(new_posts.first().map(|p| p.id.clone()))
}

fn read_json<P: AsRef<Path>, T: for<'de> Deserialize<'de>>(
    path: P,
) -> Result<T, Box<dyn std::error::Error>> {
    let data = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&data)?)
}

fn write_json<P: AsRef<Path>, T: Serialize>(
    path: P,
    data: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let text = serde_json::to_string_pretty(data)?;
    fs::write(path, text)?;
    Ok(())
}
