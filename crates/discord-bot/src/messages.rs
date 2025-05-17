use super::*;

use eyre::WrapErr;
use ollama_rs::generation::chat::{ChatMessage, MessageRole};
use ollama_rs::generation::chat::request::ChatMessageRequest;
use ollama_rs::Ollama;
use rand::Rng;
use regex::Regex;
use serenity::all::{CreateMessage, GetMessages, UserId};
use serenity::model::channel::Message;
use serenity::prelude::*;

// TODO - implement send and immediately edit to add link to user without mention
// TODO - implement pause fo particular messages in channel
pub async fn agr_to_someone(ctx: Context, msg: &Message, self_id: UserId, conf: &Config) {
    if msg.author.id == self_id {
        return;
    }

    if rand::rng().random_range(0..200) != 0 {
        return;
    }

    let victim_name = get_user_name(&msg.author.id, &ctx.http, conf).await;
    let text = [
        format!("{victim_name} опять что-то бухтит -_-"),
        format!("{victim_name}, ты тут это, того, не этого, пнятненько?"),
        format!("{victim_name}, товарищ майор проинформирован о вашем поведении. Добавлена запись в личное дело."),
        format!("{victim_name}, а минусы будут?"),
        format!("К {victim_name} сзади подкрался крипер :boom:"),
        format!("{victim_name}, где макет?!"),
        format!("{victim_name} выдаёт базу"),
        format!("{victim_name} выдаёт кринж"),
        format!("{victim_name} иди-ка проспись"),
        format!("{victim_name} ты только что гранату!"),
    ];
    let random_text_index = rand::rng().random_range(0..text.len());
    let random_text = text[random_text_index].clone();

    if let Err(why) = msg.channel_id.say(&ctx.http, random_text).await {
        println!("Error sending message: {why:?}")
    }
}

#[derive(Clone, Debug)]
pub struct MyChatMessage {
    pub cm: ChatMessage,
    pub msg_id: serenity::all::MessageId,
    pub timestamp: serenity::all::Timestamp,
}

pub async fn react(ctx: &Context, msg: &Message, self_id: UserId, conf: &Config) {
    let channel = msg
        .channel(ctx)
        .await
        .wrap_err_with(|| "Error getting channel")
        .unwrap()
        .id(); // TODO - handle error
    _ = channel.broadcast_typing(ctx).await;
    let ctx_clone = ctx.clone();

    // Create a shared Notify instance to signal stopping the loop
    let stop_signal = Arc::new(tokio::sync::Notify::new());
    // Clone the stop_signal for the spawned task
    let stop_signal_clone = stop_signal.clone();
    // Spawn the async task
    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = stop_signal_clone.notified() => {
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(7)) => {
                    _ = channel.broadcast_typing(&ctx_clone).await;
                }
            }
        }
    });
    // Simulate some delay before stopping the task
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let ollama = Ollama::new(
        env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost".to_string()),
        env::var("OLLAMA_PORT")
            .unwrap_or_else(|_| "11434".to_string())
            .parse::<u16>()
            .unwrap(),
    );

    let messages_chain = get_messages_chain(ctx, msg, 5)
        .await
        .unwrap_or_else(|_| vec![]);
    let mut messages_chain = process_messages(ctx, self_id, conf, &messages_chain)
        .await
        .unwrap_or_else(|_| vec![]);
    messages_chain.push(ChatMessage {
        role: MessageRole::System,
        content: env::var("SYSTEM_PROMPT").unwrap(),
        images: None,
        tool_calls: vec![],
    });

    let response_text = match messages_chain.len() {
        0 => {
            println!("No messages in the chain");
            env::var("ERROR_NO_MESSAGES").unwrap_or_else(|_| "ERROR_NO_MESSAGES".to_string())
        }
        _ => {
            let request = ChatMessageRequest::new(env::var("MODEL_NAME").unwrap(), messages_chain);

            _ = channel.broadcast_typing(ctx).await;
            match ollama.send_chat_messages(request).await {
                Ok(response) => response.message.content,
                Err(e) => {
                    println!("Error: {}", e);
                    env::var("ERROR_OLLAMA_ERROR")
                        .unwrap_or_else(|_| "ERROR_OLLAMA_ERROR".to_string())
                }
            }
        }
    };

    println!("===================================================================================");
    println!("Response: {}", response_text);
    let response_text = remove_think_blocks(&response_text);
    let response_text = replace_mentions(&response_text, &ctx.http, conf).await;
    let response_text = response_text.chars().take(2000).collect::<String>();
    let builder = CreateMessage::new()
        .content(response_text)
        .reference_message(msg);
    if let Err(why) = msg.channel_id.send_message(&ctx.http, builder).await {
        println!("Error sending message: {why:?}")
    };
    println!("===================================================================================");

    // Notify the task to stop
    stop_signal.notify_waiters();
    // Wait for the task to complete
    handle.await.unwrap();
}

pub async fn react_to_mention(
    ctx: &Context,
    msg: &Message,
    self_id: UserId,
    conf: &Config,
) -> bool {
    if msg.author.id == self_id {
        return false;
    }

    if msg.mentions.is_empty() {
        return false;
    }

    if !msg.mentions.iter().any(|mention| mention.id == self_id) {
        return false;
    }

    react(ctx, msg, self_id, conf).await;

    true
}

pub async fn react_to_trigger_word(
    ctx: &Context,
    msg: &Message,
    self_id: UserId,
    conf: &Config,
) -> bool {
    if msg.author.id == self_id {
        return false;
    }

    let trigger_words = env::var("TRIGGER_WORDS").unwrap();
    let trigger_words = trigger_words
        .split('\n')
        .collect::<Vec<&str>>();
    if !trigger_words.iter().any(|word| msg.content.contains(word)) {
        return false;
    }

    react(ctx, msg, self_id, conf).await;

    true
}

pub async fn get_messages_chain(
    ctx: &Context,
    msg: &Message,
    last_messages_number: u8,
) -> eyre::Result<Vec<Message>> {
    let channel_id = msg
        .channel(ctx)
        .await
        .wrap_err_with(|| "Error getting channel")?
        .id();
    let mut msg_chain = channel_id
        .messages(ctx, GetMessages::new().limit(last_messages_number))
        .await
        .wrap_err_with(|| "Error getting messages list")?;

    let mut current_msg = msg.clone();
    loop {
        println!("current_msg: {:?}", current_msg.content);
        let next_msg = match current_msg.referenced_message.clone() {
            Some(m) => *m,
            None => {
                println!("there no more referenced_message");
                return Ok(msg_chain);
            }
        };
        let next_msg = msg.channel_id.message(ctx, next_msg.id).await?;
        msg_chain.push(next_msg.clone());
        current_msg = next_msg;
    }
}

pub async fn process_messages(
    ctx: &Context,
    bot_id: UserId,
    conf: &Config,
    msg_chain: &Vec<Message>,
) -> eyre::Result<Vec<ChatMessage>> {
    let mut messages_chain: Vec<MyChatMessage> = vec![];

    for msg in msg_chain {
        let msg_id = msg.id;
        for messages_chain_item in messages_chain.iter() {
            if msg_id == messages_chain_item.msg_id {
                continue;
            }
        }

        let author_id = msg.author.id;
        let author_name = get_user_name(&author_id, &ctx.http, conf).await;
        let role = match author_id {
            id if id == bot_id => MessageRole::Assistant,
            _ => MessageRole::User,
        };
        let content = match role {
            MessageRole::User => format!("@{author_id} \"{author_name}\": {}", msg.content),
            _ => msg.content.clone(),
        };

        let llm_message = ChatMessage {
            role,
            content,
            images: None,
            tool_calls: vec![],
        };

        messages_chain.push(MyChatMessage {
            cm: llm_message,
            msg_id: msg.id,
            timestamp: msg.timestamp,
        });
    }

    // Sort by timestamp in ascending order
    messages_chain.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    match messages_chain.binary_search_by(|msg| msg.timestamp.cmp(&msg.timestamp)) {
        Ok(index) => messages_chain.truncate(index + 1),
        Err(_) => println!("================================\nTimestamp not found in messages.\n================================")
    };

    Ok(messages_chain
        .iter()
        .map(|m| m.cm.clone())
        .collect::<Vec<ChatMessage>>())
}

/// Replaces all `<@ID>` occurrences in the input string with the user name
/// obtained asynchronously via `get_user_name`.
pub async fn replace_mentions(input: &str, cache_http: impl CacheHttp, conf: &Config) -> String {
    let re = Regex::new(r"<@(\d+)>").unwrap();
    let mut result = String::with_capacity(input.len());
    let mut last_match_end = 0;

    // Iterate over all regex captures in the input.
    for caps in re.captures_iter(input) {
        let m = caps.get(0).unwrap();
        // Append the segment of text before the current match.
        result.push_str(&input[last_match_end..m.start()]);
        let id_str = &caps[1];
        let id = id_str.parse::<u64>().unwrap_or(0);
        let user_id = UserId::new(id);
        let user_name = get_user_name(&user_id, &cache_http, conf).await;
        result.push_str(&user_name);
        last_match_end = m.end();
    }
    // Append any remaining text after the last match.
    result.push_str(&input[last_match_end..]);
    result
}

/// Removes all `<think>...</think>` blocks from the input string.
///
/// # Arguments
///
/// * `input` - A multiline string slice.
///
/// # Returns
///
/// A new `String` with the `<think>...</think>` blocks removed.
///
/// # Examples
///
/// ```
/// let text = "Start\n<think>\nHidden content\n</think>\nEnd";
/// let result = remove_think_blocks(text);
/// assert_eq!(result, "Start\n\nEnd");
/// ```
pub fn remove_think_blocks(input: &str) -> String {
    // Use (?s) to enable DOTALL so '.' matches newlines.
    let re = Regex::new(r"(?s)<think>.*?</think>").unwrap();
    re.replace_all(input, "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_single_think_block() {
        let input = "This is a test.\n<think>\nSome thoughts\nspanning multiple lines.\n</think>\nEnd of text.";
        let expected = "This is a test.\n\nEnd of text.";
        assert_eq!(remove_think_blocks(input), expected);
    }

    #[test]
    fn test_remove_multiple_think_blocks() {
        let input = "Hello <think>\nBlock one\n</think> world <think>another block</think>!";
        let expected = "Hello  world !";
        assert_eq!(remove_think_blocks(input), expected);
    }

    #[test]
    fn test_no_think_block() {
        let input = "No think blocks here.";
        let expected = "No think blocks here.";
        assert_eq!(remove_think_blocks(input), expected);
    }
}
