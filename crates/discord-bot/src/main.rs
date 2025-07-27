mod config;
mod messages;
mod reporter;
mod send_images;
mod stat;
mod storage;
mod util;

use config::*;
use messages::*;
use reporter::*;
use send_images::*;
use stat::*;
use storage::*;
use util::*;

use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::env;
use std::fs::{create_dir_all, File};
use std::io::Write;
use std::sync::Arc;

use chrono::Utc;
use kc::KEMONO_COOMER_REGEX;
use regex::Regex;
use serenity::client::Client;
use serenity::model::gateway::GatewayIntents;
use tokio::signal;
use uuid::Uuid;

struct Handler {
    stat: Arc<Mutex<Stat>>,
    storage: Arc<Mutex<Storage>>,
    config: Arc<Mutex<Config>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let mut stat_guard = self.stat.lock().await;
        stat_guard.message_stat.update_streak(&msg);
        let storage_guard = self.storage.lock().await;
        let config_guard = self.config.lock().await;
        let conf = config_guard.clone();
        drop(config_guard);

        // Check for kemono URLs and save to file
        if let Err(e) = check_and_save_kemono_url(&msg).await {
            eprintln!("Error saving kemono URL: {}", e);
        }

        if react_to_mention(&ctx, &msg, storage_guard.self_id, &conf).await {
            return;
        };
        // TODO - enable later
        // if react_to_trigger_word(&ctx, &msg, storage_guard.self_id, &conf).await {
        //     return;
        // };
        // TODO - enable later
        // agr_to_someone(ctx.clone(), &msg, storage_guard.self_id, &conf).await;
    }

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, ctx: Context, ready: Ready) {
        let mut storage_guard = self.storage.lock().await;
        storage_guard.self_id = ready.user.id;
        let config_guard = self.config.lock().await;
        let conf = config_guard.clone();
        drop(config_guard);
        let bot_name = get_user_name(&storage_guard.self_id, &ctx.http, &conf).await;
        println!("{} is connected!", bot_name);
    }
}

#[tokio::main]
async fn main() {
    let stat_save_file = "stat/stat.json"; // TODO - move to config

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let stat = Stat::load_from_file(stat_save_file).unwrap_or_default();
    let arc_stat = Arc::new(Mutex::new(stat));
    let config = init_config();
    let arc_config = Arc::new(Mutex::new(config.clone()));
    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let token = config.token.clone();
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            stat: arc_stat.clone(),
            storage: Arc::new(Mutex::new(Storage::default())),
            config: arc_config.clone(),
        })
        .await
        .expect("Err creating client");

    stat_reporter(client.http.clone(), arc_stat.clone(), config);

    // Clone the shard manager to use for graceful shutdown.
    let shard_manager = client.shard_manager.clone();

    // Get Discord channel ID from environment variable
    let discord_channel_id: u64 = env::var("DISCORD_CHANNEL_ID")
        .unwrap_or_else(|_| "1245814193446326322".to_string())
        .parse()
        .expect("DISCORD_CHANNEL_ID must be a valid u64");

    // Spawn file watcher task
    tokio::spawn(watch_and_send_discord_folders(token, discord_channel_id));

    println!("Starting Discord bot...");

    // Spawn a task to listen for Ctrl+C and then shut down gracefully.
    let arc_stat_clone = arc_stat.clone();
    tokio::spawn(async move {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        println!("Received Ctrl+C, initiating shutdown...");

        let stat_guard = arc_stat_clone.lock().await;
        match stat_guard.save_to_file(stat_save_file) {
            Ok(_) => println!("State saved to {stat_save_file}"),
            Err(e) => eprintln!("Error saving stat: {}", e),
        }

        shard_manager.shutdown_all().await;
    });

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform exponential backoff until
    // it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}

async fn check_and_save_kemono_url(msg: &Message) -> Result<(), Box<dyn std::error::Error>> {
    let kemono_regex = Regex::new(KEMONO_COOMER_REGEX)?;

    if let Some(url_match) = kemono_regex.find(&msg.content) {
        let url = url_match.as_str();

        // Create kc directory if it doesn't exist
        create_dir_all("./exchange/kc-links")?;

        // Generate filename with timestamp, source, and UUID
        let timestamp = Utc::now().timestamp();
        let uuid = Uuid::new_v4();
        let filename = format!("./exchange/kc-links/{}_{}_{}.txt", timestamp, "discord", uuid);

        // Write URL to file
        let mut file = File::create(&filename)?;
        file.write_all(url.as_bytes())?;

        println!("Saved kemono URL to file: {}", filename);
    }

    Ok(())
}
