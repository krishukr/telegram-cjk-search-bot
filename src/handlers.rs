use crate::{db::*, types};
use cached::{proc_macro::cached, Cached};
use clap::{CommandFactory, Parser};
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
            bot.send_message(
                msg.chat.id,
                format!(
                    "{}\n\nInline {}",
                    Command::descriptions().to_string(),
                    Cli::command().render_help()
                ),
            )
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
                    GET_USER_CHATS.lock().await.cache_clear();
                } else {
                    bot.send_message(msg.chat.id, "You are not privileged to do this.")
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
                    GET_USER_CHATS.lock().await.cache_clear();
                } else {
                    bot.send_message(msg.chat.id, "You are not privileged to do this.")
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

    if !msg.chat.is_supergroup() || msg.via_bot.as_ref().map_or(false, |b| b.id == me.id) {
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

#[derive(Parser)]
#[command(name = crate::BOT_USERNAME.get().unwrap())]
#[command(disable_version_flag = true, disable_help_flag = true)]
#[command(author, version, long_about = None)]
// #[command(about = "Search messages.")]
pub struct Cli {
    #[arg(default_value = "", hide_default_value = true)]
    query: String,

    #[arg(short = 'a', long)]
    exclude_all_bots: bool,

    #[arg(short, long)]
    exclude_bots: Option<Vec<String>>,
}

pub async fn inline_handler(bot: Bot, q: InlineQuery) -> ResponseResult<()> {
    log::debug!("{}", serde_json::to_string_pretty(&q).unwrap());
    let cli =
        Cli::try_parse_from([vec![""], (q.query.split_whitespace().collect::<Vec<_>>())].concat());
    match cli {
        Ok(cli) => {
            let search_filter = Filter {
                chats: get_user_chats(bot.clone(), q.from.id).await?,
                exclude_bots: if cli.exclude_all_bots {
                    ExcludeOption::All
                } else {
                    cli.exclude_bots
                        .map(|x| ExcludeOption::Some(x))
                        .unwrap_or(ExcludeOption::None)
                },
            };

            let search_results = Db::new()
                .search_message_with_filter(&cli.query, &search_filter)
                .await;
            bot.answer_inline_query(
                &q.id,
                futures::stream::iter(search_results.hits.into_iter().map(|m| {
                    (
                        m.result,
                        m.formatted_result.unwrap()["text"]
                            .as_str()
                            .unwrap()
                            .to_string(),
                    )
                }))
                .then(|(m, f)| construct_query_result(bot.clone(), m, f))
                .try_collect::<Vec<_>>()
                .await?,
            )
            .cache_time(0)
            .send()
            .await?;

            Ok(())
        }
        Err(e) => {
            bot.answer_inline_query(
                &q.id,
                [InlineQueryResult::Article(
                    InlineQueryResultArticle::new(
                        "1",
                        "Parse Error!",
                        InputMessageContent::Text(InputMessageContentText::new(format!(
                            "{}",
                            e.render()
                        ))),
                    )
                    .description(format!("{}", e.render())),
                )],
            )
            .cache_time(0)
            .send()
            .await?;
            Ok(())
        }
    }
}

pub async fn normal_message_handler(msg: Message) -> ResponseResult<()> {
    Db::new().insert(&types::Sender::from(&msg)).await;
    if Db::new().filter_chat_with_id(msg.chat.id).await.is_none() {
        log::debug!("{} not a enabled chat", &msg.chat.id);
        return Ok(());
    }

    Db::new().insert(&vec![types::Message::from(&msg)]).await;

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
    time = 10,
    time_refresh = true,
    result = true,
    sync_writes = true,
    key = "UserId",
    convert = r#"{ user_id }"#
)]
async fn get_user_chats(bot: Bot, user_id: UserId) -> ResponseResult<Vec<types::Chat>> {
    log::debug!("uncached get_user_chats {}", user_id);
    Ok(futures::stream::iter(Db::new().get_all_chats().await)
        .filter_map(|chat| {
            let bot = bot.clone();
            async move {
                match is_chat_member_present(bot, chat, user_id).await {
                    Ok(true) => Some(Ok(types::Chat::from(chat))),
                    Ok(false) => None,
                    Err(e) => Some(Err(e)),
                }
            }
        })
        .try_collect()
        .await?)
}

#[cached(
    time = 10,
    time_refresh = true,
    result = true,
    sync_writes = true,
    key = "ChatId",
    convert = r#"{ chat_id }"#
)]
async fn get_name_from_tg(bot: Bot, chat_id: ChatId) -> ResponseResult<Option<String>> {
    bot.get_chat(chat_id).await.map_or_else(
        |e| {
            if let RequestError::Api(ApiError::ChatNotFound) = e {
                Ok(None)
            } else {
                Err(e)
            }
        },
        |c| {
            Ok(c.title()
                .map(ToString::to_string)
                .or(c.first_name().map(|first_name| match c.last_name() {
                    Some(last_name) => format!("{} {}", first_name, last_name),
                    None => first_name.to_string(),
                })))
        },
    )
}

#[cached(
    time = 1,
    result = true,
    sync_writes = true,
    key = "ChatId",
    convert = r#"{ chat_id }"#
)]
async fn get_name_from_chat_id(bot: Bot, chat_id: ChatId) -> ResponseResult<String> {
    if let Some(n) = Db::new().get_sender_name(chat_id).await {
        tokio::spawn(async move {
            if let Some(n) = get_name_from_tg(bot, chat_id).await.unwrap_or(None) {
                Db::new()
                    .insert(&vec![types::Sender {
                        id: chat_id,
                        name: n,
                    }])
                    .await;
            }
        });
        Ok(n)
    } else {
        match get_name_from_tg(bot, chat_id).await? {
            Some(n) => {
                Db::new()
                    .insert(&vec![types::Sender {
                        id: chat_id,
                        name: n.clone(),
                    }])
                    .await;
                Ok(n)
            }
            None => Ok("Anonymous".to_string()),
        }
    }
}

async fn generate_from_str(bot: Bot, m: &types::Message) -> ResponseResult<String> {
    if let Some(from) = &m.from {
        Ok(from.clone())
    } else {
        Ok(format!(
            "{}@{}",
            get_name_from_chat_id(bot.clone(), m.sender.unwrap()).await?,
            get_name_from_chat_id(bot.clone(), m.chat_id).await?
        ))
    }
}

async fn construct_query_result(
    bot: Bot,
    m: types::Message,
    formatted_result: String,
) -> ResponseResult<InlineQueryResult> {
    Ok(InlineQueryResult::Article(
        InlineQueryResultArticle::new(
            &m.key,
            formatted_result,
            InputMessageContent::Text(
                InputMessageContentText::new(format!(
                    r#"「 {} 」 from <a href="{}">{}</a>"#,
                    html_escape::encode_text(&m.text),
                    m.link(),
                    html_escape::encode_text(&generate_from_str(bot.clone(), &m).await?),
                ))
                .parse_mode(teloxide::types::ParseMode::Html),
            ),
        )
        .description(format!(
            "{}@{}",
            generate_from_str(bot.clone(), &m).await?,
            m.format_time()
        )),
    ))
}

#[cfg(feature = "private_tests")]
#[cfg(test)]
#[path = "./private_tests/handlers_test.rs"]
mod handlers_test;
