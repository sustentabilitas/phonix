//! Local tester for the phonix wake-word detector.
//!
//! `cargo run --features cli --bin phonix-listen -- mic`
//! `cargo run --features cli --bin phonix-listen -- file clip.wav`

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use phonix::{Config, Detector, StdoutSink};

#[derive(Parser)]
#[command(name = "phonix-listen", about = "Local wake-word detector tester")]
struct Cli {
    #[arg(long, global = true, default_value_t = 0.5)]
    wake_threshold: f32,
    #[arg(long, global = true, default_value_t = 0.5)]
    vad_threshold: f32,
    #[arg(long, global = true, default_value_t = 500)]
    pre_roll_ms: u32,
    #[arg(long, global = true)]
    wake_model: Option<PathBuf>,
    /// Print live peak amplitude, VAD probability, and wake score (~2×/sec) for
    /// diagnosing capture/permission/threshold issues.
    #[arg(long, global = true)]
    debug: bool,
    /// Select the input device whose name contains this substring (mic mode).
    /// Without it, the system default input device is used.
    #[arg(long, global = true)]
    device: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

/// Accumulates audio stats and prints a `[debug]` line roughly twice a second.
struct DebugMeter {
    enabled: bool,
    interval: usize,
    acc: usize,
    peak: f32,
}

impl DebugMeter {
    fn new(enabled: bool, sample_rate: u32) -> Self {
        Self {
            enabled,
            interval: (sample_rate / 2).max(1) as usize,
            acc: 0,
            peak: 0.0,
        }
    }

    /// Record a block (input-rate, interleaved) and emit a line once per interval.
    fn observe<S: phonix::StreamSink>(&mut self, block: &[f32], detector: &Detector<S>) {
        if !self.enabled {
            return;
        }
        self.peak = block.iter().fold(self.peak, |m, &x| m.max(x.abs()));
        self.acc += block.len();
        if self.acc >= self.interval {
            eprintln!(
                "[debug] peak={:.4} vad_prob={:.3} wake_score={:.3}",
                self.peak,
                detector.last_vad_prob(),
                detector.last_wake_score()
            );
            self.acc = 0;
            self.peak = 0.0;
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// Capture from an input device (default, or --device <name>).
    Mic,
    /// Read a 16-bit PCM WAV file.
    File { path: PathBuf },
    /// List available input devices and their default configs.
    Devices,
}

/// Print every input device with its default config, marking the system default.
fn list_devices() -> phonix::Result<()> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();
    let devices = host
        .input_devices()
        .map_err(|e| phonix::Error::Audio(e.to_string()))?;
    eprintln!("Input devices (▶ = system default):");
    for device in devices {
        let name = device.name().unwrap_or_else(|_| "<unknown>".into());
        let marker = if name == default_name { "▶" } else { " " };
        match device.default_input_config() {
            Ok(c) => eprintln!(
                "  {marker} {name}  ({} Hz, {} ch, {:?})",
                c.sample_rate().0,
                c.channels(),
                c.sample_format()
            ),
            Err(e) => eprintln!("  {marker} {name}  (no default config: {e})"),
        }
    }
    Ok(())
}

/// Pick the input device: the first whose name contains `want`, or the system default.
fn select_input_device(want: &Option<String>) -> phonix::Result<cpal::Device> {
    let host = cpal::default_host();
    if let Some(want) = want {
        let devices = host
            .input_devices()
            .map_err(|e| phonix::Error::Audio(e.to_string()))?;
        for device in devices {
            if device
                .name()
                .map(|n| n.to_lowercase().contains(&want.to_lowercase()))
                .unwrap_or(false)
            {
                return Ok(device);
            }
        }
        return Err(phonix::Error::Audio(format!(
            "no input device matching {want:?} (try `phonix-listen devices`)"
        )));
    }
    host.default_input_device()
        .ok_or_else(|| phonix::Error::Audio("no default input device".into()))
}

fn base_config(cli: &Cli, sample_rate: u32, channels: u16) -> Config {
    let mut cfg = Config {
        input_sample_rate: sample_rate,
        input_channels: channels,
        wake_threshold: cli.wake_threshold,
        vad_threshold: cli.vad_threshold,
        pre_roll_ms: cli.pre_roll_ms,
        ..Config::default()
    };
    cfg.models.wake = cli.wake_model.clone();
    cfg
}

fn run_file(cli: &Cli, path: &Path) -> phonix::Result<()> {
    let mut reader = hound::WavReader::open(path).map_err(|e| phonix::Error::Wav(e.to_string()))?;
    let spec = reader.spec();
    let cfg = base_config(cli, spec.sample_rate, spec.channels);
    let mut meter = DebugMeter::new(cli.debug, spec.sample_rate);
    let mut detector = Detector::new(cfg, StdoutSink)?;

    // Normalize i16 → f32 in [-1, 1]; pass interleaved samples in blocks.
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap_or(0) as f32 / i16::MAX as f32)
        .collect();
    for block in samples.chunks(4096) {
        detector.push(block)?;
        meter.observe(block, &detector);
    }
    eprintln!("done: {}", path.display());
    Ok(())
}

fn run_mic(cli: &Cli) -> phonix::Result<()> {
    let device = select_input_device(&cli.device)?;
    let name = device.name().unwrap_or_else(|_| "<unknown>".into());
    let config = device
        .default_input_config()
        .map_err(|e| phonix::Error::Audio(e.to_string()))?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    eprintln!(
        "listening on \"{name}\": {sample_rate} Hz, {channels} ch — say the wake word (Ctrl-C to stop)"
    );
    if sample_rate == 16_000 {
        eprintln!(
            "note: 16 kHz mono usually means a Bluetooth/headset (HFP) or virtual input, which is often \
             quiet or muted. If `peak` stays low while you speak, pick your built-in mic with \
             `--device <name>` (see `phonix-listen devices`) or set it as the default in \
             System Settings ▸ Sound ▸ Input."
        );
    }

    let cfg = base_config(cli, sample_rate, channels);
    let mut meter = DebugMeter::new(cli.debug, sample_rate);
    let mut detector = Detector::new(cfg, StdoutSink)?;
    if cli.debug {
        eprintln!("[debug] enabled — peak≈0 while speaking means the mic is muted or the terminal lacks Microphone permission (macOS: System Settings ▸ Privacy & Security ▸ Microphone)");
    }

    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let err_fn = |e| eprintln!("stream error: {e}");
    let stream = device
        .build_input_stream(
            &config.into(),
            move |data: &[f32], _| {
                let _ = tx.send(data.to_vec());
            },
            err_fn,
            None,
        )
        .map_err(|e| phonix::Error::Audio(e.to_string()))?;
    stream
        .play()
        .map_err(|e| phonix::Error::Audio(e.to_string()))?;

    for block in rx {
        detector.push(&block)?;
        meter.observe(&block, &detector);
    }
    Ok(())
}

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let cli = Cli::parse();
    let result = match &cli.cmd {
        Cmd::File { path } => run_file(&cli, path),
        Cmd::Mic => run_mic(&cli),
        Cmd::Devices => list_devices(),
    };
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
