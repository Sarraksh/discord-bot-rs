use super::*;

use serenity::all::{CacheHttp, ChannelId, GuildId, User, UserId};

pub async fn get_channel_name(channel_id: &ChannelId, cache_http: impl CacheHttp) -> String {
    let undefined_channel = "In the middle of nowhere".to_string();
    channel_id
        .name(&cache_http)
        .await
        .unwrap_or(undefined_channel)
}

// TODO - log process
pub async fn get_user_name(user_id: &UserId, cache_http: impl CacheHttp, conf: &Config) -> String {
    let undefined_user = "Ахиллес сын Пелея".to_string();

    if *user_id == conf.override_user_id {
        return conf.override_user_name.clone();
    }

    let user = match user_id.to_user(&cache_http).await {
        Ok(u) => u,
        Err(err) => {
            tracing::warn!("Error getting user: {:?}", err);
            return undefined_user;
        }
    };

    if let Some(nick) = get_user_guild_name(&cache_http, &user, &conf.guild_id).await {
        if !nick.is_empty() {
            return nick;
        }
    }

    match user.name.is_empty() {
        false => user.name,
        true => {
            tracing::warn!("User has no name: {:?}", user);
            undefined_user
        }
    }
}

pub async fn get_user_guild_name(
    cache_http: impl CacheHttp,
    user: &User,
    guild_id: &GuildId,
) -> Option<String> {
    user.nick_in(cache_http, guild_id).await
}

use unicode_segmentation::UnicodeSegmentation;

pub fn count_symbols(s: &str) -> usize {
    s.graphemes(true).count()
}
