use telegram_cjk_search_bot::*;

use db::*;
use handlers::*;
use teloxide::{prelude::*, utils::command::BotCommands};

const DESCRIPTION: &str =
    "Search CJK(Chinese, Japanese, and Korean) messages in groups using inline mode.";
const SHORT_DESCRIPTION: &str = "
This bot is open-sourced at https://github.com/krishukr/telegram-cjk-search-bot";

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let bot = Bot::from_env();
    Db::new().init().await;

    bot.set_my_commands(Command::bot_commands())
        .await
        .log_on_error()
        .await;

    if std::env::var_os("DESCRIPTION_CUSTOMIZED").is_none() {
        bot.set_my_description()
            .description(DESCRIPTION)
            .await
            .log_on_error()
            .await;
        bot.set_my_short_description()
            .short_description(SHORT_DESCRIPTION)
            .await
            .log_on_error()
            .await;
    }

    crate::BOT_USERNAME
        .set(format!("@{}", bot.get_me().await.unwrap().username()))
        .unwrap();

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_edited_message().endpoint(message_handler))
        .branch(Update::filter_inline_query().endpoint(inline_handler));

    log::info!("Started");

    Dispatcher::builder(bot, handler)
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
