use telegram_cjk_search_bot::*;

use db::*;
use handlers::*;
use teloxide::{prelude::*, utils::command::BotCommands};

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let bot = Bot::from_env();
    Db::new().init().await;

    bot.set_my_commands(Command::bot_commands())
        .await
        .log_on_error()
        .await;

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
