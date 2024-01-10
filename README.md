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

### Credits

Powered by the blazing-fast [Meilisearch](https://www.meilisearch.com/).

Powered by the easy-to-use [teloxide](https://github.com/teloxide/teloxide).

Inspired by [telegram-search-bot](https://github.com/Taosky/telegram-search-bot).
