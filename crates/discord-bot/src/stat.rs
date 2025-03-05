use std::collections::HashMap;

use serenity::{
    all::{ChannelId, UserId},
    model::channel::Message,
};

#[derive(Debug, Default)]
pub struct Stat {
    pub collect_until: chrono::NaiveDateTime,
    pub message_streak: MessageStreak,
}

#[derive(Debug, Clone, Default)]
pub struct MessageStreak {
    pub current_by_channel: HashMap<ChannelId, MessageStreakUser>, // per channel, latest user and current streak
    pub personal_record: HashMap<UserId, MessageStreakPersonalRecord>, // per user record with chanel in which achieved
}

impl MessageStreak {
    pub fn update_streak(&mut self, channel_id: ChannelId, user_id: UserId) {
        let mut channel_streak = match self.current_by_channel.get(&channel_id) {
            Some(channel_streak) => channel_streak.clone(),
            None => MessageStreakUser {
                user_id,
                counter: 0,
            },
        };

        let record_candidate = channel_streak.update_streak(user_id);
        self.current_by_channel.insert(channel_id, channel_streak);

        let record_candidate = match record_candidate {
            Some(latest) => latest,
            None => return,
        };

        let mut current_record = match self.personal_record.get(&record_candidate.user_id) {
            Some(current) => current.clone(),
            None => MessageStreakPersonalRecord::default(), // TODO - insert
        };

        current_record.update_record(record_candidate.counter, channel_id);
        self.personal_record.insert(record_candidate.user_id, current_record);
    }
}

#[derive(Debug, Clone, Default)]
pub struct MessageStreakUser {
    pub user_id: UserId,
    pub counter: usize,
}

impl MessageStreakUser {
    pub fn update_streak(&mut self, user_id: UserId) -> Option<MessageStreakUser> {
        if self.user_id == user_id {
            self.counter += 1;
            return None;
        }
        let old = self.clone();
        self.user_id = user_id;
        self.counter = 1;
        Some(old)
    }
}

#[derive(Debug, Clone, Default)]
pub struct MessageStreakPersonalRecord {
    pub channel_id: ChannelId,
    pub counter: usize,
}

impl MessageStreakPersonalRecord {
    pub fn update_record(&mut self, count: usize, channel_id: ChannelId) {
        if count > self.counter {
            self.counter = count;
            self.channel_id = channel_id;
        }
    }
}
