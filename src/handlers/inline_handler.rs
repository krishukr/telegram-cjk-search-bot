use crate::{db::*, types};
use cached::{proc_macro::cached, Cached};
use clap::{CommandFactory, Parser};
use futures::{StreamExt, TryStreamExt};
use teloxide::{
    prelude::*,
    types::{
        InlineQueryResult, InlineQueryResultArticle, InputMessageContent, InputMessageContentText,
        ParseMode::Html,
    },
    ApiError, RequestError,
};

#[derive(Parser)]
#[command(name = crate::BOT_USERNAME.get().unwrap())]
#[command(disable_version_flag = true, disable_help_flag = true)]
#[command(author, version, long_about = None)]
pub struct Cli {
    /// Keywords to search
    #[arg(default_value = "", hide_default_value = true)]
    query: Vec<String>,

    /// Include messages via all bots in search results
    #[arg(short = 'a', long)]
    include_all_bots: bool,

    /// Include messages via specific bots in search results
    #[arg(short, long)]
    include_bots: Option<Vec<String>>,

    /// Only search for messages via bots
    #[arg(short = 'l', long)]
    only_all_bots: bool,

    /// Only search for messages via specific bots
    #[arg(short, long)]
    only_bots: Option<Vec<String>>,

    /// Do not include web pages in search results
    #[arg(short = 'm', long, conflicts_with = "only_urls")]
    no_urls: bool,

    /// Only search for web pages
    #[arg(short = 'w', long, conflicts_with = "no_urls")]
    only_urls: bool,
}

pub async fn inline_handler(bot: Bot, q: InlineQuery) -> ResponseResult<()> {
    log::debug!("{}", serde_json::to_string_pretty(&q).unwrap());
    match Cli::try_parse_from([vec![""], (q.query.split_whitespace().collect::<Vec<_>>())].concat())
    {
        Ok(cli) => parsed_handler(bot, q, cli).await,
        Err(e) => parse_error_handler(bot, q, e).await,
    }
}

async fn parsed_handler(bot: Bot, q: InlineQuery, cli: Cli) -> ResponseResult<()> {
    let search_filter = construct_filter(bot.clone(), &q, &cli).await?;
    let current_offset: Option<usize> = q.offset.parse::<usize>().ok();

    let search_results = Db::new()
        .search_message_with_filter(&cli.query.join(" "), &search_filter, current_offset)
        .await;
    let mut results = futures::stream::iter(search_results.hits.into_iter().map(|m| {
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
    .await?;

    let next_offset = match results.len() < INLINE_REPLY_LIMIT {
        true => String::new(),
        false => (current_offset.unwrap_or_default() + INLINE_REPLY_LIMIT).to_string(),
    };
    if results.len() < INLINE_REPLY_LIMIT {
        let title = match current_offset.unwrap_or_default() + results.len() {
            0 => "No match.",
            _ => "No more.",
        };
        results.push(InlineQueryResult::Article(InlineQueryResultArticle::new(
            "empty",
            title,
            InputMessageContent::Text(InputMessageContentText::new(title)),
        )));
    }

    bot.answer_inline_query(&q.id, results)
        .next_offset(next_offset)
        .cache_time(0)
        .send()
        .await
        .and(Ok(()))
}

async fn parse_error_handler(bot: Bot, q: InlineQuery, e: clap::Error) -> ResponseResult<()> {
    let bot_username = crate::BOT_USERNAME.get().unwrap();

    bot.answer_inline_query(
        &q.id,
        [InlineQueryResult::Article(
            InlineQueryResultArticle::new(
                "1",
                "Parse Error!",
                InputMessageContent::Text(
                    InputMessageContentText::new(format!(
                        "{}\nOptions:\n{}",
                        html_escape::encode_text(&e.render().to_string())
                            .replace(bot_username, &format!("<code>{}</code>", bot_username)),
                        html_escape::encode_text(
                            &Cli::command()
                                .help_template("{options}")
                                .render_help()
                                .to_string()
                        )
                    ))
                    .parse_mode(Html),
                ),
            )
            .description(e.render().to_string().lines().next().unwrap_or_default()),
        )],
    )
    .next_offset("")
    .cache_time(10)
    .send()
    .await
    .and(Ok(()))
}

async fn is_chat_member_present(
    bot: Bot,
    chat_id: ChatId,
    user_id: UserId,
) -> ResponseResult<bool> {
    match bot.get_chat_member(chat_id, user_id).await {
        Ok(m) => Ok(m.is_present()),
        Err(RequestError::Api(_)) => Ok(false),
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
    futures::stream::iter(Db::new().get_all_chats().await)
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
        .await
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

async fn construct_filter<'a>(
    bot: Bot,
    q: &'a InlineQuery,
    cli: &'a Cli,
) -> ResponseResult<Filter<'a>> {
    Ok(Filter {
        chats: get_user_chats(bot, q.from.id).await?,
        include_bots: if cli.include_all_bots || cli.only_all_bots || cli.only_bots.is_some() {
            FilterOption::All
        } else {
            cli.include_bots
                .as_ref()
                .map(FilterOption::Some)
                .unwrap_or(FilterOption::None)
        },
        only_bots: if cli.only_all_bots {
            FilterOption::All
        } else {
            cli.only_bots
                .as_ref()
                .map(FilterOption::Some)
                .unwrap_or(FilterOption::None)
        },
        urls: if cli.only_urls {
            EnableOption::All
        } else if cli.no_urls {
            EnableOption::Disable
        } else {
            EnableOption::Enable
        },
    })
}

async fn construct_query_result(
    bot: Bot,
    m: types::Message,
    formatted_result: String,
) -> ResponseResult<InlineQueryResult> {
    let mut article = InlineQueryResultArticle::new(
        &m.key,
        formatted_result,
        InputMessageContent::Text(
            InputMessageContentText::new(format!(
                r#"「 {} 」 from <a href="{}">{}</a>{}"#,
                html_escape::encode_text(&m.text),
                m.link(),
                html_escape::encode_text(&generate_from_str(bot.clone(), &m).await?),
                generate_in_url_html(&m)
            ))
            .parse_mode(Html),
        ),
    )
    .description(format!(
        "{}@{}{}",
        generate_from_str(bot.clone(), &m).await?,
        m.format_time(),
        generate_in_url_desc(&m)
    ));
    if let Some(u) = m.thumbnail_url {
        article = article.thumbnail_url(u);
    }
    Ok(InlineQueryResult::Article(article))
}

fn generate_in_url_html(msg: &types::Message) -> String {
    if let Some(u) = &msg.web_page {
        format!(r#" in <a href="{}">{}</a>"#, u.as_str(), u.as_str())
    } else {
        String::default()
    }
}

fn generate_in_url_desc(msg: &types::Message) -> String {
    if let Some(u) = &msg.web_page {
        format!(" in {}", u.as_str())
    } else {
        String::default()
    }
}

pub(super) async fn clear_user_chats_cache() {
    GET_USER_CHATS.lock().await.cache_clear();
}

#[cfg(feature = "private_tests")]
#[cfg(test)]
#[path = "../private_tests/inline_handler_test.rs"]
mod inline_handler_test;
