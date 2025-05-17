build:
  cargo build --bin discord-bot

build-release:
  cargo build --bin discord-bot --release

run:
  ./run.sh cargo run --bin discord-bot

cd:
  ./.cd.sh

cd-full:
  cargo build --bin discord-bot
  ./.cd-full.sh
