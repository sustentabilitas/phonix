//! Contract for feeding a meeting platform's PCM into phonix.
//!
//! Teams (Graph/ACS real-time-media), Zoom (Raw Data / RTMS), and Google Meet
//! (Meet Media API) all ultimately hand you raw PCM frames. Decode/receive them in
//! your transport layer, then push them into a [`Detector`] configured for that
//! stream's sample rate and channel count. Run one detector per participant stream
//! (set a `stream_id`) to attribute who triggered, or one over the mixed stream.
//!
//! Run with: `cargo run -p phonix --example meeting_adapter`

use phonix::{Config, Detector, StdoutSink};

fn main() -> phonix::Result<()> {
    // Example: a Zoom raw-audio stream at 32 kHz stereo.
    let cfg = Config {
        input_sample_rate: 32_000,
        input_channels: 2,
        ..Config::default()
    };
    let mut detector = Detector::new(cfg, StdoutSink)?;
    detector.set_stream_id(Some("participant-42".into()));

    // In a real adapter, this loop is driven by the platform's audio callback or
    // websocket. Here we synthesize 2 seconds of silence to show the call shape.
    let frame = vec![0.0f32; 32_000 * 2 / 50]; // 20 ms of 32 kHz stereo interleaved
    for _ in 0..100 {
        detector.push(&frame)?;
    }
    eprintln!("fed 2s; wakes so far: (see [wake] lines above, if any)");
    Ok(())
}
