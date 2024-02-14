use crate::db::*;
use cached::proc_macro::cached;
use futures::{StreamExt, TryStreamExt};
use teloxide::{
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputMessageContentText,
        Me,
    },
    utils::command::BotCommands,
    ApiError, RequestError,
};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
pub enum Command {
    #[command(description = "Display this text.")]
    Help,
    #[command(
        description = "Start to log messages in this chat. Privilege is needed for this operation."
    )]
    Start,
    #[command(
        description = "Stop to log messages in this chat. Privilege is needed for this operation."
    )]
    Stop,
}

pub async fn command_handler(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => {
            log::debug!("got command help");
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?;
        }
        Command::Start => {
            log::debug!("got command start");
            if let Some(u) = msg.from() {
                if bot
                    .get_chat_member(msg.chat.id, u.id)
                    .await?
                    .is_privileged()
                {
                    log::debug!("{} is privileged", u.id);
                    Db::new().insert_chat_with_id(msg.chat.id).await;
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "Chat {}({}) has started to log messages.",
                            msg.chat.title().unwrap_or(""),
                            msg.chat.id
                        ),
                    )
                    .reply_to_message_id(msg.id)
                    .await?;
                } else {
                    bot.send_message(msg.chat.id, "You are not privilieged to do this.")
                        .reply_to_message_id(msg.id)
                        .await?;
                }
            }
        }
        Command::Stop => {
            log::debug!("got command stop");
            if let Some(u) = msg.from() {
                if bot
                    .get_chat_member(msg.chat.id, u.id)
                    .await?
                    .is_privileged()
                {
                    log::debug!("{} is privileged", u.id);
                    Db::new().delete_chat_with_id(msg.chat.id).await;
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "Chat {}({}) has stopped to log messages.",
                            msg.chat.title().unwrap_or(""),
                            msg.chat.id
                        ),
                    )
                    .reply_to_message_id(msg.id)
                    .await?;
                } else {
                    bot.send_message(msg.chat.id, "You are not privilieged to do this.")
                        .reply_to_message_id(msg.id)
                        .await?;
                }
            }
        }
    };

    Ok(())
}

pub async fn message_handler(bot: Bot, msg: Message, me: Me) -> ResponseResult<()> {
    log::debug!("{}", serde_json::to_string_pretty(&msg).unwrap());

    if !msg.chat.is_supergroup()
        || msg.via_bot.as_ref().map_or(false, |b| b.id == me.id)
        || msg.from().map_or(false, |u| u.is_bot)
    {
        Ok(())
    } else if let Some(text) = msg.text() {
        match Command::parse(text, me.username()) {
            Ok(cmd) => command_handler(bot, msg, cmd).await,
            Err(_) => normal_message_handler(msg).await,
        }
    } else if msg.caption().is_some() {
        normal_message_handler(msg).await
    } else {
        Ok(())
    }
}

pub async fn inline_handler(bot: Bot, q: InlineQuery) -> ResponseResult<()> {
    log::debug!("{}", serde_json::to_string_pretty(&q).unwrap());

    let search_results = Db::new()
        .search_message_with_filter_chats(&q.query, &get_user_chats(bot.clone(), q.from.id).await?)
        .await;
    bot.answer_inline_query(
        &q.id,
        search_results
            .hits
            .iter()
            .map(|m| (&m.result, m.formatted_result.as_ref().unwrap()))
            .map(|(m, f)| {
                InlineQueryResult::Article(
                    InlineQueryResultArticle::new(
                        &m.key,
                        f["text"].as_str().unwrap(),
                        InputMessageContent::Text(
                            InputMessageContentText::new(format!(
                                r#"「 {} 」 from <a href="{}">{}</a>"#,
                                html_escape::encode_text(&m.text),
                                m.link(),
                                html_escape::encode_text(&m.from),
                            ))
                            .parse_mode(teloxide::types::ParseMode::Html),
                        ),
                    )
                    .description(format!("{}@{}", &m.from, m.format_time())),
                )
            }),
    )
    .send()
    .await?;

    Ok(())
}

pub async fn normal_message_handler(msg: Message) -> ResponseResult<()> {
    if Db::new().filter_chat_with_id(msg.chat.id).await.is_none() {
        log::debug!("{} not a enabled chat", &msg.chat.id);
        return Ok(());
    }

    Db::new()
        .insert_messages(&vec![crate::types::Message::from(&msg)])
        .await;

    Ok(())
}

async fn is_chat_member_present(
    bot: Bot,
    chat_id: ChatId,
    user_id: UserId,
) -> ResponseResult<bool> {
    match bot.get_chat_member(chat_id, user_id).await {
        Ok(m) => Ok(m.is_present()),
        Err(RequestError::Api(ApiError::UserNotFound)) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cached(
    time = 120,
    result = true,
    sync_writes = true,
    key = "UserId",
    convert = r#"{ user_id }"#
)]
async fn get_user_chats(bot: Bot, user_id: UserId) -> ResponseResult<Vec<crate::types::Chat>> {
    log::debug!("uncached get_user_chats {}", user_id);
    Ok(futures::stream::iter(Db::new().get_all_chats().await)
        .filter_map(|chat| {
            let bot = bot.clone();
            async move {
                match is_chat_member_present(bot, chat, user_id).await {
                    Ok(true) => Some(Ok(crate::types::Chat::from(chat))),
                    Ok(false) => None,
                    Err(e) => Some(Err(e)),
                }
            }
        })
        .try_collect()
        .await?)
}

#[cfg(feature = "private_tests")]
#[cfg(test)]
#[path = "./private_tests/handlers_test.rs"]
mod handlers_test;
