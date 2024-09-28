use clap::Parser;
use serde::Deserialize;
use serde_json::from_str;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc, time::Duration};
use telegram_cjk_search_bot::{
    db::{Db, Insertable},
    types,
};
use teloxide::prelude::*;
use teloxide::types::ChatId;
use tokio::{sync::Mutex, task::JoinHandle};

#[derive(Deserialize)]
struct Content {
    #[serde(rename = "type")]
    chat_type: String,
    name: String,
    id: ChatId,
    messages: Vec<Message>,
}

#[derive(Deserialize, Clone)]
struct Entity {
    text: String,
}

#[derive(Deserialize, Clone)]
struct Message {
    id: i32,
    #[serde(rename = "type")]
    message_type: String,
    date_unixtime: String,
    from: Option<String>,
    from_id: Option<String>,
    via_bot: Option<String>,
    text_entities: Vec<Entity>,
}

const INSERT_BATCH_LIMIT: usize = 2000;
const MAX_MARKED_CHANNEL_ID: i64 = -1000000000000;

#[derive(Parser)]
#[command(author, version, long_about = None)]
#[command(about = "Import chat history from a json file to meilisearch db.")]
struct Cli {
    #[arg(default_value = "./history/result.json")]
    file: PathBuf,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    Db::new().init().await;
    let cli = Cli::parse();

    let bot_username = format!("@{}", Bot::from_env().get_me().await.unwrap().username());
    let content = read_content_from_file(&cli.file);

    assert!(
        content.chat_type.contains("supergroup"),
        "Chat type must be 'supergroup'"
    );
    log::info!("Paresed {} items.", content.messages.len());

    let (messages_count, senders_count, handles) = process_messages(content, bot_username).await;
    log::info!(
        "Inserting {messages_count} messages and {senders_count} senders. Waiting for completion."
    );

    futures::future::join_all(handles).await;
    log::info!("Done.");
}

fn read_content_from_file(file_path: &PathBuf) -> Content {
    let file_content = fs::read_to_string(file_path)
        .unwrap_or_else(|_| panic!("Failed to read file {:?}", file_path));
    log::info!("File {} has been read. Parsing...", file_path.display());
    from_str::<Content>(&file_content).expect("Failed to parse content from file")
}

async fn process_messages(
    content: Content,
    bot_username: String,
) -> (usize, usize, Vec<JoinHandle<()>>) {
    let messages_count = Arc::new(Mutex::new(0));
    let senders = Arc::new(Mutex::new(HashMap::<ChatId, String>::new()));

    let mut handles = futures::future::join_all(
        content
            .messages
            .chunks(INSERT_BATCH_LIMIT)
            .map(|c| c.to_vec())
            .map(|c| {
                let messages_count = messages_count.clone();
                let senders = senders.clone();
                let bot_username = bot_username.clone();
                tokio::spawn(async move {
                    let mut messages_batch = Vec::with_capacity(INSERT_BATCH_LIMIT);
                    for message in c {
                        if let Some(m) = to_db_message(&bot_username, &message, &content.id).await {
                            let f = message
                                .from
                                .unwrap_or(format!("Deleted Account {}", m.sender.unwrap()));
                            senders
                                .lock()
                                .await
                                .entry(m.sender.unwrap())
                                .and_modify(|e| *e = f.clone())
                                .or_insert(f);
                            messages_batch.push(m)
                        }
                    }
                    *messages_count.lock().await += messages_batch.len();
                    spawn_insert_task(messages_batch)
                })
            }),
    )
    .await
    .into_iter()
    .map(|x| x.unwrap())
    .collect::<Vec<_>>();

    senders
        .lock()
        .await
        .entry(content.id)
        .and_modify(|e| *e = content.name.clone())
        .or_insert(content.name);
    handles.push(spawn_insert_task(
        senders
            .lock()
            .await
            .clone()
            .into_iter()
            .map(|(id, name)| types::Sender { id, name })
            .collect::<Vec<_>>(),
    ));

    let res = (
        *messages_count.lock().await,
        senders.lock().await.len(),
        handles,
    );
    res
}

async fn to_db_message(
    bot_username: &str,
    message: &Message,
    chat_id: &ChatId,
) -> Option<types::Message> {
    if message.message_type != "message"
        || message
            .via_bot
            .as_ref()
            .and_then(|u| Some(u == bot_username))
            .unwrap_or(false)
        || message.id < 1
    {
        return None;
    }

    let text = message
        .text_entities
        .iter()
        .map(|e| e.text.clone())
        .collect::<String>();
    if text.is_empty() {
        return None;
    }

    message.from_id.as_ref().and_then(|from_id| {
        Some(types::Message {
            key: format!("-100{}_{}", chat_id, message.id),
            text,
            from: None,
            sender: Some(match from_id.starts_with("user") {
                true => UserId(from_id[4..].parse::<u64>().unwrap()).into(),
                false => ChatId(MAX_MARKED_CHANNEL_ID - from_id[7..].parse::<i64>().unwrap()),
            }),
            id: message.id,
            via_bot: message.via_bot.clone(),
            chat_id: ChatId(format!("-100{}", chat_id).parse::<i64>().unwrap()),
            date: chrono::DateTime::from_timestamp(message.date_unixtime.parse().unwrap(), 0)
                .unwrap(),
        })
    })
}

fn spawn_insert_task<T>(items: Vec<T>) -> JoinHandle<()>
where
    T: Insertable + Sync + Send + 'static,
{
    tokio::spawn(async move {
        if let Some(t) = Db::new().insert(&items).await {
            t.wait_for_completion(
                &Db::new().0,
                Some(Duration::from_millis(200)),
                Some(Duration::MAX),
            )
            .await
            .unwrap();
        };
    })
}

#[cfg(test)]
mod import_test {
    use super::*;

    #[tokio::test]
    async fn private_chat_message_test() {
        let msg = serde_json::from_str::<super::Message>(
            r#"{
            "id": -999972078,
            "type": "message",
            "date": "2023-07-19T09:11:13",
            "date_unixtime": "1689729073",
            "from": "Kris Hu",
            "from_id": "user114514",
            "text": "1",
            "text_entities": [
             {
              "type": "plain",
              "text": "1"
             }
            ]
           }
        "#,
        )
        .unwrap();

        assert!(to_db_message("1", &msg, &ChatId(1)).await.is_none());
    }

    #[tokio::test]
    async fn serivce_message_test() {
        let msg = serde_json::from_str::<super::Message>(
            r#"{
                "id": 15,
                "type": "service",
                "date": "2023-07-19T23:30:23",
                "date_unixtime": "1689780623",
                "actor": "Kris Hu",
                "actor_id": "user114514",
                "action": "invite_members",
                "members": [
                 "1919810"
                ],
                "text": "",
                "text_entities": []
            }
        "#,
        )
        .unwrap();

        assert!(to_db_message("1", &msg, &ChatId(1)).await.is_none());
    }

    #[tokio::test]
    async fn empty_message_test() {
        let msg = serde_json::from_str::<super::Message>(
            r#"{
                "id": 4,
                "type": "message",
                "date": "2023-07-19T09:53:29",
                "date_unixtime": "1689731609",
                "from": "Kris Hu",
                "from_id": "user114514",
                "file": "(File not included. Change data exporting settings to download.)",
                "media_type": "voice_message",
                "mime_type": "audio/ogg",
                "duration_seconds": 4,
                "text": "",
                "text_entities": []
            }
        "#,
        )
        .unwrap();

        assert!(to_db_message("1", &msg, &ChatId(1)).await.is_none());
    }

    #[tokio::test]
    async fn normal_message_test() {
        let msg = serde_json::from_str::<super::Message>(
            r#"
            {
                "id": 346,
                "type": "message",
                "date": "2024-01-10T17:20:31",
                "date_unixtime": "1704878431",
                "from": "Kris Hu",
                "from_id": "user114514",
                "text": "还真是",
                "text_entities": [
                 {
                  "type": "plain",
                  "text": "还真是"
                 }
                ]
               }
        "#,
        )
        .unwrap();

        let genuine_msg = telegram_cjk_search_bot::types::Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"
            {
                "message_id": 346,
                "message_thread_id": null,
                "date": 1704878431,
                "chat": {
                    "id": -1001145141919,
                    "title": "Genshin Impact",
                    "type": "supergroup",
                    "is_forum": false
                },
                "via_bot": null,
                "from": {
                    "id": 114514,
                    "is_bot": false,
                    "first_name": "Kris",
                    "last_name": "Hu",
                    "username": "Krisssssss",
                    "language_code": "zh-hans"
                },
                "text": "还真是",
                "entities": [],
                "is_topic_message": false,
                "is_automatic_forward": false,
                "has_protected_content": false
        }"#,
            )
            .unwrap(),
        );

        assert_eq!(
            serde_json::to_string(&to_db_message("1", &msg, &ChatId(1145141919)).await.unwrap())
                .unwrap(),
            serde_json::to_string(&genuine_msg).unwrap()
        );
    }

    #[tokio::test]
    async fn via_bot_message_test() {
        let msg = serde_json::from_str::<super::Message>(
            r#"
            {
                "id": 346,
                "type": "message",
                "date": "2024-01-10T17:20:31",
                "date_unixtime": "1704878431",
                "from": "Kris Hu",
                "from_id": "user114514",
                "via_bot": "@TestBot",
                "text": "还真是",
                "text_entities": [
                 {
                  "type": "plain",
                  "text": "还真是"
                 }
                ]
               }
        "#,
        )
        .unwrap();

        let genuine_msg = telegram_cjk_search_bot::types::Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"
            {
                "message_id": 346,
                "message_thread_id": null,
                "date": 1704878431,
                "chat": {
                    "id": -1001145141919,
                    "title": "Genshin Impact",
                    "type": "supergroup",
                    "is_forum": false
                },
                "via_bot": {
                    "id": 1145141919,
                    "is_bot": true,
                    "first_name": "Test Bot",
                    "username": "TestBot"
                },
                "from": {
                    "id": 114514,
                    "is_bot": false,
                    "first_name": "Kris",
                    "last_name": "Hu",
                    "username": "Krisssssss",
                    "language_code": "zh-hans"
                },
                "text": "还真是",
                "entities": [],
                "is_topic_message": false,
                "is_automatic_forward": false,
                "has_protected_content": false
        }"#,
            )
            .unwrap(),
        );

        assert_eq!(
            serde_json::to_string(&to_db_message("1", &msg, &ChatId(1145141919)).await.unwrap())
                .unwrap(),
            serde_json::to_string(&genuine_msg).unwrap()
        );
    }
}
