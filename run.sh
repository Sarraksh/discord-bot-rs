#!/usr/bin/env bash

DISCORD_TOKEN=$(cat ./.config/api-key) \
OVERRIDE_USER_ID=$(cat ./.config/user-id) \
OVERRIDE_USER_NAME=$(cat ./.config/user-name) \
GUILD_ID=$(cat ./.config/guild-id) \
SYSTEM_PROMPT=$(cat ./.config/system-prompt) \
MODEL_NAME=$(cat ./.config/model-name) \
OLLAMA_HOST=$(cat ./.config/ollama-host) \
OLLAMA_PORT=$(cat ./.config/ollama-port) \
ERROR_OLLAMA_ERROR=$(cat ./.config/error-ollama-error) \
ERROR_NO_MESSAGES=$(cat ./.config/error-no-messages) \
TRIGGER_WORDS=$(cat ./.config/trigger-words) \
TABLE_HEADER=$(cat ./.config/table-header) \
TELOXIDE_TOKEN=$(cat ./.config/teloxide-token) \
"$@"