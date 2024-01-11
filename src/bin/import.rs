use clap::Parser;
use serde::Deserialize;
use serde_json::from_str;
use std::{fs, path::PathBuf};
use telegram_cjk_search_bot::{db::Db, types};
use teloxide::prelude::*;
use teloxide::types::ChatId;

#[derive(Deserialize)]
struct Content {
    name: String,
    #[serde(rename = "type")]
    chat_type: String,
    id: ChatId,
    messages: Vec<Message>,
}

#[derive(Deserialize)]
struct Entity {
    text: String,
}

#[derive(Deserialize)]
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

#[derive(Parser)]
#[command(author, version, long_about = None)]
#[command(about = "Import chat history from a json file to meilisearch db.")]
struct Cli {
    #[arg(default_value = "./history/result.json")]
    file: PathBuf,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    Db::new().init().await;
    let cli = Cli::parse();

    let bot_username = format!("@{}", Bot::from_env().get_me().await.unwrap().username());
    let content = read_content_from_file(&cli.file);

    assert!(
        content.chat_type.contains("supergroup"),
        "Chat type must be 'supergroup'"
    );
    log::info!("Paresed {} items.", content.messages.len());

    let (messages_count, handles) = process_messages(content, bot_username);
    log::info!(
        "Inserting {} messages. Waiting for completion.",
        messages_count
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

fn process_messages(
    content: Content,
    bot_username: String,
) -> (usize, Vec<tokio::task::JoinHandle<()>>) {
    let mut messages_count = 0;
    let mut handles = Vec::new();
    let mut messages_batch = Vec::with_capacity(INSERT_BATCH_LIMIT);

    for message in content.messages {
        if let Some(m) = to_db_message(&bot_username, &message, &content.id, &content.name) {
            messages_batch.push(m);
            messages_count += 1;

            if messages_batch.len() >= INSERT_BATCH_LIMIT {
                handles.push(spawn_insert_messages_task(Db::new(), messages_batch));
                messages_batch = Vec::with_capacity(INSERT_BATCH_LIMIT);
            }
        }
    }

    if !messages_batch.is_empty() {
        handles.push(spawn_insert_messages_task(Db::new(), messages_batch));
    }

    (messages_count, handles)
}

fn to_db_message(
    bot_username: &str,
    message: &Message,
    chat_id: &ChatId,
    chat_name: &str,
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
            from: format!(
                "{}@{}",
                message
                    .from
                    .as_ref()
                    .unwrap_or(&format!("已销号{}", from_id)),
                chat_name
            ),
            id: message.id,
            chat_id: ChatId(format!("-100{}", chat_id).parse::<i64>().unwrap()),
            date: chrono::DateTime::from_utc(
                chrono::NaiveDateTime::from_timestamp_opt(
                    message.date_unixtime.parse::<i64>().unwrap(),
                    0,
                )
                .unwrap(),
                chrono::Utc,
            ),
        })
    })
}

fn spawn_insert_messages_task(
    db: Db,
    messages: Vec<types::Message>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        db.insert_messages(&messages).await;
    })
}

#[cfg(test)]
mod import_test {
    use super::*;

    #[test]
    fn private_chat_message_test() {
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

        assert!(to_db_message("1", &msg, &ChatId(1), "1").is_none());
    }

    #[test]
    fn serivce_message_test() {
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

        assert!(to_db_message("1", &msg, &ChatId(1), "1").is_none());
    }

    #[test]
    fn empty_message_test() {
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

        assert!(to_db_message("1", &msg, &ChatId(1), "1").is_none());
    }

    #[test]
    fn normal_message_test() {
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
            serde_json::to_string(
                &to_db_message("1", &msg, &ChatId(1145141919), "Genshin Impact").unwrap()
            )
            .unwrap(),
            serde_json::to_string(&genuine_msg).unwrap()
        );
    }
}
