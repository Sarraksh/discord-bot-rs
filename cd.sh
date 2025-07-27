#!/usr/bin/env bash

set -e

TARGET_DIR="/opt/discord-bot/incoming"
ARCHIVE_NAME="discord-bot-package.tar"

# Create a tar archive containing the whole .config directory, the discord-bot binary, and run.sh depending on the argument passed
case $1 in
  cfg)
    tar -czf $ARCHIVE_NAME .config run.sh docker-compose.yml
    ;;
  bin)
    tar -czf $ARCHIVE_NAME --transform 's,^,bin/,' -C target/debug discord-bot telegram-bot kc-ingester
    ;;
  full)
    tar -czf $ARCHIVE_NAME .config run.sh docker-compose.yml --transform 's,^target/debug/,bin/,' target/debug/discord-bot target/debug/telegram-bot target/debug/kc-ingester
    ;;
  *)
    echo "Usage: $0 {cfg|bin|full}"
    ;;
esac

set -u

ssh $TARGET_HOST "rm -rf $TARGET_DIR/*"
scp $ARCHIVE_NAME $TARGET_HOST:$TARGET_DIR/
ssh $TARGET_HOST "cd $TARGET_DIR && tar -xzf $ARCHIVE_NAME && rm -f $ARCHIVE_NAME"
rm -rf $ARCHIVE_NAME
