#!/usr/bin/env bash
# Create a Recall.ai bot that joins a meeting and streams real-time audio to a
# phonix-recall WebSocket, then tail its status until it leaves.
#
# Usage:
#   scripts/recall/join.sh <meeting_url> <wss_url> [bot_name]
#
# Env:
#   RECALL_API_KEY      (required) your Recall API token
#   RECALL_REGION       Recall region subdomain (default: us-west-2)
#   RECALL_AUDIO_EVENT  raw-audio event to stream (default: audio_separate_raw.data;
#                       use audio_mixed_raw.data for a single mixed stream)
#
# NB: verify the recording_config shape against your current Recall API docs —
# the field names for enabling raw audio and subscribing a realtime websocket
# have changed across versions. The Recall dashboard's bot-create shows the exact
# current payload.
set -euo pipefail

[ $# -ge 2 ] || {
  echo "usage: join.sh <meeting_url> <wss_url> [bot_name]" >&2
  exit 2
}
: "${RECALL_API_KEY:?set RECALL_API_KEY}"
command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }

meeting=$1
wss=$2
name=${3:-Phonix Listener}
region=${RECALL_REGION:-us-west-2}
event=${RECALL_AUDIO_EVENT:-audio_separate_raw.data}
audio_key=${event%.data} # audio_separate_raw.data -> audio_separate_raw
base="https://${region}.recall.ai/api/v1"

body=$(jq -n \
  --arg meeting "$meeting" --arg name "$name" \
  --arg wss "$wss" --arg event "$event" --arg key "$audio_key" \
  '{
    meeting_url: $meeting,
    bot_name: $name,
    recording_config: {
      ($key): {},
      realtime_endpoints: [
        { type: "websocket", url: $wss, events: [$event] }
      ]
    }
  }')

resp=$(curl -sS -X POST "$base/bot" \
  -H "Authorization: Token $RECALL_API_KEY" \
  -H "Content-Type: application/json" \
  -d "$body")

id=$(echo "$resp" | jq -r '.id // empty')
if [ -z "$id" ]; then
  echo "bot create failed:" >&2
  echo "$resp" | jq . 2>/dev/null >&2 || echo "$resp" >&2
  exit 1
fi

echo "bot id: $id"
echo "  leave with: scripts/recall/leave.sh $id"
echo "tailing status (Ctrl-C to stop tailing; the bot keeps running):"
last=""
while true; do
  st=$(curl -sS -H "Authorization: Token $RECALL_API_KEY" "$base/bot/$id" \
    | jq -r '.status_changes[-1].code // "unknown"')
  [ "$st" != "$last" ] && { echo "  status: $st"; last=$st; }
  case "$st" in done | call_ended | fatal) break ;; esac
  sleep 3
done
