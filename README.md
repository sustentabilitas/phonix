# Phonix

Pure-Rust wake-word detection with voice-activity gating, pre-roll buffering, and a streaming boundary for live-LLM voice agents — **zero C++**.

[![phonix crates.io](https://img.shields.io/crates/v/phonix.svg)](https://crates.io/crates/phonix)
[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![CI](https://github.com/sustentabilitas/phonix/actions/workflows/ci.yml/badge.svg)](https://github.com/sustentabilitas/phonix/actions/workflows/ci.yml)

| Crate | Description |
|-------|-------------|
| [`phonix`](crates/phonix) | Wake-word detector: Silero VAD + OpenWakeWord on [`tract`](https://github.com/sonos/tract), 16 kHz resampling, ~500 ms pre-roll ring, two-state (listen/stream) machine, and a `StreamSink` boundary for forwarding the live utterance to an LLM. Ships a `phonix-listen` CLI for local testing. |

## Contents

- [Overview](#overview)
- [Why phonix](#why-phonix)
- [Quick start](#quick-start)
- [Library usage](#library-usage)
- [The `StreamSink` boundary](#the-streamsink-boundary)
- [Meeting-platform integration](#meeting-platform-integration)
- [Configuration](#configuration)
- [Training a custom wake word](#training-a-custom-wake-word)
- [Development](#development)
- [Contributing](#contributing)
- [Security](#security)
- [License](#license)

---

## Overview

Phonix listens to an audio stream, gates it with a neural voice-activity detector, detects a wake phrase using an OpenWakeWord neural model, and — on detection — emits the wake event together with a short **pre-roll** of the audio that preceded the trigger, then streams the live utterance to a downstream consumer (a live LLM such as Gemini Live).

Frame by frame, inside `Detector::push(&[f32])`:

```
adapter frames (any rate, any channels)        ← mic | WAV file | Teams | Zoom | Meet
   │
   ▼
resample → downmix to mono, 16 kHz f32 (rubato)
   │
   ├──────────────► pre-roll ring (always-on, ~500 ms circular)
   │
   ▼
[Silero VAD]  voice probability (512-sample chunks, tract)
   │
   ▼
[OpenWakeWord]  melspectrogram → embedding → classifier (1280-sample chunks, oww-rs)
   │
   ▼
score > wake_threshold AND armed (Listening, past cooldown)?
   │ yes
   ▼
  drain pre-roll ring → emit WakeEvent → sink.on_wake(event)
   │
   ▼
  STREAMING: forward live 16 kHz frames to sink.on_audio(...)
  until sustained silence (Silero) → sink.on_end() → back to Listening
```

The detector is **speaker-independent** (OpenWakeWord's speech-embedding stage ignores vocal pitch, so it works across genders, accents, and tones) and resistant to conversational false positives, because the wake decision is made by a neural classifier rather than audio fingerprinting.

## Why phonix

- **Zero C++, zero ONNX Runtime.** Every neural model — Silero VAD and the three-stage OpenWakeWord pipeline — runs on [`tract`](https://github.com/sonos/tract) (via [`oww-rs`](https://crates.io/crates/oww-rs) for OpenWakeWord), which compiles ONNX to pure-Rust execution. No `ort`, no native `onnxruntime`, no system libraries to link. You get deep-learning wake-word accuracy with a static, cross-compilable, ultra-light build.
- **Pre-roll built in.** Humans don't pause after the wake word ("Hey Moreni, what was the last action item?"). Phonix keeps an always-on ~500 ms circular buffer and prepends it to the first payload, so the LLM never misses the start of the question.
- **VAD-gated, with end-of-utterance detection.** Silero VAD runs continuously to know when speech stops, so the streamed turn closes cleanly.
- **Input-agnostic core.** The library is sync and I/O-free: it consumes `&[f32]` PCM and emits events. Mic, WAV file, and meeting-platform feeds are all thin adapters. `cpal`/`hound`/`clap` live only behind the `cli` feature.

## Quick start

### Prerequisites

- [Rust stable toolchain](https://rustup.rs/)
- The Silero VAD model (the OpenWakeWord models ship bundled inside `oww-rs`):

```bash
./crates/phonix/models/fetch.sh   # downloads silero_vad.onnx + verifies its checksum
```

### Local CLI

```bash
# Live microphone (say "Hey Jarvis" — the bundled bootstrap model)
cargo run --features cli --bin phonix-listen -- mic --debug

# Pick a specific input device (the default may be a quiet Bluetooth/HFP mic)
cargo run --features cli --bin phonix-listen -- devices
cargo run --features cli --bin phonix-listen -- --device "MacBook" mic --debug

# Reproducible file mode (regression testing / threshold tuning)
cargo run --features cli --bin phonix-listen -- file path/to/clip.wav
```

`--debug` prints live peak amplitude, VAD probability, and wake score — the fastest way to diagnose capture, device, permission, or threshold issues.

## Library usage

```rust
use phonix::{Config, Detector, StdoutSink};

let mut detector = Detector::new(Config::default(), StdoutSink)?;
detector.push(&pcm_frames)?; // f32 PCM at Config.input_sample_rate / channels
# Ok::<(), phonix::Error>(())
```

The core is sync and allocation-light. Implement [`StreamSink`](#the-streamsink-boundary) to forward audio to your LLM; `Detector` is `Send` and cheap to instantiate, so you can run one per audio stream.

## The `StreamSink` boundary

The wake event and its consumer are the seam where you wire in a live LLM (Gemini Live, a websocket, etc.). Phonix never imports an LLM SDK.

```rust
pub struct WakeEvent {
    pub model: String,            // "hey_jarvis" by default, your trained word later
    pub score: f32,               // classifier confidence at the trigger
    pub pre_roll: Vec<f32>,       // ~500 ms of 16 kHz mono audio captured BEFORE the trigger
    pub stream_id: Option<String>,// participant/stream id, for per-stream meeting use
}

pub trait StreamSink: Send {
    fn on_wake(&mut self, event: &WakeEvent); // fires first — send pre_roll, then live audio
    fn on_audio(&mut self, frames: &[f32]);   // live utterance, 16 kHz mono f32
    fn on_end(&mut self);                      // VAD detected end-of-utterance; close the turn
}
```

The call order — `on_wake` (carrying `pre_roll`) → `on_audio` → `on_end` — guarantees the pre-roll reaches the LLM before any live audio, with no gap. The crate ships `StdoutSink` (for the CLI) and `VecSink` (for tests).

## Meeting-platform integration

Microsoft Teams (Graph / ACS real-time-media), Zoom (Meeting SDK raw data / RTMS), and Google Meet (Meet Media API) all ultimately deliver raw PCM frames. Each is just another adapter that pushes PCM into `Detector::push(&[f32])`:

- **Input-agnostic** — the core never reaches for a device; see [`examples/meeting_adapter.rs`](crates/phonix/examples/meeting_adapter.rs).
- **Configurable rate + channel downmix** — 48 kHz stereo and 16 kHz mono normalize alike to the 16 kHz mono the models need.
- **Per-stream** — run one `Detector` per participant (set a `stream_id`) to attribute *who* triggered, or one over the mixed stream.

> The meeting platform's own media SDK and any Opus decoding live in *your* adapter — outside phonix's pure-Rust boundary — so projects that only need the mic/file path never pull those dependencies.

## Configuration

```rust
pub struct Config {
    pub input_sample_rate: u32,   // e.g. 48_000 (mic) / 16_000 (Teams)
    pub input_channels: u16,      // downmixed to mono
    pub vad_threshold: f32,       // Silero voice probability, default 0.5
    pub wake_threshold: f32,      // wake score, default 0.5 — tune vs false positives
    pub pre_roll_ms: u32,         // default 500
    pub cooldown_ms: u32,         // refractory after a trigger, default 1500
    pub end_silence_ms: u32,      // sustained silence → on_end, default 700
    pub models: ModelPaths,       // silero_vad path; optional custom wake model
}
```

`wake_threshold`, `cooldown_ms`, and `end_silence_ms` are the knobs you'll tune for your environment. All fields have sane defaults.

## Training a custom wake word

The bundled `hey_jarvis` model is a bootstrap so the crate is usable today. To detect your own phrase (e.g. "Moreni"), train an OpenWakeWord classifier on a synthetic-TTS dataset and drop in the `.onnx`:

```rust
let mut cfg = Config::default();
cfg.models.wake = Some("models/moreni.onnx".into()); // one line; no other code changes
```

See [`docs/training/README.md`](docs/training/README.md) for the full pipeline (synthetic-TTS dataset → hard-negative confusables → train the classifier head → export ONNX → validate against the regression harness).

## Development

```bash
git clone https://github.com/sustentabilitas/phonix.git
cd phonix
./crates/phonix/models/fetch.sh        # model-gated tests need silero_vad.onnx

cargo build                             # lean library (no I/O deps)
cargo build --features cli              # + phonix-listen binary (cpal/hound/clap)
cargo test                              # unit + integration + regression
```

| Command | What it enables |
|---|---|
| `cargo build` | Default: the pure, sync, I/O-free detection library |
| `cargo build --features cli` | The `phonix-listen` binary (mic + file modes) |

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow, commit conventions, and DCO sign-off.

## Contributing

Contributions are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md) and our [Code of Conduct](CODE_OF_CONDUCT.md). Non-trivial work follows a **spec → plan → implement (TDD)** flow.

## Security

Please report vulnerabilities privately — see [SECURITY.md](SECURITY.md). Do not open public issues for security reports.

## License

Licensed under the [GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0-only).
