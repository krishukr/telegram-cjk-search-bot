use telegram_cjk_search_bot::*;

use clap::Parser;
use db::Db;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use std::fs;
use std::path::PathBuf;
use teloxide::types::ChatId;

#[derive(Serialize, Deserialize)]
struct Content {
    name: String,
    r#type: String,
    id: ChatId,
    messages: Vec<Message>,
}

#[derive(Serialize, Deserialize)]
struct Entitiy {
    r#type: String,
    text: String,
}

#[derive(Serialize, Deserialize)]
struct Message {
    id: i32,
    r#type: String,
    date_unixtime: String,
    from: Option<String>,
    from_id: Option<String>,
    text_entities: Vec<Entitiy>,
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

    let cli = Cli::parse();
    let mut msgs: Vec<types::Message> = vec![];
    let mut handles = vec![];

    let content = from_str::<Content>(&fs::read_to_string(cli.file).unwrap()).unwrap();
    assert!(content.r#type.contains("supergroup"));
    log::info!("parse succeed.");

    let mut successful_count: usize = 0;
    for m in content.messages {
        if m.r#type != "message" {
            continue;
        }
        if let Some(from_id) = &m.from_id {
            let mut txt = String::new();
            m.text_entities.iter().for_each(|ele| {
                txt.push_str(&ele.text);
            });
            if txt.is_empty() {
                continue;
            }
            msgs.push(types::Message {
                key: format!("-100{}_{}", &content.id, m.id),
                text: txt,
                from: format!(
                    "{}@{}",
                    match m.from {
                        Some(f) => f,
                        None => format!("已销号{}", from_id),
                    },
                    &content.name
                ),
                id: m.id,
                chat_id: teloxide::types::ChatId(
                    format!("-100{}", content.id).parse::<i64>().unwrap(),
                ),
                date: chrono::DateTime::from_utc(
                    chrono::NaiveDateTime::from_timestamp_opt(
                        m.date_unixtime.parse::<i64>().unwrap(),
                        0,
                    )
                    .unwrap(),
                    chrono::Utc,
                ),
            });
            successful_count += 1;
            if msgs.len() >= INSERT_BATCH_LIMIT {
                handles.push(tokio::spawn(async move {
                    let imsgs = msgs;
                    Db::new().insert_messages(&imsgs).await;
                }));
                msgs = vec![];
            }
        }
    }
    if !msgs.is_empty() {
        handles.push(tokio::spawn(async move {
            let imsgs = msgs;
            Db::new().insert_messages(&imsgs).await;
        }));
    }

    log::info!(
        "insert {} messages. waiting for complete.",
        successful_count
    );
    futures::future::join_all(handles).await;
    log::info!("done.");
}
