build:
  cargo build --bin discord-bot

run:
  DISCORD_TOKEN=$(cat ./.api-key) \
  OVERRIDE_USER_ID=$(cat ./.user-id) \
  OVERRIDE_USER_NAME=$(cat ./.user-name) \
  GUILD_ID=$(cat ./.guild-id) \
  cargo run --bin discord-bot
