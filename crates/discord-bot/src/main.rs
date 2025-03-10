mod config;
mod messages;
mod reporter;
mod stat;
mod storage;
mod util;

use config::*;
use messages::*;
use reporter::*;
use stat::*;
use storage::*;
use util::*;

use serenity::all::UserId;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::{clone, env};
use std::sync::Arc;

struct Handler {
    stat: Arc<Mutex<Stat>>,
    storage: Arc<Mutex<Storage>>,
    config: Arc<Mutex<Config>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let mut stat_guard = self.stat.lock().await;
        stat_guard.message_streak.update_streak(&msg);
        let storage_guard = self.storage.lock().await;
        let config_guard = self.config.lock().await;
        let conf = config_guard.clone();
        drop(config_guard);
        agr_to_someone(ctx.clone(), &msg, storage_guard.self_id, &conf).await;
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
    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let stat = Stat::default();
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

    stat_reporter(client.http.clone(), arc_stat, config);

    // Finally, start a single shard, and start listening to events.
    //
    // Shards will automatically attempt to reconnect, and will perform exponential backoff until
    // it reconnects.
    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}

pub fn format_table(s: String, d: chrono::Duration) -> String {
    [
        "Личный зачёт по флуду за неделю:",
        "```",
        s.as_str(),
        "```",
    ]
    .join("\n")
}
