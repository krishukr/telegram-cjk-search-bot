use std::{collections::HashMap, sync::Arc};

use telegram_cjk_search_bot::*;

use db::*;
use handlers::*;
use teloxide::prelude::*;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let bot = Bot::from_env();
    Db::new().init().await;

    let user_in_groups_cache = Arc::new(Mutex::new(HashMap::<UserId, Vec<types::Chat>>::new()));

    let handler = dptree::entry()
        .branch(
            Update::filter_message()
                .filter_command::<Command>()
                .endpoint(command_handler),
        )
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_edited_message().endpoint(message_handler))
        .branch(Update::filter_inline_query().endpoint(inline_handler));

    log::info!("Started");

    Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![user_in_groups_cache])
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
