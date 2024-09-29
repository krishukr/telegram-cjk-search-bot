use super::command_handler;
use crate::{db::*, handlers::command_handler::help_handler, types};
use teloxide::{prelude::*, types::Me, utils::command::BotCommands};

pub async fn message_handler(bot: Bot, msg: Message, me: Me) -> ResponseResult<()> {
    log::debug!("{}", serde_json::to_string_pretty(&msg).unwrap());


    if msg.thread_id.is_some()
        || !msg.chat.is_supergroup()
        || msg.via_bot.as_ref().map_or(false, |b| b.id == me.id)
    {
        Ok(())
    } else if msg.chat.is_private() {
        if msg.edit_date().is_none() {
            help_handler(bot, msg).await
        } else {
            Ok(())
        }
    } else if let Some(text) = msg.text() {
        match command_handler::Command::parse(text, me.username()) {
            Ok(cmd) => command_handler(bot, msg, cmd).await,
            Err(_) => normal_message_handler(msg).await,
        }
    } else if msg.caption().is_some() {
        normal_message_handler(msg).await
    } else {
        Ok(())
    }
}

async fn normal_message_handler(msg: Message) -> ResponseResult<()> {
    if !msg.chat.is_supergroup() || Db::new().filter_chat_with_id(msg.chat.id).await.is_none() {
        log::debug!("{} not a enabled chat", &msg.chat.id);
        return Ok(());
    }

    Db::new().insert(&types::Sender::from(&msg)).await;
    Db::new().insert(&vec![types::Message::from(&msg)]).await;

    Ok(())
}
