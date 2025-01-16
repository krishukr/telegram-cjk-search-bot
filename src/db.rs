use meilisearch_sdk::{
    search::{SearchResults, Selectors},
    Client,
    Error::Meilisearch,
    ErrorCode::DocumentNotFound,
    MeilisearchError, TaskInfo,
};
use serde::{de::DeserializeOwned, Serialize};
use teloxide::types::ChatId;

use crate::types::*;

const GET_LIMIT: usize = 100;
pub const INLINE_REPLY_LIMIT: usize = 20;

pub struct Db(pub Client);

pub trait Insertable: Serialize {
    const INDEX: &'static str;
    const KEY: Option<&'static str>;

    fn init(db: &Db) -> impl std::future::Future<Output = ()> + Send;
}

pub enum FilterOption<'a, T> {
    Some(&'a Vec<T>),
    All,
    None,
}

pub enum EnableOption {
    All,
    Enable,
    Disable,
}

pub struct Filter<'a> {
    pub chats: Vec<Chat>,
    pub include_bots: FilterOption<'a, String>,
    pub only_bots: FilterOption<'a, String>,
    pub urls: EnableOption,
}

impl Db {
    pub fn new() -> Self {
        Db(Client::new(
            std::env::var("MEILISEARCH_HOST").unwrap(),
            std::env::var("MEILISEARCH_API_KEY").ok(),
        ))
    }

    pub async fn init(self) {
        <Message as Insertable>::init(&self).await;
        <Chat as Insertable>::init(&self).await;
        <Sender as Insertable>::init(&self).await;
    }

    pub async fn search_message_with_filter(
        self,
        text: &String,
        filter: &Filter<'_>,
        offset: Option<usize>,
    ) -> SearchResults<Message> {
        log::debug!("search message with filter {}", filter.render());
        self.0
            .index(Message::INDEX)
            .search()
            .with_limit(INLINE_REPLY_LIMIT)
            .with_offset(offset.unwrap_or_default())
            .with_query(text)
            .with_filter(&filter.render())
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
        self.insert(&vec![Chat::from(id)]).await;
    }

    pub async fn delete_chat_with_id(self, id: ChatId) {
        self.0.index(Chat::INDEX).delete_document(id).await.unwrap();
    }

    pub async fn filter_chat_with_id(self, id: ChatId) -> Option<Chat> {
        self.get_one_document(Chat::INDEX, id.to_string().as_str())
            .await
    }

    pub async fn get_all_chats(self) -> Vec<ChatId> {
        let mut res: Vec<ChatId> = Vec::new();
        let index = self.0.index(Chat::INDEX);
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
        self.get_one_document(Sender::INDEX, id.to_string().as_str())
            .await
            .map(|s: Sender| s.name)
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

    pub async fn insert<T>(self, docs: &Vec<T>) -> Option<TaskInfo>
    where
        T: Insertable,
    {
        self.insert_documents(T::INDEX, docs, T::KEY).await
    }

    async fn insert_documents<T>(
        self,
        index: &str,
        docs: &Vec<T>,
        key: Option<&str>,
    ) -> Option<TaskInfo>
    where
        T: Serialize,
    {
        if docs.is_empty() {
            return None;
        }
        log::debug!("{}", serde_json::to_string_pretty(docs).unwrap());
        Some(self.0.index(index).add_documents(docs, key).await.unwrap())
    }
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

impl Insertable for Message {
    const INDEX: &'static str = "messages";
    const KEY: Option<&'static str> = Some("key");

    async fn init(db: &Db) {
        let client = &db.0;
        client.create_index(Self::INDEX, Self::KEY).await.unwrap();
        client
            .index(Self::INDEX)
            .set_searchable_attributes(&["text"])
            .await
            .unwrap();
        client
            .index(Self::INDEX)
            .set_filterable_attributes(&["chat_id", "via_bot", "web_page"])
            .await
            .unwrap();
        client
            .index(Self::INDEX)
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
    }
}

impl Insertable for Chat {
    const INDEX: &'static str = "chats";
    const KEY: Option<&'static str> = Some("id");

    async fn init(db: &Db) {
        let client = &db.0;
        client.create_index(Self::INDEX, Self::KEY).await.unwrap();
        client
            .index(Self::INDEX)
            .set_searchable_attributes(Vec::<String>::new())
            .await
            .unwrap();
    }
}

impl Insertable for Sender {
    const INDEX: &'static str = "senders";
    const KEY: Option<&'static str> = Some("id");

    async fn init(db: &Db) {
        let client = &db.0;
        client.create_index(Self::INDEX, Self::KEY).await.unwrap();
        client
            .index(Self::INDEX)
            .set_searchable_attributes(Vec::<String>::new())
            .await
            .unwrap();
    }
}

impl Filter<'_> {
    fn render(&self) -> String {
        format!(
            "chat_id IN {:?}{}{}{}",
            self.chats,
            match &self.include_bots {
                FilterOption::Some(x) => format!(" AND (via_bot NOT EXISTS OR via_bot IN {:?})", x),
                FilterOption::All => "".to_string(),
                FilterOption::None => " AND via_bot NOT EXISTS".to_string(),
            },
            match &self.only_bots {
                FilterOption::Some(x) => format!(" AND via_bot IN {:?}", x),
                FilterOption::All => " AND via_bot EXISTS".to_string(),
                FilterOption::None => "".to_string(),
            },
            match self.urls {
                EnableOption::All => " AND web_page EXISTS".to_string(),
                EnableOption::Enable => String::default(),
                EnableOption::Disable => " AND web_page NOT EXISTS".to_string(),
            }
        )
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
