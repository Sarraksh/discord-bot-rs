set dotenv-load := true

build:
  cargo build

build-release:
  cargo build --release

run:
  ./run.sh cargo run --bin discord-bot

cd-full:
  cargo build
  ./cd.sh full

cd-bin:
  cargo build
  ./cd.sh bin

cd-cfg:
  ./cd.sh cfg
