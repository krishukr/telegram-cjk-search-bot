use std::{env, str::FromStr};

use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use teloxide::types::{ChatId, MessageId};

use crate::ogp::WebPage;

#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct Chat {
    pub id: ChatId,
}

impl std::fmt::Debug for Chat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id)
    }
}

impl From<teloxide::types::ChatId> for Chat {
    fn from(id: teloxide::types::ChatId) -> Self {
        Self { id }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Sender {
    pub id: ChatId,
    pub name: String,
}

impl Sender {
    pub fn from(msg: &teloxide::types::Message) -> Vec<Self> {
        vec![
            {
                let (id, name) = msg
                    .sender_chat
                    .as_ref()
                    .map(|c| (c.id, c.title().unwrap().to_string()))
                    .unwrap_or_else(|| {
                        (
                            msg.from.as_ref().unwrap().id.into(),
                            msg.from.as_ref().unwrap().full_name(),
                        )
                    });
                Self { id, name }
            },
            Self {
                id: msg.chat.id,
                name: msg.chat.title().unwrap().to_string(),
            },
        ]
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    pub key: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub sender: Option<ChatId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_bot: Option<String>,
    pub id: i32,
    pub chat_id: ChatId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_page: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<Url>,
    pub date: DateTime<Utc>,
}

impl From<&teloxide::types::Message> for Message {
    fn from(msg: &teloxide::types::Message) -> Self {
        Self {
            key: format!("{}_{}", msg.chat.id, msg.id),
            text: msg.text().or(msg.caption()).unwrap().to_string(),
            from: None,
            sender: Some(
                msg.sender_chat
                    .as_ref()
                    .map(|c| c.id)
                    .unwrap_or_else(|| msg.from.as_ref().unwrap().id.into()),
            ),
            via_bot: msg
                .via_bot
                .as_ref()
                .map(|u| format!("@{}", u.username.clone().unwrap())),
            id: msg.id.0,
            web_page: None,
            thumbnail_url: None,
            chat_id: msg.chat.id,
            date: msg.date,
        }
    }
}

impl Message {
    pub fn format_time(&self) -> String {
        self.date
            .with_timezone(&Tz::from_str(&env::var("TZ").unwrap_or_default()).unwrap_or_default())
            .format("%Y-%m-%d")
            .to_string()
    }

    pub fn link(&self) -> String {
        teloxide::types::Message::url_of(self.chat_id, None, MessageId(self.id))
            .unwrap()
            .to_string()
    }

    pub fn set_web_page(mut self, page: &WebPage) -> Self {
        self.web_page = Some(page.url.clone());
        self.thumbnail_url = page.thumbnail_url.clone();
        self.key = format!(
            "{}_{}",
            self.key,
            crc32fast::hash(page.url.as_str().as_bytes())
        );
        self.text = html_escape::decode_html_entities(&format!("{}\n{}", page.title, page.desc))
            .to_string();
        self
    }
}

#[cfg(test)]
mod types_tests {
    use super::*;

    #[test]
    fn message_date_format_test() {
        std::env::set_var("TZ", "Asia/Shanghai");
        let msg = Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"{
            "message_id": 2,
            "message_thread_id": null,
            "date": 1689699600,
            "chat": {
                "id": -1001,
                "title": "test",
                "type": "supergroup",
                "is_forum": false
            },
            "via_bot": null,
            "from": {
                "id": 1,
                "is_bot": false,
                "first_name": "Fop",
                "last_name": "Bar",
                "username": "Foo_Bar",
                "language_code": "zh-hans"
            },
            "text": "2",
            "entities": [],
            "is_topic_message": false,
            "is_automatic_forward": false,
            "has_protected_content": false
        }"#,
            )
            .unwrap(),
        );
        assert_eq!(msg.format_time(), "2023-07-19");
    }

    #[test]
    fn message_channel_anonymous_test() {
        let msg = Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"{
            "message_id": 139417,
            "message_thread_id": null,
            "date": 1689781753,
            "chat": {
                "id": -1002,
                "title": "test2",
                "type": "supergroup",
                "is_forum": false
            },
            "via_bot": null,
            "from": {
                "id": 1,
                "is_bot": true,
                "first_name": "Group",
                "username": "GroupAnonymousBot"
            },
            "sender_chat": {
                "id": -1002,
                "title": "test2",
                "type": "supergroup",
                "is_forum": false
            },
            "text": "test",
            "entities": [],
            "is_topic_message": false,
            "is_automatic_forward": false,
            "has_protected_content": false
        }"#,
            )
            .unwrap(),
        );
        assert_eq!(msg.sender.unwrap(), ChatId(-1002));
    }

    #[test]
    fn message_user_test() {
        let msg = Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"{
            "message_id": 2,
            "message_thread_id": null,
            "date": 1689731458,
            "chat": {
                "id": -1001,
                "title": "test",
                "type": "supergroup",
                "is_forum": false
            },
            "via_bot": null,
            "from": {
                "id": 1,
                "is_bot": false,
                "first_name": "Foo",
                "last_name": "Bar",
                "username": "Foo_Bar",
                "language_code": "zh-hans"
            },
            "text": "2",
            "entities": [],
            "is_topic_message": false,
            "is_automatic_forward": false,
            "has_protected_content": false
            }"#,
            )
            .unwrap(),
        );
        assert_eq!(
            msg.sender.unwrap(),
            teloxide::types::ChatId::from(teloxide::types::UserId(1))
        );
    }

    #[test]
    fn message_text_form_text_test() {
        let msg = Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"{
            "message_id": 2,
            "message_thread_id": null,
            "date": 1689731458,
            "chat": {
                "id": -1001,
                "title": "test",
                "type": "supergroup",
                "is_forum": false
            },
            "via_bot": null,
            "from": {
                "id": 1,
                "is_bot": false,
                "first_name": "Foo",
                "last_name": "Bar",
                "username": "Foo_Bar",
                "language_code": "zh-hans"
            },
            "text": "2",
            "entities": [],
            "is_topic_message": false,
            "is_automatic_forward": false,
            "has_protected_content": false
        }"#,
            )
            .unwrap(),
        );
        assert_eq!(msg.text, "2");
    }

    #[test]
    fn message_text_from_caption_test() {
        let msg = Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"{
            "message_id": 3,
            "message_thread_id": null,
            "date": 1689731481,
            "chat": {
                "id": -1001,
                "title": "test",
                "type": "supergroup",
                "is_forum": false
            },
            "via_bot": null,
            "from": {
                "id": 1,
                "is_bot": false,
                "first_name": "Foo",
                "last_name": "Bar",
                "username": "Foo_Bar",
                "language_code": "zh-hans"
            },
            "photo": [
                {
                    "file_id": "0-1",
                    "file_unique_id": "2",
                    "file_size": 1224,
                    "width": 90,
                    "height": 81
                },
                {
                    "file_id": "0-1",
                    "file_unique_id": "2",
                    "file_size": 3493,
                    "width": 156,
                    "height": 141
                }
            ],
            "caption": "112",
            "caption_entities": [],
            "is_topic_message": false,
            "is_automatic_forward": false,
            "has_protected_content": false
        }"#,
            )
            .unwrap(),
        );
        assert_eq!(msg.text, "112")
    }

    #[test]
    fn message_link_test() {
        let msg = Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"{
            "message_id": 3,
            "message_thread_id": null,
            "date": 1689731481,
            "chat": {
                "id": -1001952114514,
                "title": "test",
                "type": "supergroup",
                "is_forum": false
            },
            "via_bot": null,
            "from": {
                "id": 1,
                "is_bot": false,
                "first_name": "Foo",
                "last_name": "Bar",
                "username": "Foo_Bar",
                "language_code": "zh-hans"
            },
            "photo": [
                {
                    "file_id": "0-1",
                    "file_unique_id": "2",
                    "file_size": 1224,
                    "width": 90,
                    "height": 81
                },
                {
                    "file_id": "0-1",
                    "file_unique_id": "2",
                    "file_size": 3493,
                    "width": 156,
                    "height": 141
                }
            ],
            "caption": "112",
            "caption_entities": [],
            "is_topic_message": false,
            "is_automatic_forward": false,
            "has_protected_content": false
        }"#,
            )
            .unwrap(),
        );
        assert_eq!(msg.link(), "https://t.me/c/1952114514/3")
    }

    #[test]
    fn message_no_via_bot_test() {
        let msg = Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"{
            "message_id": 2,
            "message_thread_id": null,
            "date": 1689731458,
            "chat": {
                "id": -1001,
                "title": "test",
                "type": "supergroup",
                "is_forum": false
            },
            "via_bot": null,
            "from": {
                "id": 1,
                "is_bot": false,
                "first_name": "Foo",
                "last_name": "Bar",
                "username": "Foo_Bar",
                "language_code": "zh-hans"
            },
            "text": "2",
            "entities": [],
            "is_topic_message": false,
            "is_automatic_forward": false,
            "has_protected_content": false
            }"#,
            )
            .unwrap(),
        );
        assert!(msg.via_bot.is_none());
    }

    #[test]
    fn message_via_bot_test() {
        let msg = Message::from(
            &serde_json::from_str::<teloxide::types::Message>(
                r#"{
            "message_id": 2,
            "message_thread_id": null,
            "date": 1689731458,
            "chat": {
                "id": -1001,
                "title": "test",
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
                "id": 1,
                "is_bot": false,
                "first_name": "Foo",
                "last_name": "Bar",
                "username": "Foo_Bar",
                "language_code": "zh-hans"
            },
            "text": "2",
            "entities": [],
            "is_topic_message": false,
            "is_automatic_forward": false,
            "has_protected_content": false
            }"#,
            )
            .unwrap(),
        );
        assert_eq!(msg.via_bot.unwrap(), "@TestBot");
    }
}
