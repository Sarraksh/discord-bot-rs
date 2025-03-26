use super::*;

use rand::Rng;
use serenity::all::{CreateMessage, UserId};
use serenity::model::channel::Message;
use serenity::prelude::*;

// TODO - implement send and immediately edit to add link to user without mention
pub async fn agr_to_someone(ctx: Context, msg: &Message, self_id: UserId, conf: &Config) {
    if msg.author.id == self_id {
        return;
    }

    if rand::rng().random_range(0..1) != 0 {
        return;
    }

    let victim_name = get_user_name(&msg.author.id, &ctx.http, conf).await;
    let text = [
        format!("{victim_name} опять что-то бухтит -_-"),
        format!("{victim_name} не бухти да не бухтим будешь :pray:"),
        format!("{victim_name}, привет! Как дела? Как сам? Как семья?"),
        format!("{victim_name}, есть чё?"),
        format!("{victim_name}, ты тут это, того, не этого, пнятненько?"),
        format!("{victim_name}, товарищ майор проинформирован о вашем поведении. Добавлена запись в личное дело."),
        format!("{victim_name}, а минусы будут?"),
        format!("К {victim_name} сзади подкрался крипер :boom:"),
        format!("{victim_name}, где макет?!"),
        format!("{victim_name} выдал базу"),
        format!("{victim_name} выдал кринж"),
        format!("{victim_name} иди-ка проспись"),
    ];
    let random_text_index = rand::rng().random_range(0..text.len());
    let random_text = text[random_text_index].clone();

    if let Err(why) = msg.channel_id.say(&ctx.http, random_text).await {
        println!("Error sending message: {why:?}")
    }
}

pub async fn react_to_mention(ctx: Context, msg: &Message, self_id: UserId, conf: &Config) -> bool {
    if msg.author.id == self_id {
        return false;
    }
    
    if msg.mentions.is_empty() {
        return false;
    }

    if !msg.mentions.iter().any(|mention| mention.id == self_id) {
        return false;
    }

    let victim_name = get_user_name(&msg.author.id, &ctx.http, conf).await;
    let text = [
        format!("{victim_name} опять что-то бухтит -_-"),
        format!("{victim_name} не бухти да не бухтим будешь :pray:"),
        format!("{victim_name}, привет! Как дела? Как сам? Как семья?"),
        format!("{victim_name}, есть чё?"),
        format!("{victim_name}, ты тут это, того, не этого, пнятненько?"),
        format!("{victim_name}, товарищ майор проинформирован о вашем поведении. Добавлена запись в личное дело."),
        format!("{victim_name}, а минусы будут?"),
        format!("К {victim_name} сзади подкрался крипер :boom:"),
        format!("{victim_name}, где макет?!"),
        format!("{victim_name} выдал базу"),
        format!("{victim_name} выдал кринж"),
        format!("{victim_name} иди-ка проспись"),
    ];
    let random_text_index = rand::rng().random_range(0..text.len());
    let random_text = text[random_text_index].clone();

    let builder = CreateMessage::new().content(random_text).reference_message(msg);
    if let Err(why) = msg.channel_id.send_message(&ctx.http, builder).await{
        println!("Error sending message: {why:?}")
    };

    true
}
