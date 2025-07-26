// use super::*;
// use serenity::all::{CacheHttp, ChannelId, GuildId, Message, UserId};
use serenity::all::UserId;

#[derive(Debug, Clone, Default)]
pub struct Storage {
    pub self_id: UserId,
}
