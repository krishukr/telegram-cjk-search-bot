use meilisearch_sdk::{
    search::{SearchResults, Selectors},
    Client,
};
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
        self.0
            .index("chats")
            .set_filterable_attributes(&["id"])
            .await
            .unwrap();
    }

    pub async fn insert_messages(self, msgs: &Vec<Message>) {
        if msgs.is_empty() {
            return;
        }
        log::debug!("{}", serde_json::to_string_pretty(&msgs).unwrap());
        self.0
            .index("messages")
            .add_documents(msgs, Some("key"))
            .await
            .unwrap();
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
            .with_crop_length(match Self::check_contain_utf8(text) {
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
        self.0
            .index("chats")
            .search()
            .with_filter(&format!("id = {}", id))
            .execute::<Chat>()
            .await
            .unwrap()
            .hits
            .get(0)
            .map(|c| c.result)
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

    fn check_contain_utf8(s: &String) -> bool {
        for b in s.as_bytes() {
            if *b > 127 {
                return true;
            }
        }
        false
    }
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}
