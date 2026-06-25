//! Phonix — pure-Rust wake-word detection with Silero VAD and pre-roll buffering.
//!
//! The core is sync and I/O-free: feed [`Detector::push`] PCM `f32` frames at any
//! sample rate; it emits [`WakeEvent`]s and streams audio to a [`StreamSink`].

mod error;
pub use error::{Error, Result};

mod config;
pub use config::{Config, ModelPaths};

mod audio;
pub use audio::{PreRollRing, Resampler};

mod sink;
pub use sink::{StdoutSink, StreamSink, VecSink, WakeEvent};

mod engine;
pub use engine::{VoiceActivity, WakeWord};

mod vad;
pub use vad::SileroVad;

mod wake;
pub use wake::OwwWake;

mod detector;
pub use detector::Detector;
