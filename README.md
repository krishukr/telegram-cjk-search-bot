<div align="center">
<h1>Telegram-CJK-Search-Bot</h1>

A simple message-searching bot that supports CJK (Chinese, Japanese, Korean) languages.
</div>

### Features

- Search messages sorted by relevance.
- Edited messages will be updated in the database as well.
- Users can only search messages in chats they have already joined.

### Quick Start

1. Create a bot using [@BotFather](https://t.me/botfather) and save the token for later use.
1. Edit the bot to turn ON inline mode and turn OFF privacy mode.
1. Find a place to store all the data.
1. `wget https://raw.githubusercontent.com/krishukr/telegram-cjk-search-bot/master/docker-compose.yml`
1. Edit the `docker-compose.yml` file to replace `TELOXIDE_TOKEN=xxx:xxx` with your token obtained from BotFather.
1. `docker compose up -d`
1. Add the bot to a chat of which you are the owner or admin. **Note: Supergroup only.**
1. Send the command `/start@your_bot` to the bot.

And you're all set! All **future** messages in this chat can be searched by sending your bot inline queries, like so: `@your_bot filter`.

Want to index historical messages as well? Just follow these steps:

1. Export the chat messages in JSON format.
1. Place the `result.json` file in the `./history` directory.
1. `docker compose run --rm bot /app/import`

Since there are no documents for exported messages, unexpected issues may arise during this process.

Feel free to reach me if you have any questions.

### Frequently Asked Questions

#### What is a "supergroup"?

A supergroup is a type of Telegram group that supports larger member capacities and offers advanced features. Each message in a supergroup has a unique URL, which is necessary for searching and locating messages.

#### How can I create one?

To create a supergroup, start by creating a public group in Telegram. Once the group is public, it converts into a supergroup automatically.

#### What about my existing groups?

If you have existing private groups, just set the group to public, **save** the setting, and switch back to private. This gives you a private supergroup.

#### Do you have a ready-to-use instance?

Officially, there is no public instance provided.

#### How to update?

This projects follows [Semantic Versioning](https://semver.org). Docker image tags include `MAJOR.MINOR.PATCH` and `MAJOR.MINOR` .

```sh
docker compose pull
docker compose up -d
```

### Development

This project utilzes Nix Flake to manage development and building environment. You can run `nix develop` to open an interactive shell or run `direnv allow` to integrate the environment with your workflow.

Pull requests and issues would be supremely welcomed.

### Credits

Powered by the blazing-fast [Meilisearch](https://www.meilisearch.com/).

Powered by the easy-to-use [teloxide](https://github.com/teloxide/teloxide).

Inspired by [telegram-search-bot](https://github.com/Taosky/telegram-search-bot).
