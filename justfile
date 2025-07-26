set dotenv-load := true

build:
  cargo build --bin discord-bot

build-release:
  cargo build --bin discord-bot --release

run:
  ./run.sh cargo run --bin discord-bot

cd-full:
  cargo build --bin discord-bot
  ./cd.sh full

cd-bin:
  cargo build --bin discord-bot
  ./cd.sh bin

cd-cfg:
  ./cd.sh cfg
