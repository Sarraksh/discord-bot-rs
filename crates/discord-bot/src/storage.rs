use super::*;
use serenity::all::{CacheHttp, ChannelId, GuildId, Message, UserId};

#[derive(Debug, Clone, Default)]
pub struct Storage {
    pub self_id: UserId,
}
