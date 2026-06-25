use std::path::PathBuf;

/// Filesystem locations of the ONNX models.
#[derive(Debug, Clone)]
pub struct ModelPaths {
    /// Silero VAD model (`silero_vad.onnx`).
    ///
    /// The default value `"crates/phonix/models/silero_vad.onnx"` is relative to
    /// the **workspace root**. This is correct when running the `phonix-listen`
    /// binary from the workspace root (as the README shows). Unit tests run from
    /// the crate directory (`crates/phonix/`) and therefore use
    /// `"models/silero_vad.onnx"` instead. Embedders or library callers should
    /// supply an explicit, absolute path to avoid any working-directory ambiguity.
    pub vad: PathBuf,
    /// Wake classifier. `None` uses the bundled `hey_jarvis` model from `oww-rs`;
    /// `Some(path)` loads a custom model such as `moreni.onnx`.
    pub wake: Option<PathBuf>,
}

/// Runtime configuration for the [`crate::Detector`].
#[derive(Debug, Clone)]
pub struct Config {
    /// Sample rate of the audio handed to `push` (e.g. 48_000 for a mic).
    pub input_sample_rate: u32,
    /// Channel count of the audio handed to `push` (downmixed to mono).
    pub input_channels: u16,
    /// Silero voice probability above which a chunk counts as speech (default 0.5).
    pub vad_threshold: f32,
    /// Wake classifier score above which the wake phrase fires (default 0.5).
    pub wake_threshold: f32,
    /// Length of audio retained before a trigger and delivered as pre-roll (default 500).
    pub pre_roll_ms: u32,
    /// Refractory period after an utterance ends, suppressing re-triggers (default 1500).
    pub cooldown_ms: u32,
    /// Continuous silence that ends a streaming utterance (default 700).
    pub end_silence_ms: u32,
    /// Model file locations.
    pub models: ModelPaths,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            input_sample_rate: 16_000,
            input_channels: 1,
            vad_threshold: 0.5,
            wake_threshold: 0.5,
            pre_roll_ms: 500,
            cooldown_ms: 1_500,
            end_silence_ms: 700,
            models: ModelPaths {
                vad: PathBuf::from("crates/phonix/models/silero_vad.onnx"),
                wake: None,
            },
        }
    }
}

impl Config {
    /// Samples retained in the pre-roll ring (`pre_roll_ms` at 16 kHz mono).
    pub fn pre_roll_samples(&self) -> usize {
        self.pre_roll_ms as usize * 16
    }
    /// Cooldown length in 16 kHz samples.
    pub fn cooldown_samples(&self) -> usize {
        self.cooldown_ms as usize * 16
    }
    /// End-of-utterance silence length in 16 kHz samples.
    pub fn end_silence_samples(&self) -> usize {
        self.end_silence_ms as usize * 16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sane_and_samples_convert() {
        let c = Config::default();
        assert_eq!(c.pre_roll_ms, 500);
        assert_eq!(c.pre_roll_samples(), 8_000);
        assert_eq!(c.cooldown_samples(), 24_000);
        assert_eq!(c.end_silence_samples(), 11_200);
        assert!(c.models.wake.is_none());
    }
}
