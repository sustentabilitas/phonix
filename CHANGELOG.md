# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Pure-Rust wake-word `Detector`: Silero VAD + OpenWakeWord (via `tract`/`oww-rs`),
  any-rate/any-channel resampling to 16 kHz mono, ~500 ms pre-roll ring, and a
  two-state (Listening/Streaming) machine with sample-based cooldown.
- `StreamSink` trait (the live-LLM boundary) with `WakeEvent` carrying the pre-roll,
  plus `StdoutSink` and `VecSink`.
- `phonix-listen` CLI: `mic` (cpal) and `file` (hound) modes, `devices`/`--device`
  input selection, and a `--debug` meter (peak amplitude, VAD probability, wake score).
- Meeting-platform adapter example and a fixture-driven regression harness
  (recall + false-positive).
- `docs/training/` guide for training a custom wake-word classifier.

[Unreleased]: https://github.com/sustentabilitas/phonix/commits/main
