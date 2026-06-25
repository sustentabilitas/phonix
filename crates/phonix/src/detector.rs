use std::collections::VecDeque;

use crate::audio::{PreRollRing, Resampler};
use crate::config::Config;
use crate::engine::{VoiceActivity, WakeWord};
use crate::sink::{StreamSink, WakeEvent};
use crate::Result;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum State {
    Listening,
    Streaming,
}

/// Wake-word detector state machine. Generic over the downstream [`StreamSink`].
pub struct Detector<S: StreamSink> {
    cfg: Config,
    resampler: Resampler,
    ring: PreRollRing,
    vad: Box<dyn VoiceActivity>,
    wake: Box<dyn WakeWord>,
    sink: S,

    vad_q: VecDeque<f32>,
    wake_q: VecDeque<f32>,

    state: State,
    voice: bool,
    silence_run: usize,
    cooldown: usize,
    stream_id: Option<String>,

    // Observability: most recent VAD probability and wake score, for `--debug`/tuning.
    last_vad_prob: f32,
    last_wake_score: f32,
}

impl<S: StreamSink> Detector<S> {
    pub fn with_engines(
        config: Config,
        vad: Box<dyn VoiceActivity>,
        wake: Box<dyn WakeWord>,
        sink: S,
    ) -> Result<Self> {
        let resampler = Resampler::new(config.input_sample_rate, config.input_channels)?;
        let ring = PreRollRing::new(config.pre_roll_samples());
        Ok(Self {
            cfg: config,
            resampler,
            ring,
            vad,
            wake,
            sink,
            vad_q: VecDeque::new(),
            wake_q: VecDeque::new(),
            state: State::Listening,
            voice: false,
            silence_run: 0,
            cooldown: 0,
            stream_id: None,
            last_vad_prob: 0.0,
            last_wake_score: 0.0,
        })
    }

    /// Build a detector with the real Silero VAD + OpenWakeWord engines.
    pub fn new(config: Config, sink: S) -> Result<Self> {
        let vad: Box<dyn VoiceActivity> =
            Box::new(crate::vad::SileroVad::from_path(&config.models.vad)?);
        let wake: Box<dyn WakeWord> = match &config.models.wake {
            None => Box::new(crate::wake::OwwWake::bundled(config.wake_threshold)?),
            Some(p) => Box::new(crate::wake::OwwWake::from_path(
                p,
                "moreni",
                config.wake_threshold,
            )?),
        };
        Self::with_engines(config, vad, wake, sink)
    }

    pub fn set_stream_id(&mut self, id: Option<String>) {
        self.stream_id = id;
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    /// Most recent Silero VAD voice probability `[0,1]` (for debug/tuning).
    pub fn last_vad_prob(&self) -> f32 {
        self.last_vad_prob
    }

    /// Most recent OpenWakeWord score `[0,1]` (for debug/tuning).
    pub fn last_wake_score(&self) -> f32 {
        self.last_wake_score
    }

    pub fn into_sink(self) -> S {
        self.sink
    }

    pub fn push(&mut self, interleaved: &[f32]) -> Result<()> {
        let samples = self.resampler.process(interleaved)?;
        for &s in &samples {
            self.ring.push(s);
            self.vad_q.push_back(s);
            self.wake_q.push_back(s);
        }
        self.drain_vad();
        self.drain_wake();
        // Epilogue: if streaming (state may have just flipped inside drain_wake),
        // deliver the ENTIRE current push's resampled samples via on_audio. This
        // guarantees the audio stream is contiguous with no gap after the trigger,
        // regardless of push block size or pre_roll_ms setting. A small overlap
        // between pre_roll and the first on_audio call is acceptable.
        if self.state == State::Streaming {
            if !samples.is_empty() {
                self.sink.on_audio(&samples);
            }
            if self.silence_run >= self.cfg.end_silence_samples() {
                self.sink.on_end();
                self.state = State::Listening;
                self.cooldown = self.cfg.cooldown_samples();
                self.silence_run = 0;
            }
        }
        Ok(())
    }

    fn drain_vad(&mut self) {
        let n = self.vad.chunk_size();
        while self.vad_q.len() >= n {
            let chunk: Vec<f32> = self.vad_q.drain(..n).collect();
            let prob = self.vad.process(&chunk);
            self.last_vad_prob = prob;
            self.voice = prob > self.cfg.vad_threshold;
            if self.voice {
                self.silence_run = 0;
            } else {
                self.silence_run += n;
            }
            self.cooldown = self.cooldown.saturating_sub(n);
        }
    }

    fn drain_wake(&mut self) {
        let n = self.wake.chunk_size();
        while self.wake_q.len() >= n {
            let chunk: Vec<f32> = self.wake_q.drain(..n).collect();
            // Feed EVERY chunk to the wake model: OpenWakeWord/oww-rs is a streaming
            // model whose internal mel/embedding/detection ring buffers require a
            // continuous sequence of consecutive chunks to recognise the phrase.
            // Gating the inference on VAD would leave gaps and the wake word would
            // never assemble. VAD is used only for end-of-utterance + cooldown timing.
            let score = self.wake.process(&chunk);
            self.last_wake_score = score;
            // A high score is accepted as a trigger only when armed: in the
            // Listening state and past the post-utterance cooldown. (oww-rs's own
            // classifier already rejects non-speech, so no extra VAD voice gate is
            // needed here — and adding one would risk suppressing a valid trigger.)
            if self.state != State::Listening || self.cooldown > 0 {
                continue;
            }
            if score > self.cfg.wake_threshold {
                let event = WakeEvent {
                    model: self.wake.model_name().to_string(),
                    score,
                    pre_roll: self.ring.snapshot(),
                    stream_id: self.stream_id.clone(),
                };
                self.sink.on_wake(&event);
                self.wake_q.clear();
                self.state = State::Streaming;
                self.silence_run = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::fakes::{AlwaysWake, ConstVad, EnergyVad, ScriptedWake};
    use crate::sink::VecSink;

    fn cfg() -> Config {
        Config {
            input_sample_rate: 16_000,
            input_channels: 1,
            ..Config::default()
        }
    }

    /// Build a detector with fake engines (512-sample VAD, 1280-sample wake).
    fn det(vad_prob: f32, fire_on_call: usize) -> Detector<VecSink> {
        Detector::with_engines(
            cfg(),
            Box::new(ConstVad {
                size: 512,
                prob: vad_prob,
            }),
            Box::new(ScriptedWake {
                size: 1280,
                fire_on_call,
                calls: 0,
            }),
            VecSink::default(),
        )
        .unwrap()
    }

    #[test]
    fn wake_below_threshold_never_triggers() {
        // The wake model never scores above threshold (fire_on_call out of reach):
        // no trigger, even with voice present. Rejecting non-wake audio is the wake
        // model's job (oww-rs), not the VAD's.
        let mut d = det(1.0, usize::MAX);
        d.push(&vec![0.5f32; 16_000]).unwrap();
        assert_eq!(d.sink().wakes.len(), 0);
    }

    #[test]
    fn wake_is_fed_continuously_regardless_of_vad_voice() {
        // Regression guard: the wake model is fed every chunk and a high score
        // triggers even when the VAD reports no voice. (Previously the wake
        // inference was VAD-gated, which starved oww-rs's streaming buffers and
        // prevented detection entirely.)
        let mut d = det(0.0, 1); // ConstVad prob 0.0 ⇒ "no voice"
        d.push(&vec![0.0f32; 16_000]).unwrap();
        assert_eq!(
            d.sink().wakes.len(),
            1,
            "wake must fire on a high score even when VAD says no voice"
        );
    }

    #[test]
    fn voice_plus_wake_fires_once_with_full_preroll() {
        // Voice active; wake fires on its 2nd 1280-chunk.
        let mut d = det(1.0, 2);
        // Feed 1s of audio = 16000 samples ⇒ 12 wake chunks, 31 vad chunks.
        d.push(&vec![0.5f32; 16_000]).unwrap();
        let s = d.sink();
        assert_eq!(s.wakes.len(), 1, "exactly one trigger");
        // pre-roll ring capacity = 500ms = 8000 samples; it is full by chunk 2.
        assert_eq!(s.wakes[0].pre_roll.len(), 8_000);
        assert_eq!(s.wakes[0].model, "fake");
    }

    /// Test that silence triggers on_end and that cooldown then blocks re-trigger,
    /// before eventually expiring and allowing a second wake event.
    ///
    /// Arithmetic (all at 16 kHz mono, VAD chunk=512, wake chunk=1280):
    ///
    /// Phase 1 — trigger:
    ///   Feed 10_240 voice samples (0.5 amplitude) = 20 VAD chunks × 512 (no leftovers)
    ///   = 8 wake chunks × 1_280 (8 × 1280 = 10_240, no leftovers).
    ///   Ring fills after 8_000 samples; 8 wake chunks arrive, first fires immediately
    ///   (AlwaysWake returns 1.0, state=Listening, cooldown=0, voice=true).
    ///   wake_q.clear() removes remaining chunks. state→Streaming, silence_run=0.
    ///   Expected: wakes==1, ends==0. No queue leftovers.
    ///
    /// Phase 2 — silence to end utterance:
    ///   Feed 11_264 silent samples (0.0 amplitude) = 22 VAD chunks × 512 (no leftovers).
    ///   First chunk pure silence (no leftover voice from phase 1 pollutes it).
    ///   voice=false each chunk → silence_run += 512 each time = 11_264 ≥ 11_200 (end_silence_samples).
    ///   on_end fires, state→Listening, cooldown=24_000, silence_run=0.
    ///   Expected: wakes==1, ends==1. No queue leftovers.
    ///
    /// Phase 3 — cooldown blocks re-trigger:
    ///   Feed 2_560 voice samples = 5 VAD chunks × 512 (no leftovers) = 2 wake chunks × 1_280.
    ///   drain_vad: cooldown = 24_000 - 5×512 = 21_440 > 0; voice=true.
    ///   drain_wake: 2 chunks processed; guard (cooldown > 0) causes continue on both.
    ///   Expected: wakes still 1 (cooldown blocks). No queue leftovers.
    ///
    /// Phase 4 — cooldown expires, re-trigger allowed:
    ///   Feed 22_784 voice samples = 44 VAD chunks × 512 + 256 leftover (ok, doesn't matter).
    ///   drain_vad: 44 chunks; after chunk 42 (21_504 consumed total, ≥ 21_440 remaining
    ///   cooldown) saturating_sub drives cooldown to 0; chunks 43-44 keep voice=true.
    ///   drain_wake: 17 full chunks × 1_280 in wake_q; first fires (cooldown=0, voice=true).
    ///   Expected: wakes==2 (second trigger after cooldown expires), ends still 1.
    #[test]
    fn silence_emits_on_end_and_cooldown_blocks_then_allows_retrigger() {
        use crate::engine::fakes::{AlwaysWake, EnergyVad};

        let mut d = Detector::with_engines(
            cfg(),
            Box::new(EnergyVad { size: 512 }),
            Box::new(AlwaysWake { size: 1280 }),
            VecSink::default(),
        )
        .unwrap();

        // Phase 1: 10_240 = 20 VAD chunks × 512 = 8 wake chunks × 1_280 — no queue leftovers.
        // Ring fills at 8_000; first wake chunk fires immediately (AlwaysWake + voice + Listening).
        d.push(&vec![0.5f32; 10_240]).unwrap();
        assert_eq!(d.sink().wakes.len(), 1, "phase 1: exactly one wake trigger");
        assert_eq!(d.sink().ends, 0, "phase 1: no end yet");

        // Phase 2: 11_264 = 22 VAD chunks × 512 — no leftovers so all chunks are pure silence.
        // silence_run = 22 × 512 = 11_264 ≥ end_silence_samples (11_200) → on_end fires.
        d.push(&vec![0.0f32; 11_264]).unwrap();
        assert_eq!(d.sink().ends, 1, "phase 2: on_end fired exactly once");
        assert_eq!(
            d.sink().wakes.len(),
            1,
            "phase 2: no new wake during silence"
        );

        // Phase 3: 2_560 = 5 VAD chunks × 512 = 2 wake chunks × 1_280 — no leftovers.
        // drain_vad: cooldown 24_000 → 21_440. drain_wake: both chunks skipped (cooldown > 0).
        d.push(&vec![0.5f32; 2_560]).unwrap();
        assert_eq!(
            d.sink().wakes.len(),
            1,
            "phase 3: cooldown blocks re-trigger"
        );
        assert_eq!(d.sink().ends, 1, "phase 3: no additional end");

        // Phase 4: 22_784 voice samples. drain_vad processes 44 chunks × 512; after
        // 42 chunks (21_504 > 21_440 remaining cooldown), saturating_sub drives cooldown to 0.
        // drain_wake then sees 17 wake chunks with cooldown=0 + voice=true → first fires.
        d.push(&vec![0.5f32; 22_784]).unwrap();
        assert_eq!(
            d.sink().wakes.len(),
            2,
            "phase 4: second wake after cooldown expires"
        );
        assert_eq!(d.sink().ends, 1, "phase 4: still exactly one end event");
    }

    /// Regression: pushing a block LARGER than the pre_roll window must not drop
    /// any audio between the trigger point and the end of the push.
    ///
    /// Arithmetic (16 kHz mono, EnergyVad size=512, AlwaysWake size=1280):
    ///   pre_roll_ms = 100 → pre_roll_samples = 1_600
    ///   Push 4_096 voice samples (amplitude 0.5).
    ///   drain_vad: 8 VAD chunks × 512; first chunk sets voice=true.
    ///   drain_wake: first 1_280-chunk fires immediately (Listening + cooldown=0 + voice).
    ///     wake_q.clear(), state→Streaming.
    ///   Epilogue: state==Streaming → on_audio(&samples) where samples.len()==4_096.
    ///   Expected: exactly 1 wake, sink.audio.len()==4_096 (whole push delivered, no gap).
    #[test]
    fn trigger_midpush_streams_whole_push_no_gap() {
        let cfg = Config {
            input_sample_rate: 16_000,
            input_channels: 1,
            pre_roll_ms: 100,
            ..Config::default()
        };
        let mut d = Detector::with_engines(
            cfg,
            Box::new(EnergyVad { size: 512 }),
            Box::new(AlwaysWake { size: 1280 }),
            VecSink::default(),
        )
        .unwrap();

        // Single large push: 4_096 > pre_roll_samples (1_600).
        // The wake fires partway through (after the first 1_280-sample wake chunk).
        // The whole push must still be delivered via on_audio.
        d.push(&vec![0.5f32; 4_096]).unwrap();

        let s = d.sink();
        assert_eq!(s.wakes.len(), 1, "exactly one wake trigger");
        assert_eq!(
            s.audio.len(),
            4_096,
            "entire push delivered via on_audio (no gap after trigger)"
        );
    }
}
