//! phonix-eval — offline recall / false-positive evaluation for the phonix
//! wake-word detector.
//!
//! Point it at a `positive/` corpus (clips that say the wake word) and a
//! `negative/` corpus (conversational / confusable audio that must not trigger),
//! optionally degraded through Opus + background speakers (see scripts/eval/),
//! and it reports detection rate and false-positive rate across a threshold
//! sweep — the numbers you need to choose `wake_threshold` and to decide whether
//! the model is good enough for production.
//!
//! Run from the workspace root, e.g.:
//!   cargo run -p phonix-eval -- \
//!     --positive eval/opus24k/positive --negative eval/opus24k/negative \
//!     --thresholds 0.3,0.5,0.7

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use phonix::{Config, Detector, VecSink};

#[derive(Parser)]
#[command(
    name = "phonix-eval",
    about = "Recall / false-positive eval on degraded audio"
)]
struct Args {
    /// Directory of WAV clips that DO contain the wake word.
    #[arg(long, default_value = "eval/positive")]
    positive: PathBuf,
    /// Directory of WAV clips that do NOT (conversation, confusables, noise).
    #[arg(long, default_value = "eval/negative")]
    negative: PathBuf,
    /// Silero VAD model path (relative to the working directory).
    #[arg(long, default_value = "crates/phonix/models/silero_vad.onnx")]
    vad_model: PathBuf,
    /// Custom wake model; omit to use the bundled `hey_jarvis`.
    #[arg(long)]
    wake_model: Option<PathBuf>,
    /// Comma-separated wake thresholds to sweep.
    #[arg(long, default_value = "0.3,0.5,0.7")]
    thresholds: String,
}

/// A WAV decoded to interleaved f32 in [-1, 1], with its rate and channel count.
struct Clip {
    name: String,
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
}

impl Clip {
    fn seconds(&self) -> f32 {
        let frames = self.samples.len() / self.channels.max(1) as usize;
        frames as f32 / self.sample_rate as f32
    }
}

fn read_wav(path: &Path) -> Result<Clip> {
    let mut reader =
        hound::WavReader::open(path).with_context(|| format!("open {}", path.display()))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / scale)
                .collect()
        }
        hound::SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap_or(0.0)).collect(),
    };
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Clip {
        name,
        samples,
        sample_rate: spec.sample_rate,
        channels: spec.channels,
    })
}

fn load_corpus(dir: &Path) -> Result<Vec<Clip>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut clips = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().map(|e| e == "wav").unwrap_or(false) {
            clips.push(read_wav(&path)?);
        }
    }
    clips.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(clips)
}

/// Run one clip through a fresh detector at `threshold`; return the number of
/// wake events fired.
fn count_wakes(clip: &Clip, base: &Config, threshold: f32) -> Result<usize> {
    let cfg = Config {
        input_sample_rate: clip.sample_rate,
        input_channels: clip.channels,
        wake_threshold: threshold,
        ..base.clone()
    };
    let mut det = Detector::new(cfg, VecSink::default())?;
    for block in clip.samples.chunks(4096) {
        det.push(block)?;
    }
    Ok(det.sink().wakes.len())
}

/// Detection rate over the positive corpus: fraction of clips that fired ≥1.
fn recall(pos_detected: usize, pos_total: usize) -> f32 {
    if pos_total == 0 {
        f32::NAN
    } else {
        pos_detected as f32 / pos_total as f32
    }
}

/// False positives per hour of negative audio.
fn fp_per_hour(fp_events: usize, neg_seconds: f32) -> f32 {
    if neg_seconds <= 0.0 {
        0.0
    } else {
        fp_events as f32 / (neg_seconds / 3600.0)
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    let thresholds: Vec<f32> = args
        .thresholds
        .split(',')
        .map(|s| s.trim().parse::<f32>())
        .collect::<std::result::Result<_, _>>()
        .context("parsing --thresholds")?;

    let mut base = Config::default();
    base.models.vad = args.vad_model.clone();
    base.models.wake = args.wake_model.clone();

    let positives = load_corpus(&args.positive)?;
    let negatives = load_corpus(&args.negative)?;
    let neg_seconds: f32 = negatives.iter().map(Clip::seconds).sum();
    let model = args
        .wake_model
        .as_deref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "hey_jarvis (bundled)".into());

    eprintln!(
        "model: {model}\npositives: {} clips    negatives: {} clips ({:.1}s)\n",
        positives.len(),
        negatives.len(),
        neg_seconds
    );
    if positives.is_empty() && negatives.is_empty() {
        eprintln!(
            "no clips found — put WAVs under {} and {} (see scripts/eval/ to generate a degraded corpus)",
            args.positive.display(),
            args.negative.display()
        );
        return Ok(());
    }

    println!("threshold   recall (pos)     neg clips fired   FP events   FP/hour");
    println!("---------   --------------   ---------------   ---------   -------");
    for &t in &thresholds {
        let mut pos_detected = 0usize;
        for c in &positives {
            if count_wakes(c, &base, t)? >= 1 {
                pos_detected += 1;
            }
        }
        let mut neg_fired = 0usize;
        let mut fp_events = 0usize;
        for c in &negatives {
            let n = count_wakes(c, &base, t)?;
            if n >= 1 {
                neg_fired += 1;
                fp_events += n;
            }
        }
        let recall_pct = recall(pos_detected, positives.len()) * 100.0;
        let fph = fp_per_hour(fp_events, neg_seconds);
        println!(
            "{:<9.2}   {:>2}/{:<2} ({:>5.1}%)   {:>3}/{:<3}           {:>5}      {:>6.1}",
            t,
            pos_detected,
            positives.len(),
            recall_pct,
            neg_fired,
            negatives.len(),
            fp_events,
            fph,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recall_is_fraction_detected() {
        assert!((recall(2, 4) - 0.5).abs() < 1e-6);
        assert!(recall(0, 0).is_nan());
    }

    #[test]
    fn fp_per_hour_scales_with_duration() {
        // 3 false positives over 30 minutes (1800s) = 6 per hour.
        assert!((fp_per_hour(3, 1800.0) - 6.0).abs() < 1e-3);
        assert_eq!(fp_per_hour(5, 0.0), 0.0);
    }

    #[test]
    fn clip_seconds_accounts_for_channels() {
        let c = Clip {
            name: "x".into(),
            samples: vec![0.0; 32_000], // 16000 stereo frames
            sample_rate: 16_000,
            channels: 2,
        };
        assert!((c.seconds() - 1.0).abs() < 1e-6);
    }
}
