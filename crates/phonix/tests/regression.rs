//! Fixture-driven recall and false-positive regression tests.
//! Empty fixture folders pass with a printed notice.
use std::fs;
use std::path::{Path, PathBuf};

use phonix::{Config, Detector, VecSink};

fn wavs(dir: &str) -> Vec<PathBuf> {
    let p = Path::new(dir);
    if !p.exists() {
        return Vec::new();
    }
    fs::read_dir(p)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "wav").unwrap_or(false))
        .collect()
}

fn count_wakes(path: &Path) -> usize {
    if !Path::new("models/silero_vad.onnx").exists() {
        return usize::MAX; // sentinel: model missing ⇒ skip assertions
    }
    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    let mut cfg = Config {
        input_sample_rate: spec.sample_rate,
        input_channels: spec.channels,
        ..Config::default()
    };
    // Tests run with the crate dir as cwd, so use the crate-relative model path
    // (matching the skip-check above) rather than Config::default's
    // workspace-root-relative default.
    cfg.models.vad = "models/silero_vad.onnx".into();
    let mut d = Detector::new(cfg, VecSink::default()).unwrap();
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap_or(0) as f32 / i16::MAX as f32)
        .collect();
    for block in samples.chunks(4096) {
        d.push(block).unwrap();
    }
    d.sink().wakes.len()
}

#[test]
fn positives_have_recall() {
    let files = wavs("tests/fixtures/positive");
    if files.is_empty() {
        eprintln!("notice: no positive fixtures yet");
        return;
    }
    for f in files {
        let n = count_wakes(&f);
        if n == usize::MAX {
            eprintln!("skip (no model): {}", f.display());
            continue;
        }
        assert!(n >= 1, "expected a detection in {}", f.display());
    }
}

#[test]
fn negatives_have_no_false_positives() {
    let files = wavs("tests/fixtures/negative");
    if files.is_empty() {
        eprintln!("notice: no negative fixtures yet");
        return;
    }
    for f in files {
        let n = count_wakes(&f);
        if n == usize::MAX {
            eprintln!("skip (no model): {}", f.display());
            continue;
        }
        assert_eq!(n, 0, "false positive in {}", f.display());
    }
}
