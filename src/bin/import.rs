use clap::Parser;
use futures::future::join_all;
use reqwest::Url;
use serde::Deserialize;
use serde_json::from_str;
use std::{collections::HashMap, fs, path::PathBuf, sync::Arc, time::Duration};
use telegram_cjk_search_bot::{
    db::{Db, Insertable},
    handlers::get_url_in_whitelist,
    ogp::read_open_graph,
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
    #[serde(rename = "type")]
    entity_type: String,
    text: String,
    href: Option<String>,
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
    #[arg(default_value = "/app/history/result.json")]
    file: PathBuf,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();
    Db::new().init().await;
    let cli = Cli::parse();

    let me = Bot::from_env().get_me().await.unwrap();
    let bot_username = format!("@{}", me.username());
    let bot_userid = me.id;
    let content = read_content_from_file(&cli.file);

    assert!(
        content.chat_type.contains("supergroup"),
        "Chat type must be 'supergroup'"
    );
    log::info!("Paresed {} items.", content.messages.len());

    let (messages_count, url_count, senders_count, message_handles, web_page_handles) =
        process_messages(content, bot_username, bot_userid).await;
    log::info!("Found {messages_count} messages, {url_count} URLs, and {senders_count} senders.");

    log::info!("Crawling web pages.");
    join_all(process_web_pages(web_page_handles).await).await;

    log::info!("Waiting for database to complete indexing.");
    join_all(message_handles).await;
    log::info!("Done.");
}

fn read_content_from_file(file_path: &PathBuf) -> Content {
    let file_content = fs::read_to_string(file_path)
        .unwrap_or_else(|_| panic!("Failed to read file {:?}", file_path));
    log::info!("File {} has been read. Parsing...", file_path.display());
    from_str::<Content>(&file_content).expect("Failed to parse content from file")
}

#[allow(clippy::all)]
async fn process_messages(
    content: Content,
    bot_username: String,
    bot_userid: UserId,
) -> (
    usize,
    usize,
    usize,
    Vec<JoinHandle<()>>,
    Vec<JoinHandle<Option<types::Message>>>,
) {
    let messages_count = Arc::new(Mutex::new(0));
    let url_count = Arc::new(Mutex::new(0));
    let senders = Arc::new(Mutex::new(HashMap::<ChatId, String>::new()));

    let web_page_handles = Arc::new(Mutex::new(vec![]));
    let mut handles = futures::future::join_all(
        content
            .messages
            .chunks(INSERT_BATCH_LIMIT)
            .map(|c| c.to_vec())
            .map(|c| {
                let messages_count = messages_count.clone();
                let url_count = url_count.clone();
                let senders = senders.clone();
                let web_pages_handles = web_page_handles.clone();
                let bot_username = bot_username.clone();
                tokio::spawn(async move {
                    let mut messages_batch = Vec::with_capacity(INSERT_BATCH_LIMIT);
                    let mut web_pages = vec![];
                    for message in c {
                        if let Some(m) =
                            to_db_message(&bot_username, bot_userid, &message, &content.id).await
                        {
                            let f = message
                                .from
                                .unwrap_or(format!("Deleted Account {}", m.sender.unwrap()));
                            senders
                                .lock()
                                .await
                                .entry(m.sender.unwrap())
                                .and_modify(|e| *e = f.clone())
                                .or_insert(f);
                            messages_batch.push(m.clone());

                            web_pages.extend(
                                message
                                    .text_entities
                                    .iter()
                                    .filter_map(|e| match e.entity_type.as_str() {
                                        "link" => Url::parse(&e.text.clone()).ok(),
                                        "text_link" => Url::parse(&e.href.clone()?).ok(),
                                        _ => None,
                                    })
                                    .filter_map(|u| {
                                        let m = m.clone();
                                        let u = get_url_in_whitelist(&u)?;
                                        Some(tokio::spawn(async move {
                                            Some(m.clone().set_web_page(&read_open_graph(u).await?))
                                        }))
                                    }),
                            );
                        }
                    }
                    *messages_count.lock().await += messages_batch.len();
                    *url_count.lock().await += web_pages.len();
                    web_pages_handles.lock().await.extend(web_pages.into_iter());
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
        *url_count.lock().await,
        senders.lock().await.len(),
        handles,
        Arc::try_unwrap(web_page_handles).unwrap().into_inner(),
    );
    res
}

async fn to_db_message(
    bot_username: &str,
    bot_userid: UserId,
    message: &Message,
    chat_id: &ChatId,
) -> Option<types::Message> {
    if message.message_type != "message"
        || message.via_bot.as_ref().is_some_and(|u| u == bot_username)
        || message
            .from_id
            .as_ref()
            .is_some_and(|u| u[4..] == bot_userid.0.to_string())
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

    message.from_id.as_ref().map(|from_id| types::Message {
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
        date: chrono::DateTime::from_timestamp(message.date_unixtime.parse().unwrap(), 0).unwrap(),
        web_page: None,
        thumbnail_url: None,
    })
}

async fn process_web_pages(
    handles: Vec<JoinHandle<Option<types::Message>>>,
) -> Vec<JoinHandle<()>> {
    join_all(handles)
        .await
        .into_iter()
        .filter_map(|x| x.unwrap())
        .collect::<Vec<_>>()
        .chunks(INSERT_BATCH_LIMIT)
        .map(|x| x.to_vec())
        .map(spawn_insert_task)
        .collect::<Vec<_>>()
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

        assert!(to_db_message("1", UserId(1), &msg, &ChatId(1))
            .await
            .is_none());
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

        assert!(to_db_message("1", UserId(1), &msg, &ChatId(1))
            .await
            .is_none());
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

        assert!(to_db_message("1", UserId(1), &msg, &ChatId(1))
            .await
            .is_none());
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
            serde_json::to_string(
                &to_db_message("1", UserId(1), &msg, &ChatId(1145141919))
                    .await
                    .unwrap()
            )
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
            serde_json::to_string(
                &to_db_message("1", UserId(1), &msg, &ChatId(1145141919))
                    .await
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_string(&genuine_msg).unwrap()
        );
    }

    #[tokio::test]
    async fn from_bot_test() {
        let msg = serde_json::from_str::<super::Message>(
            r#"
            {
                "id": 346,
                "type": "message",
                "date": "2024-01-10T17:20:31",
                "date_unixtime": "1704878431",
                "from": "Bot",
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

        assert!(
            to_db_message("Bot", UserId(114514), &msg, &ChatId(1145141919))
                .await
                .is_none()
        );
    }
}
