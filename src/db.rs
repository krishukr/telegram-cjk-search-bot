use std::time::Duration;

use meilisearch_sdk::{
    search::{SearchResults, Selectors},
    Client,
    Error::Meilisearch,
    ErrorCode::DocumentNotFound,
    MeilisearchError,
};
use serde::{de::DeserializeOwned, Serialize};
use teloxide::types::ChatId;

use crate::types::*;

const GET_LIMIT: usize = 100;

pub struct Db(Client);

impl Db {
    pub fn new() -> Self {
        Db(Client::new(
            std::env::var("MEILISEARCH_HOST").unwrap(),
            std::env::var("MEILISEARCH_API_KEY").ok(),
        ))
    }

    pub async fn init(self) {
        self.0.create_index("messages", Some("key")).await.unwrap();
        self.0
            .index("messages")
            .set_searchable_attributes(&["text"])
            .await
            .unwrap();
        self.0
            .index("messages")
            .set_filterable_attributes(&["chat_id"])
            .await
            .unwrap();
        self.0
            .index("messages")
            .set_ranking_rules([
                "words",
                "typo",
                "proximity",
                "attribute",
                "sort",
                "exactness",
                "date:desc",
            ])
            .await
            .unwrap();
        self.0.create_index("chats", Some("id")).await.unwrap();
        self.0
            .index("chats")
            .set_searchable_attributes(Vec::<String>::new())
            .await
            .unwrap();
        self.0.create_index("senders", Some("id")).await.unwrap();
        self.0
            .index("senders")
            .set_searchable_attributes(Vec::<String>::new())
            .await
            .unwrap();
    }

    pub async fn insert_messages(self, msgs: &Vec<Message>) {
        self.insert_documents("messages", msgs, Some("key")).await;
    }

    pub async fn search_message_with_filter_chats(
        self,
        text: &String,
        chats: &Vec<Chat>,
    ) -> SearchResults<Message> {
        log::debug!(
            "search message with filter {}",
            format!("chat_id IN {:?}", chats)
        );
        self.0
            .index("messages")
            .search()
            .with_query(text)
            .with_filter(&format!("chat_id IN {:?}", chats))
            .with_attributes_to_crop(Selectors::Some(&[("text", None)]))
            .with_crop_length(match check_contain_utf8(text) {
                true => 15,
                false => 6,
            })
            .execute::<Message>()
            .await
            .unwrap()
    }

    pub async fn insert_chat_with_id(self, id: ChatId) {
        self.0
            .index("chats")
            .add_documents(&[Chat::from(id)], Some("id"))
            .await
            .unwrap();
    }

    pub async fn delete_chat_with_id(self, id: ChatId) {
        self.0.index("chats").delete_document(id).await.unwrap();
    }

    pub async fn filter_chat_with_id(self, id: ChatId) -> Option<Chat> {
        self.get_one_document("chats", id.to_string().as_str())
            .await
    }

    pub async fn get_all_chats(self) -> Vec<ChatId> {
        let mut res: Vec<ChatId> = Vec::new();
        let index = self.0.index("chats");
        let mut query = index.search().with_limit(GET_LIMIT).build();

        let mut offset: usize = 0;
        loop {
            let query_res = query.with_offset(offset).execute::<Chat>().await.unwrap();
            if query_res.hits.is_empty() {
                break;
            }
            res.append(
                &mut query_res
                    .hits
                    .iter()
                    .map(|c| c.result.id)
                    .collect::<Vec<_>>(),
            );
            offset += GET_LIMIT;
        }

        res
    }

    pub async fn get_sender_name(self, id: ChatId) -> Option<String> {
        self.get_one_document("senders", id.to_string().as_str())
            .await
            .map(|s: Sender| s.name)
    }

    pub async fn insert_senders(self, senders: &Vec<Sender>) {
        self.0
            .index("senders")
            .add_documents(senders, Some("id"))
            .await
            .unwrap()
            .wait_for_completion(&self.0, None, None)
            .await
            .unwrap();
    }

    async fn get_one_document<T>(self, index: &str, key: &str) -> Option<T>
    where
        T: DeserializeOwned + 'static,
    {
        match self.0.index(index).get_document(key).await {
            Ok(d) => Some(d),
            Err(Meilisearch(MeilisearchError {
                error_code: DocumentNotFound,
                ..
            })) => None,
            Err(e) => panic!("{e}"),
        }
    }

    async fn insert_documents<T>(self, index: &str, docs: &Vec<T>, key: Option<&str>)
    where
        T: Serialize,
    {
        if docs.is_empty() {
            return;
        }
        log::debug!("{}", serde_json::to_string_pretty(docs).unwrap());
        self.0
            .index(index)
            .add_documents(docs, key)
            .await
            .unwrap()
            .wait_for_completion(
                &self.0,
                if docs.len() > 100 {
                    Some(Duration::from_millis(200))
                } else {
                    None
                },
                if docs.len() > 100 {
                    Some(Duration::from_secs(300))
                } else {
                    None
                },
            )
            .await
            .unwrap();
    }
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

fn check_contain_utf8(s: &String) -> bool {
    for b in s.as_bytes() {
        if *b > 127 {
            return true;
        }
    }
    false
}
