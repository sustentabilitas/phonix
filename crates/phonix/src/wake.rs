use std::path::Path;

use crate::engine::WakeWord;
use crate::{Error, Result};

const CHUNK: usize = 1280;

/// OpenWakeWord wake stage backed by `oww-rs` (pure-Rust tract pipeline:
/// melspectrogram → embedding → classifier). Consumes 1280-sample 16 kHz chunks.
///
/// The `threshold` passed at construction is oww-rs's internal detection
/// threshold. It is **not** optional tuning: oww-rs's `Detection.probability` is
/// an *average of the per-frame scores that exceed this threshold* (see
/// `calculate_average` in oww-rs), returned only when that average itself exceeds
/// the threshold. Constructing with `0.0` averages every nonzero frame and
/// dilutes a genuine wake to ~0.25; constructing with the real wake threshold
/// (e.g. 0.5) yields ~0.97 for a true wake and 0.0 for non-wake audio. Pass the
/// same value the detector compares against (`Config::wake_threshold`).
///
/// Note: oww-rs's `Detection.detected` boolean is deliberately ignored — it uses a
/// wall-clock 2 s debounce that assumes real-time feeding and never fires when
/// audio is processed faster than real time (e.g. offline WAV/regression tests).
pub struct OwwWake {
    model: oww_rs::oww::OwwModel,
    name: String,
}

impl OwwWake {
    /// Use the bundled `hey_jarvis` classifier (the bootstrap model). `threshold`
    /// is oww-rs's detection threshold — pass `Config::wake_threshold`.
    pub fn bundled(threshold: f32) -> Result<Self> {
        let model = oww_rs::oww::OwwModel::new(
            oww_rs::config::SpeechUnlockType::OpenWakeWordHeyJarvis,
            threshold,
        )
        .map_err(|e| Error::ModelLoad(format!("oww hey_jarvis: {e}")))?;
        Ok(Self {
            model,
            name: "hey_jarvis".into(),
        })
    }

    /// Load a custom classifier such as `moreni.onnx`. `threshold` is oww-rs's
    /// detection threshold — pass `Config::wake_threshold`.
    ///
    /// Note: `name` is passed as oww-rs's `model_unlock_word`; its effect for custom
    /// `.onnx` files should be verified when `moreni.onnx` is integrated.
    pub fn from_path<P: AsRef<Path>>(path: P, name: &str, threshold: f32) -> Result<Self> {
        let model = oww_rs::oww::OwwModel::from_file(path.as_ref(), name.to_string(), threshold)
            .map_err(|e| Error::ModelLoad(format!("oww {name}: {e}")))?;
        Ok(Self {
            model,
            name: name.to_string(),
        })
    }
}

impl WakeWord for OwwWake {
    fn chunk_size(&self) -> usize {
        CHUNK
    }

    fn model_name(&self) -> &str {
        &self.name
    }

    fn process(&mut self, chunk: &[f32]) -> f32 {
        debug_assert_eq!(chunk.len(), CHUNK);
        self.model.detection(chunk.to_vec()).probability
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_loads_and_silence_does_not_fire() {
        let mut w = OwwWake::bundled(0.5).unwrap();
        assert_eq!(w.chunk_size(), 1280);
        assert_eq!(w.model_name(), "hey_jarvis");
        let mut last = 1.0;
        for _ in 0..20 {
            last = w.process(&[0.0f32; CHUNK]);
        }
        assert!(last < 0.5, "silence should not trigger wake, got {last}");
    }
}
