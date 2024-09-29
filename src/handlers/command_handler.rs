use super::inline_handler;
use crate::db::*;
use clap::CommandFactory;
use teloxide::{prelude::*, types::ReplyParameters, utils::command::BotCommands};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported in supergroups:"
)]
pub enum Command {
    #[command(description = "Display this text.")]
    Help,
    #[command(
        description = "Start logging messages in this supergroup. You need to be an Admin or Owner to perform this action."
    )]
    Start,
    #[command(
        description = "Stop logging messages in this supergroup. You need to be an Admin or Owner to perform this action."
    )]
    Stop,
}

enum ChatAction {
    Start,
    Stop,
}

impl ChatAction {
    async fn perform(&self, chat_id: ChatId) -> ResponseResult<()> {
        match self {
            ChatAction::Start => Ok(Db::new().insert_chat_with_id(chat_id).await),
            ChatAction::Stop => Ok(Db::new().delete_chat_with_id(chat_id).await),
        }
    }

    fn message(&self) -> &'static str {
        match self {
            ChatAction::Start => "started to log messages",
            ChatAction::Stop => "stopped to log messages",
        }
    }
}

pub async fn command_handler(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    match cmd {
        Command::Help => help_handler(bot, msg).await,
        Command::Start => chat_action_handler(bot, msg, ChatAction::Start).await,
        Command::Stop => chat_action_handler(bot, msg, ChatAction::Stop).await,
    }
}

pub async fn help_handler(bot: Bot, msg: Message) -> ResponseResult<()> {
    log::debug!("got command help");
    bot.send_message(
        msg.chat.id,
        format!("
{}\n
To start a query, type <code>{}</code> in the text input field in any chat. Typing to \"Saved Messages\" is recommended because it won't interrupt others. \n
{}",
            html_escape::encode_text(&Command::descriptions().to_string()),
            crate::BOT_USERNAME.get().unwrap(),
            html_escape::encode_text(&inline_handler::Cli::command().render_help().to_string())
                .replace(crate::BOT_USERNAME.get().unwrap(), &format!("<code>{}</code>", crate::BOT_USERNAME.get().unwrap()))
        ),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .await
    .and(Ok(()))
}

async fn chat_action_handler(bot: Bot, msg: Message, action: ChatAction) -> ResponseResult<()> {
    if !is_privileged(&bot, &msg).await? {
        reply_to_message(
            &bot,
            &msg,
            "You need to be either Admin or Owner of this group to perform this action.",
        )
        .await
    } else if !msg.chat.is_supergroup() {
        reply_to_message(&bot, &msg, "
Commands can only be used in a supergroup.

Tips: You can change a group to supergroup by setting its type to Public, and you can set it back to Private if you want.
        ").await
    } else {
        action.perform(msg.chat.id).await?;
        inline_handler::clear_user_chats_cache().await;
        reply_to_message(
            &bot,
            &msg,
            format!(
                "Chat {}({}) has {}.",
                msg.chat.title().unwrap_or_default(),
                msg.chat.id,
                action.message()
            ),
        )
        .await
    }
}

async fn is_privileged(bot: &Bot, msg: &Message) -> ResponseResult<bool> {
    if let Some(u) = &msg.from {
        Ok(bot
            .get_chat_member(msg.chat.id, u.id)
            .await?
            .is_privileged())
    } else {
        Ok(false)
    }
}

async fn reply_to_message<T>(bot: &Bot, msg: &Message, text: T) -> ResponseResult<()>
where
    T: Into<String>,
{
    bot.send_message(msg.chat.id, text)
        .reply_parameters(ReplyParameters {
            message_id: msg.id,
            ..Default::default()
        })
        .await
        .and(Ok(()))
}
