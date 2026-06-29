# phonix-recall

A pure-Linux WebSocket service that feeds [Recall.ai](https://recall.ai) real-time meeting audio into the [phonix](../phonix) wake-word detector. Recall handles joining the Teams/Zoom/Meet call; this service receives the decoded PCM, runs one detector per participant, and acts on detections — deployable as an ordinary container on GKE (no Windows, no media SDK).

```
Teams/Zoom/Meet ──► Recall bot ──(WSS: base64 s16le PCM)──► phonix-recall (GKE pod)
                                          decode → Detector per participant
                                          → WakeEvent{pre_roll} → StreamSink (→ your LLM)
```

## Run locally

```bash
./crates/phonix/models/fetch.sh   # one-time: fetch silero_vad.onnx
PHONIX_VAD_MODEL=crates/phonix/models/silero_vad.onnx \
  cargo run -p phonix-recall            # listens on :8080
# health: curl localhost:8080/healthz   → ok
# Recall connects to:  ws://<host>:8080/ws
```

## Configuration (env)

| Variable | Default | Purpose |
|---|---|---|
| `PORT` | `8080` | Listen port |
| `PHONIX_VAD_MODEL` | `models/silero_vad.onnx` | Path to the Silero VAD model |
| `PHONIX_WAKE_MODEL` | _(unset → bundled `hey_jarvis`)_ | Path to a custom wake model (e.g. `moreni.onnx`) |
| `PHONIX_SAMPLE_RATE` | `16000` | Sample rate of Recall's PCM (must match your `recording_config`) |
| `PHONIX_WAKE_THRESHOLD` | `0.5` | Wake score threshold |
| `RUST_LOG` | `info` | Log filter |

## Recall.ai setup

When you create the bot, point a real-time **websocket** endpoint at this service and subscribe to a **raw audio** event:

- `audio_separate_raw.data` → per-participant streams (recommended; you learn *who* triggered).
- `audio_mixed_raw.data` → a single mixed stream (one detector, keyed `mixed`).

> **Verify two things in your Recall plan/config:** (1) you have **raw audio** streaming enabled (not just transcription — it's gated separately), and (2) the PCM format. This service assumes **s16le, mono, 16 kHz**; if yours differs, set `PHONIX_SAMPLE_RATE` and, if the JSON shape differs, adjust the field paths in [`src/recall.rs`](src/recall.rs) (they're isolated there).

## Container & GKE

```bash
./crates/phonix/models/fetch.sh                                  # model into the build context
docker build -f crates/phonix-recall/Dockerfile -t phonix-recall:dev .
docker run -p 8080:8080 phonix-recall:dev
```

On GKE: run it as a `Deployment` + `Service`, and terminate **TLS at an Ingress/Gateway** so Recall connects over **`wss://`** (the app speaks plain `ws` on `:8080` behind it). Use `/healthz` for liveness/readiness probes. The image is `debian-slim` + a static-ish pure-Rust binary — no ONNX Runtime, no audio system libraries.

> One detector per participant each loads its own Silero VAD model, so memory scales with concurrent speakers; fine for a first deployment, shareable later if needed.

## Next step

`LogSink` (in [`src/main.rs`](src/main.rs)) just logs detections. To stream the question to an LLM, replace it with a `StreamSink` that opens a session in `on_wake` (sending `event.pre_roll` first), forwards `on_audio` frames, and closes the turn in `on_end`.
