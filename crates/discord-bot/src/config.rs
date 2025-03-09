use super::*;

use serenity::all::{GuildId, UserId};

#[derive(Debug, Clone)]
pub struct Config {
    pub token: String,
    pub override_user_id: UserId,
    pub override_user_name: String,
    pub guild_id: GuildId,
}

pub fn init_config() -> Config {
    Config {
        token: env::var("DISCORD_TOKEN")
            .expect("Expected a token in the environment DISCORD_TOKEN"),
        override_user_id: env::var("OVERRIDE_USER_ID")
            .expect("Expected an ID in the environment OVERRIDE_USER_ID")
            .parse()
            .expect("Expected a valid ID in the environment OVERRIDE_USER_ID"),
        override_user_name: env::var("OVERRIDE_USER_NAME")
            .expect("Expected a name in the environment OVERRIDE_USER_NAME"),
        guild_id: env::var("GUILD_ID")
            .expect("Expected an ID in the environment GUILD_ID")
            .parse()
            .expect("Expected a valid ID in the environment GUILD_ID"),
    }
}
