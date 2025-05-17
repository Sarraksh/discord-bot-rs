use super::*;

use serenity::all::Http;
use serenity::prelude::*;
use std::sync::Arc;

pub fn stat_reporter(http: Arc<Http>, stat: Arc<Mutex<Stat>>, conf: Config) {
    tokio::spawn(async move {
        let flood_channel_id = serenity::all::ChannelId::from(1245807370341191812); // TODO - move to config

        let mut stat_guard = stat.lock().await;
        stat_guard.init_collection();
        drop(stat_guard);

        loop {
            let now = chrono::Utc::now().naive_utc();

            let stat_guard = stat.lock().await;
            let diff = (stat_guard.collect_until - now).to_std().unwrap();
            drop(stat_guard);

            tokio::time::sleep(diff).await;

            let mut stat_guard = stat.lock().await;
            if let Some(table) = stat_guard.collect_report(&http, &conf).await {
                let table_message = [
                    env::var("TABLE_HEADER").unwrap_or_default().as_str(),
                    "```",
                    table.as_str(),
                    "```",
                ]
                .join("\n");
                if let Err(why) = flood_channel_id.say(&http, table_message).await {
                    println!("Error sending message: {why:?}");
                }
            } else {
                println!("No data to report");
            };
            stat_guard.message_stat = MessageStat::default();
        }
    });
}
