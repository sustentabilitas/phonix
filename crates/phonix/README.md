# phonix

Pure-Rust wake-word detection with Silero VAD, ~500 ms pre-roll buffering, and a
`StreamSink` boundary for streaming the live utterance to an LLM. Zero C++: all
ONNX runs on `tract` (`tract-onnx` + `oww-rs`).

## Models

```bash
./models/fetch.sh   # downloads silero_vad.onnx; OWW models ship inside oww-rs
```

## Library

```rust
use phonix::{Config, Detector, StdoutSink};

let mut detector = Detector::new(Config::default(), StdoutSink)?;
detector.push(&pcm_frames)?; // f32 PCM at Config.input_sample_rate / channels
# Ok::<(), phonix::Error>(())
```

Implement `StreamSink` to forward `pre_roll` + live audio to Gemini Live (or any
LLM). `on_wake` delivers the pre-roll first, then `on_audio`, then `on_end`.

## Test binary

```bash
cargo run --features cli --bin phonix-listen -- mic
cargo run --features cli --bin phonix-listen -- file path/to/clip.wav
```

## Swapping in a custom "Moreni" model

Train per `docs/training/README.md`, then set `Config.models.wake =
Some("moreni.onnx".into())`. No code changes.
