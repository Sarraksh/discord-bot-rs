use super::*;

use rand::Rng;
use serenity::all::UserId;
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
        format!("Вжух! Теперь {victim_name} фуриёб =D"),
        format!("{victim_name}, где макет?!"),
        format!("{victim_name} выдал базу"),
        format!("{victim_name} выдал кринж"),
    ];
    let random_text_index = rand::rng().random_range(0..text.len());
    let random_text = text[random_text_index].clone();

    if let Err(why) = msg.channel_id.say(&ctx.http, random_text).await {
        println!("Error sending message: {why:?}");
        return;
    }
}
