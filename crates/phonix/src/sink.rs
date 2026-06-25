/// Emitted when the wake phrase is detected.
#[derive(Debug, Clone)]
pub struct WakeEvent {
    /// Model that fired (`"hey_jarvis"` now, `"moreni"` later).
    pub model: String,
    /// Classifier confidence at the trigger.
    pub score: f32,
    /// ~`pre_roll_ms` of 16 kHz mono audio captured *before* the trigger.
    pub pre_roll: Vec<f32>,
    /// Participant/stream identifier, for per-stream meeting use.
    pub stream_id: Option<String>,
}

/// Downstream consumer of detections — the live-LLM boundary.
///
/// Call order guarantees the pre-roll is delivered before any live audio:
/// `on_wake` (carrying `pre_roll`) fires first, then zero or more `on_audio`
/// for the live utterance, then exactly one `on_end`.
pub trait StreamSink: Send {
    fn on_wake(&mut self, event: &WakeEvent);
    fn on_audio(&mut self, frames: &[f32]);
    fn on_end(&mut self);
}

/// Human-readable sink for the test binary.
pub struct StdoutSink;

impl StreamSink for StdoutSink {
    fn on_wake(&mut self, event: &WakeEvent) {
        eprintln!(
            "[wake] model={} score={:.3} pre_roll={} samples ({} ms)",
            event.model,
            event.score,
            event.pre_roll.len(),
            event.pre_roll.len() / 16
        );
    }
    fn on_audio(&mut self, frames: &[f32]) {
        eprintln!(
            "[audio] {} samples ({} ms)",
            frames.len(),
            frames.len() / 16
        );
    }
    fn on_end(&mut self) {
        eprintln!("[end] utterance complete");
    }
}

/// Test sink: records every call.
#[derive(Default)]
pub struct VecSink {
    pub wakes: Vec<WakeEvent>,
    pub audio: Vec<f32>,
    pub ends: usize,
}

impl StreamSink for VecSink {
    fn on_wake(&mut self, event: &WakeEvent) {
        self.wakes.push(event.clone());
    }
    fn on_audio(&mut self, frames: &[f32]) {
        self.audio.extend_from_slice(frames);
    }
    fn on_end(&mut self) {
        self.ends += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_sink_records_calls_in_shape() {
        let mut s = VecSink::default();
        s.on_wake(&WakeEvent {
            model: "hey_jarvis".into(),
            score: 0.9,
            pre_roll: vec![0.0; 8000],
            stream_id: None,
        });
        s.on_audio(&[0.1, 0.2]);
        s.on_end();
        assert_eq!(s.wakes.len(), 1);
        assert_eq!(s.wakes[0].pre_roll.len(), 8000);
        assert_eq!(s.audio, vec![0.1, 0.2]);
        assert_eq!(s.ends, 1);
    }
}
