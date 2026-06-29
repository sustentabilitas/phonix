# Recall.ai bot helpers

Small scripts to drive a Recall.ai bot that joins a meeting and streams real-time
audio into a running [`phonix-recall`](../../crates/phonix-recall) service.

Requires `curl` and `jq`. Set your token (and region, if not `us-west-2`):

```bash
export RECALL_API_KEY=...           # required
export RECALL_REGION=us-west-2      # your Recall region subdomain
```

| Script | Purpose |
|---|---|
| `join.sh <meeting_url> <wss_url> [bot_name]` | Create a bot, point its realtime audio at your `wss://…/ws`, print the id, tail status |
| `status.sh <bot_id>` | Show the bot's status history (diagnose lobby/join issues) |
| `leave.sh <bot_id>` | Make the bot leave the call |

## Full local loop

```bash
# 1. run phonix-recall
./crates/phonix/models/fetch.sh
PHONIX_VAD_MODEL=crates/phonix/models/silero_vad.onnx RUST_LOG=info \
  cargo run -p phonix-recall            # :8080

# 2. expose it over wss (Recall connects to you; TLS required)
ngrok http 8080                          # → https://abc123.ngrok.app

# 3. send a bot into your Teams meeting
scripts/recall/join.sh \
  "https://teams.microsoft.com/l/meetup-join/..." \
  "wss://abc123.ngrok.app/ws"

# 4. admit the bot from the Teams lobby, say "Hey Jarvis", watch phonix-recall logs
# 5. scripts/recall/leave.sh <bot_id>
```

`RECALL_AUDIO_EVENT=audio_mixed_raw.data scripts/recall/join.sh …` streams one mixed
stream instead of per-participant.

> **Verify against current Recall docs:** the `recording_config` shape (how raw audio
> is enabled + how a realtime websocket is subscribed) and the `leave_call` endpoint
> have changed across Recall API versions. Creating a bot once from the Recall
> dashboard shows the exact current payload — copy that into `join.sh` if it differs.
