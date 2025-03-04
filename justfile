build:
  cargo build --bin discord-bot

run:
  DISCORD_TOKEN=$(cat ./.api-key) cargo run --bin discord-bot
