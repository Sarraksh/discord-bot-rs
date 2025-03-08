mod stat;
mod storage;
use stat::*;
use storage::*;

use rand::Rng;
use serenity::all::{Http, UserId};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::env;
use std::sync::Arc;

struct Handler {
    stat: Arc<Mutex<Stat>>,
    storage: Arc<Mutex<Storage>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let mut stat_guard = self.stat.lock().await;
        stat_guard.message_streak.update_streak(&msg);
        let storage_guard = self.storage.lock().await;
        send_user_is_bubbling(ctx.clone(), &msg, storage_guard.self_id).await;
    }

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, ctx: Context, ready: Ready) {
        let mut storage_guard = self.storage.lock().await;
        storage_guard.self_id = ready.user.id;
        get_user_name(&storage_guard.self_id, &ctx.http).await;
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let token =
        env::var("DISCORD_TOKEN").expect("Expected a token in the environment DISCORD_TOKEN");

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut stat = Stat::default();
    stat.init_collection();
    let arc_stat = Arc::new(Mutex::new(stat));
    // Create a new instance of the Client, logging in as a bot. This will automatically prepend
    // your bot token with "Bot ", which is a requirement by Discord for bot users.
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler {
            stat: arc_stat.clone(),
            storage: Arc::new(Mutex::new(Storage::default())),
        })
        .await
        .expect("Err creating client");

    qwe(client.http.clone(), arc_stat);

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
        format!("Личный зачёт по флуду за недавно (ну типа того) {d}",),
        "".to_string(),
        "```".to_string(),
        s,
        "```".to_string(),
    ]
    .join("\n")
}

pub fn qwe(http: Arc<Http>, stat: Arc<Mutex<Stat>>) {
    tokio::spawn(async move {
        let flood_channel_id = serenity::all::ChannelId::from(1245807370341191812); // TODO - move to config

        loop {
            let now = chrono::Utc::now().naive_utc();

            let stat_guard = stat.lock().await;
            let diff = (stat_guard.collect_until - now).to_std().unwrap();
            drop(stat_guard);

            tokio::time::sleep(diff).await;

            let mut stat_guard = stat.lock().await;
            if let Some(table) = stat_guard.check_collection_time(&http).await {
                println!("==== table is ======\n{:?}\n==============", table);
                let table_message = format_table(table, stat_guard.last_collection_duration);
                if let Err(why) = flood_channel_id.say(&http, table_message).await {
                    println!("Error sending message: {why:?}");
                }
            };
            stat_guard.message_streak = MessageStreak::default();
        }
    });
}

// TODO - implement send and immediately edit to add link to user without mention
async fn send_user_is_bubbling(ctx: Context, msg: &Message, self_id: UserId) {
    if msg.author.id == self_id {
        return;
    }

    if rand::rng().random_range(0..20) != 0 {
        return;
    }

    let victim_name = get_user_name(&msg.author.id, &ctx.http).await;
    let text = [
        format!("{victim_name} опять что-то бухтит -_-"),
        format!("{victim_name} не бухти да не бухтим будешь :pray:"),
        format!("{victim_name} привет! Как дела? Как сам? Как семья?"),
        format!("{victim_name} есть чё?"),
        format!("{victim_name} ты тут это, того, не этого, пнятненько?"),
        format!("{victim_name} , товарищ майор проинформирован о вашем поведении. Добавлена запись в личное дело."),
        format!("{victim_name} а минусы будут?"),
        format!("К {victim_name} сзади подкрался крипер :boom:"),
        format!("Вжух! Теперь {victim_name} фуриёб =D"),
        format!("{victim_name}, где макет?!"),
        format!("{victim_name} выдал базу"),
        format!("{victim_name} выдал кринж"),
    ];
    // let text = vec!["опять что-то бухтит -_-"];
    let random_text_index = rand::rng().random_range(0..text.len());
    let random_text = text[random_text_index].clone();

    if let Err(why) = msg
        .channel_id
        .say(&ctx.http, format!("{victim_name} {random_text}"))
        .await
    {
        println!("Error sending message: {why:?}");
        return;
    }
}
