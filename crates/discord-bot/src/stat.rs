use std::{collections::HashMap, vec};

use chrono::Timelike;
use serenity::all::{CacheHttp, ChannelId, GuildId, UserId};

#[derive(Debug, Default)]
pub struct Stat {
    pub collection_start: chrono::NaiveDateTime,
    pub collect_until: chrono::NaiveDateTime,
    pub last_collection_duration: chrono::Duration,
    pub message_streak: MessageStreak,
}

impl Stat {
    pub fn init_collection(&mut self) {
        self.collection_start = chrono::Utc::now().naive_utc();
        self.collect_until = next_update_time();
    }

    pub async fn check_collection_time(&mut self, cache_http: impl CacheHttp) -> Option<String> {
        let next_time = next_update_time();
        if self.collect_until == next_time {
            println!("Collection time is not reached yet");
            return None;
        }
        println!("Collection time reached");

        self.last_collection_duration = self.collect_until - self.collection_start;
        self.collection_start = self.collect_until;
        self.collect_until = next_time;
        self.message_streak.flush_records();
        self.message_streak.format_results_table(cache_http).await
    }
}

#[derive(Debug, Clone, Default)]
pub struct MessageStreak {
    pub current_by_channel: HashMap<ChannelId, MessageStreakUser>, // per channel, latest user and current streak
    pub personal_record: HashMap<UserId, MessageStreakPersonalRecord>, // per user record with chanel in which achieved
    pub messages_count: HashMap<UserId, usize>,
    pub attachments_count: HashMap<UserId, usize>,
}

impl MessageStreak {
    pub async fn format_results_table(&self, cache_http: impl CacheHttp) -> Option<String> {
        let user_header = "User".to_string();
        let messages_header = "Messages".to_string();
        let series_header = "Max series".to_string();
        let attachments_header = "Files".to_string();

        let mut user_name_list = vec![user_header.clone()];
        let mut channel_name_list = vec![];
        let mut rows = vec![];

        let mut max_symbols_in_user_name = 0;
        let mut max_symbols_in_series_channel_name = 0;

        let max_symbols_in_messages = match messages_header.len() {
            0..4 => 4,
            _ => messages_header.len(),
        };
        let max_symbols_in_attachments = match attachments_header.len() {
            0..4 => 4,
            _ => attachments_header.len(),
        };

        for (user_id, personal_record) in self.personal_record.iter() {
            let user_name = get_user_name(user_id, &cache_http).await;
            user_name_list.push(user_name);

            let channel_name = get_channel_name(&personal_record.channel_id, &cache_http).await;
            channel_name_list.push(channel_name);
        }

        for user_name in user_name_list {
            if user_name.len() > max_symbols_in_user_name {
                max_symbols_in_user_name = user_name.len();
            }
        }

        for channel_name in channel_name_list {
            if channel_name.len() > max_symbols_in_series_channel_name {
                max_symbols_in_series_channel_name = channel_name.len();
            }
        }
        let max_symbols_in_series = max_symbols_in_series_channel_name + 4 + 4; // 4 - for " in ", 4 - for digits

        if max_symbols_in_user_name == 0 {
            return None;
        }

        rows.push(format!("{user_header:>max_symbols_in_user_name$} | {messages_header:<max_symbols_in_messages$} | {series_header:>max_symbols_in_series$} | {attachments_header:>max_symbols_in_attachments$}"));
        rows.push(format!("{:>max_symbols_in_user_name$} | {:<max_symbols_in_messages$} | {:>max_symbols_in_series_channel_name$} | {:>max_symbols_in_attachments$}", "", "", "", ""));

        for (user_id, personal_record) in self.personal_record.iter() {
            let user_name = get_user_name(user_id, &cache_http).await;
            let messages_count = self.messages_count.get(user_id).unwrap_or(&0);
            let series_record_counter = personal_record.counter;
            let series_record_channel_name =
                get_channel_name(&personal_record.channel_id, &cache_http).await;
            let attachments_count = self.attachments_count.get(user_id).unwrap_or(&0);

            rows.push(format!("{user_name:>max_symbols_in_user_name$} | {messages_count:<max_symbols_in_messages$} | {series_record_counter:>4} in {series_record_channel_name:<max_symbols_in_series_channel_name$} | {attachments_count:>max_symbols_in_attachments$}"));
        }

        Some(rows.join("\n"))
    }

    pub fn update_streak(&mut self, channel_id: ChannelId, user_id: UserId) {
        self.messages_count
            .entry(user_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        self.attachments_count
            .entry(user_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);

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
            None => MessageStreakPersonalRecord::default(),
        };

        current_record.update_record(record_candidate.counter, channel_id);
        self.personal_record
            .insert(record_candidate.user_id, current_record);
    }

    pub fn flush_records(&mut self) {
        let last_list = self.current_by_channel.drain();
        last_list.for_each(|(channel_id, record_candidate)| {
            let mut current_record = match self.personal_record.get(&record_candidate.user_id) {
                Some(current) => current.clone(),
                None => MessageStreakPersonalRecord::default(),
            };

            current_record.update_record(record_candidate.counter, channel_id);
            self.personal_record
                .insert(record_candidate.user_id, current_record);
        });
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

pub fn next_update_time() -> chrono::NaiveDateTime {
    let now = chrono::Utc::now().naive_utc();
    let next = now + chrono::Duration::hours(1);
    chrono::NaiveDateTime::new(
        next.date(),
        chrono::NaiveTime::from_hms_opt(next.hour(), 0, 0).unwrap(),
    )
}

pub async fn get_channel_name(channel_id: &ChannelId, cache_http: impl CacheHttp) -> String {
    let undefined_channel = "In the middle of nowhere".to_string();
    channel_id
        .name(&cache_http)
        .await
        .unwrap_or(undefined_channel)
}

pub async fn get_user_name(user_id: &UserId, cache_http: impl CacheHttp) -> String {
    let undefined_user = "Achilles son of Peleus".to_string();

    match user_id.to_user(&cache_http).await {
        Err(_) => undefined_user,
        Ok(user) => match user.global_name {
            Some(global) => global,
            None => user
                .nick_in(cache_http, GuildId::from(1245747866555908197)) // TODO - move to config
                .await
                .unwrap_or(undefined_user),
        },
    }
}
