//! Smoke test the real detector wiring. Requires the Silero model; skips if absent.
use std::path::{Path, PathBuf};

use phonix::{Config, Detector, ModelPaths, VecSink};

#[test]
fn real_detector_processes_silence_without_triggering() {
    let model_path = Path::new("models/silero_vad.onnx");
    if !model_path.exists() {
        eprintln!("skipping: models/silero_vad.onnx not fetched");
        return;
    }
    let cfg = Config {
        input_sample_rate: 16_000,
        input_channels: 1,
        // Explicit path because Config::default() uses a repo-root-relative path but integration tests run from the crate dir.
        models: ModelPaths {
            vad: PathBuf::from("models/silero_vad.onnx"),
            wake: None,
        },
        ..Config::default()
    };
    let mut d = Detector::new(cfg, VecSink::default()).unwrap();
    d.push(&vec![0.0f32; 32_000]).unwrap(); // 2s silence
    assert_eq!(d.sink().wakes.len(), 0);
}
