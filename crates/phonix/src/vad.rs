use std::path::Path;
use std::sync::Arc;

use tract_onnx::prelude::*;
use tract_onnx::tract_hir::prelude::InferenceSimplePlan;

use crate::engine::VoiceActivity;
use crate::{Error, Result};

const SR: i64 = 16_000;
const CHUNK: usize = 512;
const CONTEXT: usize = 64;
const STATE_LEN: usize = 2 * 128; // 256 = 2 × 128 (state tensor [2,1,128] flattened)

// Silero v5 uses nested ONNX `If` ops that tract-onnx 0.23 cannot translate to
// TypedModel (into_typed() / into_optimized() fail on nested If->to_typed).
// We therefore stay on InferenceModel -> InferenceSimplePlan, which runs via
// EvalOp::eval() and works correctly at inference time.
type Runnable = Arc<InferenceSimplePlan>;

/// Silero VAD v5 running on tract. Consumes 512-sample 16 kHz mono chunks.
pub struct SileroVad {
    model: Runnable,
    state: Vec<f32>,   // [2,1,128]
    context: Vec<f32>, // last 64 samples of the previous chunk
}

impl SileroVad {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        // Probe confirmed: input[0]=input [1,576] f32, input[1]=state [2,1,128] f32, input[2]=sr scalar i64
        //
        // Silero v5 uses an ONNX `If` node guarded by `sr == 16000`.
        // `into_optimized()` fails because nested If ops can't be translated to TypedModel.
        // Running `into_runnable()` directly on the analysed InferenceModel avoids this;
        // tract executes the If via EvalOp::eval() which branches at runtime.
        let sr_fact: InferenceFact = tensor0(SR).into();
        let model = tract_onnx::onnx()
            .model_for_path(path.as_ref())
            .map_err(|e| Error::ModelLoad(format!("silero: {e}")))?
            .with_input_fact(0, f32::fact([1usize, CONTEXT + CHUNK]).into())
            .and_then(|m| m.with_input_fact(1, f32::fact([2usize, 1usize, 128usize]).into()))
            .and_then(|m| m.with_input_fact(2, sr_fact))
            .map_err(|e| Error::ModelShape(format!("silero input facts: {e}")))?
            .into_runnable()
            .map_err(|e| Error::ModelLoad(format!("silero runnable: {e}")))?;
        Ok(Self {
            model,
            state: vec![0.0; STATE_LEN],
            context: vec![0.0; CONTEXT],
        })
    }

    fn run(&mut self, chunk: &[f32]) -> Result<f32> {
        // Build [1, 576] = context(64) + chunk(512).
        let mut input = Vec::with_capacity(CONTEXT + CHUNK);
        input.extend_from_slice(&self.context);
        input.extend_from_slice(chunk);

        let in_t: Tensor = tract_ndarray::Array2::from_shape_vec((1, CONTEXT + CHUNK), input)
            .map_err(|e| Error::Inference(format!("input shape: {e}")))?
            .into();
        let state_t: Tensor =
            tract_ndarray::Array3::from_shape_vec((2, 1, 128), self.state.clone())
                .map_err(|e| Error::Inference(format!("state shape: {e}")))?
                .into();
        let sr_t: Tensor = tensor0(SR);

        let out = self
            .model
            .run(tvec!(in_t.into(), state_t.into(), sr_t.into()))
            .map_err(|e| Error::Inference(format!("silero run: {e}")))?;

        let prob = out[0]
            .to_plain_array_view::<f32>()
            .map_err(|e| Error::Inference(format!("prob view: {e}")))?
            .iter()
            .copied()
            .next()
            .ok_or_else(|| Error::ModelShape("silero empty prob".into()))?;

        self.state = out[1]
            .to_plain_array_view::<f32>()
            .map_err(|e| Error::Inference(format!("state view: {e}")))?
            .iter()
            .copied()
            .collect();

        // Roll context: last 64 samples of this chunk become next call's context.
        self.context.copy_from_slice(&chunk[CHUNK - CONTEXT..]);
        Ok(prob)
    }
}

impl VoiceActivity for SileroVad {
    fn chunk_size(&self) -> usize {
        CHUNK
    }

    fn process(&mut self, chunk: &[f32]) -> f32 {
        debug_assert_eq!(chunk.len(), CHUNK);
        self.run(chunk).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "models/silero_vad.onnx"; // cwd is crates/phonix during `cargo test -p`

    fn model_present() -> bool {
        Path::new(MODEL).exists()
    }

    #[test]
    fn silence_scores_low() {
        if !model_present() {
            eprintln!("skipping: {MODEL} not fetched");
            return;
        }
        let mut vad = SileroVad::from_path(MODEL).unwrap();
        let mut last = 1.0f32;
        for _ in 0..10 {
            last = vad.process(&[0.0f32; CHUNK]);
        }
        assert!(last < 0.5, "silence prob should be low, got {last}");
    }

    #[test]
    fn output_is_a_probability() {
        if !model_present() {
            eprintln!("skipping: {MODEL} not fetched");
            return;
        }
        let mut vad = SileroVad::from_path(MODEL).unwrap();
        let mut x = vec![0.0f32; CHUNK];
        for (i, s) in x.iter_mut().enumerate() {
            *s = (i as f32 * 0.05).sin() * 0.3; // 250 Hz-ish tone
        }
        let p = vad.process(&x);
        assert!((0.0..=1.0).contains(&p), "prob out of range: {p}");
    }
}
