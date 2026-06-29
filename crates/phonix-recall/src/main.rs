//! phonix-recall — a pure-Linux WebSocket service that feeds Recall.ai's
//! real-time meeting audio into the phonix wake-word detector.
//!
//! Recall connects to `/ws` and streams base64 s16le PCM; we decode it, run one
//! [`Detector`] per participant, and log detections. Swap [`LogSink`] for a real
//! `StreamSink` (e.g. a Gemini Live client) once detection is verified live.
//!
//! Configuration (env):
//!   PORT                  listen port (default 8080)
//!   PHONIX_VAD_MODEL      path to silero_vad.onnx (default "models/silero_vad.onnx")
//!   PHONIX_WAKE_MODEL     optional custom wake model (default: bundled hey_jarvis)
//!   PHONIX_SAMPLE_RATE    Recall PCM sample rate (default 16000)
//!   PHONIX_WAKE_THRESHOLD wake score threshold (default 0.5)

mod recall;

use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use phonix::{Config, Detector, StreamSink, WakeEvent};
use tracing::{error, info, warn};

struct AppState {
    cfg: Config,
}

/// A `StreamSink` that logs detections with participant attribution. The
/// integration point: replace this with a client that forwards `pre_roll` then
/// live audio to your LLM.
struct LogSink {
    participant: String,
    streamed: usize,
}

impl LogSink {
    fn new(participant: &str) -> Self {
        Self {
            participant: participant.to_string(),
            streamed: 0,
        }
    }
}

impl StreamSink for LogSink {
    fn on_wake(&mut self, e: &WakeEvent) {
        self.streamed = 0;
        info!(
            participant = %self.participant,
            model = %e.model,
            score = e.score,
            pre_roll_ms = e.pre_roll.len() / 16,
            "wake detected"
        );
    }

    fn on_audio(&mut self, frames: &[f32]) {
        self.streamed += frames.len();
    }

    fn on_end(&mut self) {
        info!(
            participant = %self.participant,
            streamed_ms = self.streamed / 16,
            "utterance complete"
        );
    }
}

fn make_detector(participant: &str, base: &Config) -> phonix::Result<Detector<LogSink>> {
    let mut d = Detector::new(base.clone(), LogSink::new(participant))?;
    d.set_stream_id(Some(participant.to_string()));
    Ok(d)
}

async fn ws_handler(State(state): State<Arc<AppState>>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// One Recall connection. Owns its per-participant detector map (single task, no
/// shared mutable state), so concurrency stays trivial.
async fn handle_socket(mut socket: WebSocket, state: Arc<AppState>) {
    info!("recall websocket connected");
    let mut detectors: HashMap<String, Detector<LogSink>> = HashMap::new();

    while let Some(Ok(msg)) = socket.recv().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        let Some(parsed) = recall::parse_audio_message(text.as_str()) else {
            continue;
        };

        let det = match detectors.entry(parsed.participant_key.clone()) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => match make_detector(&parsed.participant_key, &state.cfg) {
                Ok(d) => e.insert(d),
                Err(err) => {
                    error!(participant = %parsed.participant_key, %err, "failed to build detector");
                    continue;
                }
            },
        };

        if let Err(err) = det.push(&parsed.frames) {
            warn!(participant = %parsed.participant_key, %err, "detector push failed");
        }
    }

    info!(streams = detectors.len(), "recall websocket closed");
}

fn config_from_env() -> Config {
    let mut cfg = Config {
        input_sample_rate: env_parse("PHONIX_SAMPLE_RATE", 16_000),
        input_channels: 1,
        wake_threshold: env_parse("PHONIX_WAKE_THRESHOLD", 0.5_f32),
        ..Config::default()
    };
    cfg.models.vad = std::env::var("PHONIX_VAD_MODEL")
        .unwrap_or_else(|_| "models/silero_vad.onnx".to_string())
        .into();
    cfg.models.wake = std::env::var("PHONIX_WAKE_MODEL").ok().map(Into::into);
    cfg
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg = config_from_env();

    // Fail fast (and clearly) if the models can't load, rather than per-connection.
    if let Err(e) = Detector::new(cfg.clone(), LogSink::new("_preflight")) {
        anyhow::bail!("failed to load models (check PHONIX_VAD_MODEL): {e}");
    }
    info!(
        sample_rate = cfg.input_sample_rate,
        wake_threshold = cfg.wake_threshold,
        wake_model = ?cfg.models.wake,
        "phonix-recall ready"
    );

    let state = Arc::new(AppState { cfg });
    let app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let port: u16 = env_parse("PORT", 8080);
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "listening — Recall → /ws, health → /healthz");
    axum::serve(listener, app).await?;
    Ok(())
}
