use crate::{Error, Result};
use rubato::{FastFixedIn, PolynomialDegree, Resampler as _};

const TARGET_RATE: u32 = 16_000;
/// Input frames consumed by the resampler per internal `process` call.
const CHUNK: usize = 1024;

/// Downmixes interleaved input to mono and resamples to 16 kHz `f32`.
///
/// Input arrives in arbitrary sizes; leftover mono samples are buffered between
/// calls so the underlying fixed-size resampler always gets full chunks.
pub struct Resampler {
    channels: usize,
    /// `None` ⇒ already 16 kHz; downmix still applied.
    inner: Option<FastFixedIn<f32>>,
    mono_in: Vec<f32>, // buffered mono samples awaiting a full CHUNK
}

impl Resampler {
    pub fn new(input_rate: u32, channels: u16) -> Result<Self> {
        let channels = channels.max(1) as usize;
        let inner = if input_rate == TARGET_RATE {
            None
        } else {
            let ratio = TARGET_RATE as f64 / input_rate as f64;
            let r = FastFixedIn::<f32>::new(ratio, 1.0, PolynomialDegree::Septic, CHUNK, 1)
                .map_err(|e| Error::Resample(e.to_string()))?;
            Some(r)
        };
        Ok(Self {
            channels,
            inner,
            mono_in: Vec::new(),
        })
    }

    /// Average interleaved channels down to mono.
    fn downmix(&self, interleaved: &[f32], out: &mut Vec<f32>) {
        if self.channels == 1 {
            out.extend_from_slice(interleaved);
            return;
        }
        for frame in interleaved.chunks_exact(self.channels) {
            let sum: f32 = frame.iter().sum();
            out.push(sum / self.channels as f32);
        }
    }

    pub fn process(&mut self, interleaved: &[f32]) -> Result<Vec<f32>> {
        let mut mono = Vec::with_capacity(interleaved.len() / self.channels.max(1) + 1);
        self.downmix(interleaved, &mut mono);

        let Some(inner) = self.inner.as_mut() else {
            return Ok(mono); // identity path
        };

        self.mono_in.extend_from_slice(&mono);
        let mut out = Vec::new();
        let mut pos = 0;
        while self.mono_in.len() - pos >= CHUNK {
            let wave_in = vec![self.mono_in[pos..pos + CHUNK].to_vec()];
            let resampled = inner
                .process(&wave_in, None)
                .map_err(|e| Error::Resample(e.to_string()))?;
            out.extend_from_slice(&resampled[0]);
            pos += CHUNK;
        }
        self.mono_in.drain(..pos);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_path_passes_mono_16k_through() {
        let mut r = Resampler::new(16_000, 1).unwrap();
        let out = r.process(&[0.1, 0.2, 0.3]).unwrap();
        assert_eq!(out, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn downmix_stereo_to_mono_at_16k() {
        // stereo 16k → identity rate but channel downmix still applies
        let mut r = Resampler::new(16_000, 2).unwrap();
        // two stereo frames: (0.0,1.0) -> 0.5 ; (0.4,0.6) -> 0.5
        let out = r.process(&[0.0, 1.0, 0.4, 0.6]).unwrap();
        assert_eq!(out, vec![0.5, 0.5]);
    }

    #[test]
    fn downsamples_48k_to_roughly_one_third_length() {
        let mut r = Resampler::new(48_000, 1).unwrap();
        // 9600 input samples @48k ≈ 3200 output samples @16k (allow slack for buffering)
        let input = vec![0.0f32; 9600];
        let out = r.process(&input).unwrap();
        let expected = 3200i64;
        assert!(
            (out.len() as i64 - expected).abs() < 300,
            "got {} expected ~{}",
            out.len(),
            expected
        );
    }
}
