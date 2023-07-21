<div align="center">
<h1>telegram-CJK-search-bot</h1>

A simple message searching bot that supports CJK. 
</div>

### Features

- Relevance sorted search message.
- A edited message will be updated in database as well.
- A user can search messages in chats already joined only.

# Still WIP with NO Promise to work

### Quick Start

1. Create a bot by using [@BotFather](https://t.me/botfather) and take the token for later usage.
1. Edit the bot to turn inline mode ON and turn privacy mode OFF.
1. find a place to store all the data.
1. `wget https://raw.githubusercontent.com/krishukr/telegram-cjk-saerch-bot/master/docker-compose.yml`
1. edit `docker-compose.yml` to replace `TELOXIDE_TOKEN=xxx:xxx` with your token got from BotFather.
1. `docker compose up -d`
1. add the bot to chat you create or operate. **Notice: Supergroup Only.** 
1. send command `/start@your_bot`.

And you are done! All the **future** messages in this chat can be searched by simply inline using your bot like `@your_bot filter` .

Want to index history messages as well? just keep on.

1. export the messages in JSON format.
1. place the `result.json` in `./history` .
1. `docker compose run --rm bot /app/import`
1. wait patiently :)

Since there is no documents for exported messages, anything can happen during this process.

Feel free to reach me if you have any question.


### Credits

Powered by blazing fast [meilisearch](https://www.meilisearch.com/).

Powered by easy-to-use [teloxide](https://github.com/teloxide/teloxide).

Insipred by [telegram-search-bot](https://github.com/Taosky/telegram-search-bot).
