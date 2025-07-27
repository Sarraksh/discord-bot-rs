use super::*;

use chrono::{Datelike, Timelike};
use serde::{Deserialize, Serialize};
use serenity::all::{CacheHttp, ChannelId, Message, UserId};
use std::{collections::HashMap, vec};

use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::error::Error;
use tracing::{info, warn};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Stat {
    pub collection_start: chrono::NaiveDateTime,
    pub collect_until: chrono::NaiveDateTime,
    pub last_collection_duration: chrono::Duration,
    pub message_stat: MessageStat,
}

impl Stat {
    pub fn init_collection(&mut self) {
        self.collection_start = chrono::Utc::now().naive_utc();
        info!("Now time is     : UTC {:?}", self.collection_start);
        self.collect_until = next_update_time();
    }

    pub async fn collect_report(
        &mut self,
        cache_http: impl CacheHttp,
        conf: &Config,
    ) -> Option<String> {
        let next_time = next_update_time();
        self.last_collection_duration = self.collect_until - self.collection_start;
        self.collection_start = self.collect_until;
        self.collect_until = next_time;
        self.message_stat.flush_records();
        self.message_stat
            .format_results_table(cache_http, conf)
            .await
    }

    pub fn save_to_file(&self, file_path: &str) -> Result<(), Box<dyn Error>> {
        let file = File::create(file_path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, self)?;
        Ok(())
    }

    pub fn load_from_file(file_path: &str) -> Result<Stat, Box<dyn Error>> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);
        let stat = serde_json::from_reader(reader)?;
        Ok(stat)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageStat {
    pub current_by_channel: HashMap<ChannelId, MessageStreakUser>, // per channel, latest user and current streak
    pub personal_record: HashMap<UserId, MessageStreakPersonalRecord>, // per user record with chanel in which achieved
    pub messages_count: HashMap<UserId, usize>,
    pub attachments_count: HashMap<UserId, usize>,
}

impl MessageStat {
    pub async fn format_results_table(
        &self,
        cache_http: impl CacheHttp,
        conf: &Config,
    ) -> Option<String> {
        let user_header = "User".to_string();
        let messages_header = "Messages".to_string();
        let series_header = "Max series".to_string();
        let attachments_header = "Files".to_string();

        let mut user_name_list = vec![user_header.clone()];
        let mut channel_name_list = vec![];
        let mut rows = vec![];

        let mut max_symbols_in_user_name = 0;
        let mut max_symbols_in_series_channel_name = 0;

        let max_symbols_in_messages = match count_symbols(&messages_header) {
            0..4 => 4,
            _ => count_symbols(&messages_header),
        };
        let max_symbols_in_attachments = match count_symbols(&attachments_header) {
            0..4 => 4,
            _ => count_symbols(&attachments_header),
        };

        for (user_id, personal_record) in self.personal_record.iter() {
            let user_name = get_user_name(user_id, &cache_http, conf).await;
            user_name_list.push(user_name);

            let channel_name = get_channel_name(&personal_record.channel_id, &cache_http).await;
            channel_name_list.push(channel_name);
        }

        for user_name in user_name_list {
            // println!("{:>2} | {user_name}", count_symbols(&user_name));
            let symbols = count_symbols(&user_name);
            if symbols > max_symbols_in_user_name {
                max_symbols_in_user_name = symbols;
            }
        }

        for channel_name in channel_name_list {
            let symbols = count_symbols(&channel_name);
            if symbols > max_symbols_in_series_channel_name {
                max_symbols_in_series_channel_name = symbols;
            }
        }
        let max_symbols_in_series = max_symbols_in_series_channel_name + 4 + 4; // 4 - for " in ", 4 - for digits

        // TODO - refactor
        if max_symbols_in_user_name == 0 {
            return None;
        }

        rows.push(format!("{user_header:<max_symbols_in_user_name$} | {messages_header:<max_symbols_in_messages$} | {series_header:<max_symbols_in_series$} | {attachments_header:<max_symbols_in_attachments$}"));
        rows.push(format!("{:-<max_symbols_in_user_name$}-|-{:-<max_symbols_in_messages$}-|-{:-<max_symbols_in_series$}-|-{:-<max_symbols_in_attachments$}", "-", "-", "-", "-"));

        for (user_id, personal_record) in self.personal_record.iter() {
            let user_name = get_user_name(user_id, &cache_http, conf).await;
            let user_name_pad = max_symbols_in_user_name - count_symbols(&user_name);
            let user_name = format!("{user_name}{:<user_name_pad$}", "");

            let messages_count = self.messages_count.get(user_id).unwrap_or(&0);
            let series_record_counter = personal_record.counter;
            let series_record_channel_name =
                get_channel_name(&personal_record.channel_id, &cache_http).await;
            let attachments_count = self.attachments_count.get(user_id).unwrap_or(&0);

            rows.push(format!("{user_name} | {messages_count:>max_symbols_in_messages$} | {series_record_counter:>4} in {series_record_channel_name:<max_symbols_in_series_channel_name$} | {attachments_count:>max_symbols_in_attachments$}"));
        }

        rows.iter().for_each(|row| {
            info!("{}", row);
        });

        Some(rows.join("\n"))
    }

    pub fn update_streak(&mut self, msg: &Message) {
        let user_id = msg.author.id;
        let channel_id = msg.channel_id;

        self.messages_count
            .entry(user_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        let attachments = msg.attachments.len();
        self.attachments_count
            .entry(user_id)
            .and_modify(|count| *count += attachments)
            .or_insert(attachments);

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    // let next = next_update_time_min();
    let next = next_sunday_start_time();
    info!("Next update time: UTC {:?}", next);
    next
}

fn next_sunday_start_time() -> chrono::NaiveDateTime {
    let current_time = chrono::Utc::now().naive_utc(); // Get current local time
    let current_weekday = current_time.weekday(); // Get current weekday

    // Calculate days until next Sunday
    let days_until_sunday = 7 - current_weekday.num_days_from_sunday();
    let next_sunday = current_time.date() + chrono::Duration::days(days_until_sunday as i64); // Calculate next Sunday

    // Set time to start of the day (00:00:00)
    next_sunday.and_hms_opt(0, 0, 0).expect("Invalid time") // Return NaiveDateTime for next Sunday at midnight
}

pub fn next_update_time_min() -> chrono::NaiveDateTime {
    let now = chrono::Utc::now().naive_utc();
    let next = now + chrono::Duration::minutes(1) + chrono::Duration::seconds(10);
    chrono::NaiveDateTime::new(
        next.date(),
        chrono::NaiveTime::from_hms_opt(next.hour(), next.minute(), 0).unwrap(),
    )
}
