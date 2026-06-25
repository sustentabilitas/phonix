/// Voice-activity detector over fixed-size 16 kHz mono chunks.
pub trait VoiceActivity: Send {
    /// Exact number of samples [`process`](Self::process) expects per call.
    fn chunk_size(&self) -> usize;
    /// Returns voice probability in `[0, 1]` for one chunk.
    fn process(&mut self, chunk: &[f32]) -> f32;
}

/// Wake-word stage over fixed-size 16 kHz mono chunks.
pub trait WakeWord: Send {
    /// Exact number of samples [`process`](Self::process) expects per call.
    fn chunk_size(&self) -> usize;
    /// Name reported in [`crate::WakeEvent::model`].
    fn model_name(&self) -> &str;
    /// Returns the latest wake score in `[0, 1]` for one chunk.
    fn process(&mut self, chunk: &[f32]) -> f32;
}

#[cfg(test)]
pub(crate) mod fakes {
    use super::*;

    /// VAD that returns a constant probability.
    pub struct ConstVad {
        pub size: usize,
        pub prob: f32,
    }
    impl VoiceActivity for ConstVad {
        fn chunk_size(&self) -> usize {
            self.size
        }
        fn process(&mut self, _chunk: &[f32]) -> f32 {
            self.prob
        }
    }

    /// Wake stage that fires (score 1.0) on its Nth chunk, else 0.0.
    pub struct ScriptedWake {
        pub size: usize,
        pub fire_on_call: usize,
        pub calls: usize,
    }
    impl WakeWord for ScriptedWake {
        fn chunk_size(&self) -> usize {
            self.size
        }
        fn model_name(&self) -> &str {
            "fake"
        }
        fn process(&mut self, _chunk: &[f32]) -> f32 {
            self.calls += 1;
            if self.calls == self.fire_on_call {
                1.0
            } else {
                0.0
            }
        }
    }

    /// VAD that classifies audio by energy: returns 1.0 if mean absolute amplitude
    /// exceeds 0.01 (voice), else 0.0 (silence). Lets tests control voice/silence
    /// purely by the audio they feed (0.5 = voice, 0.0 = silence).
    pub struct EnergyVad {
        pub size: usize,
    }
    impl VoiceActivity for EnergyVad {
        fn chunk_size(&self) -> usize {
            self.size
        }
        fn process(&mut self, chunk: &[f32]) -> f32 {
            let mean_abs = chunk.iter().map(|x| x.abs()).sum::<f32>() / chunk.len() as f32;
            if mean_abs > 0.01 {
                1.0
            } else {
                0.0
            }
        }
    }

    /// Wake stage that always fires (score 1.0). Used so the detector can
    /// re-trigger after cooldown expires — ScriptedWake only fires once ever.
    pub struct AlwaysWake {
        pub size: usize,
    }
    impl WakeWord for AlwaysWake {
        fn chunk_size(&self) -> usize {
            self.size
        }
        fn model_name(&self) -> &str {
            "always"
        }
        fn process(&mut self, _chunk: &[f32]) -> f32 {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fakes::*;
    use super::*;

    #[test]
    fn scripted_wake_fires_only_on_target_call() {
        let mut w = ScriptedWake {
            size: 1280,
            fire_on_call: 2,
            calls: 0,
        };
        assert_eq!(w.process(&[]), 0.0);
        assert_eq!(w.process(&[]), 1.0);
        assert_eq!(w.process(&[]), 0.0);
        assert_eq!(w.chunk_size(), 1280);
    }

    #[test]
    fn energy_vad_returns_one_for_loud_chunk_and_zero_for_silent() {
        let mut v = EnergyVad { size: 512 };
        // Loud chunk: amplitude 0.5 well above 0.01 threshold.
        assert_eq!(v.process(&vec![0.5f32; 512]), 1.0);
        // Silent chunk: all zeros, mean abs = 0.0.
        assert_eq!(v.process(&vec![0.0f32; 512]), 0.0);
        assert_eq!(v.chunk_size(), 512);
    }
}
