use super::inline_handler;
use crate::db::*;
use clap::CommandFactory;
use teloxide::{prelude::*, types::ReplyParameters, utils::command::BotCommands};

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
        Command::Help => help_handler(bot, msg).await,
        Command::Start => start_handler(bot, msg).await,
        Command::Stop => stop_handler(bot, msg).await,
    }
}

pub async fn help_handler(bot: Bot, msg: Message) -> ResponseResult<()> {
    log::debug!("got command help");
    bot.send_message(
        msg.chat.id,
        format!(
            "{}\n\nInline {}",
            Command::descriptions().to_string(),
            inline_handler::Cli::command().render_help()
        ),
    )
    .await
    .and(Ok(()))
}

async fn start_handler(bot: Bot, msg: Message) -> ResponseResult<()> {
    log::debug!("got command start");
    if is_privileged(&bot, &msg).await? {
        Db::new().insert_chat_with_id(msg.chat.id).await;
        inline_handler::clear_user_chats_cache().await;
        bot.send_message(
            msg.chat.id,
            format!(
                "Chat {}({}) has started to log messages.",
                msg.chat.title().unwrap_or(""),
                msg.chat.id
            ),
        )
        .reply_parameters(ReplyParameters {
            message_id: msg.id,
            ..Default::default()
        })
        .await
        .and(Ok(()))
    } else {
        Ok(())
    }
}

async fn stop_handler(bot: Bot, msg: Message) -> ResponseResult<()> {
    log::debug!("got command stop");
    if is_privileged(&bot, &msg).await? {
        Db::new().delete_chat_with_id(msg.chat.id).await;
        inline_handler::clear_user_chats_cache().await;
        bot.send_message(
            msg.chat.id,
            format!(
                "Chat {}({}) has stopped to log messages.",
                msg.chat.title().unwrap_or(""),
                msg.chat.id
            ),
        )
        .reply_parameters(ReplyParameters {
            message_id: msg.id,
            ..Default::default()
        })
        .await
        .and(Ok(()))
    } else {
        Ok(())
    }
}

async fn is_privileged(bot: &Bot, msg: &Message) -> ResponseResult<bool> {
    if let Some(u) = &msg.from {
        if bot
            .get_chat_member(msg.chat.id, u.id)
            .await?
            .is_privileged()
        {
            log::debug!("{} is privileged", u.id);
            return Ok(true);
        }
        bot.send_message(msg.chat.id, "You are not privileged to do this.")
            .reply_parameters(ReplyParameters {
                message_id: msg.id,
                ..Default::default()
            })
            .await?;
    }
    Ok(false)
}
