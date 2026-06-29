//! Parsing of Recall.ai real-time audio messages into PCM frames for phonix.
//!
//! This is the only place that knows Recall's wire shape, kept pure (no I/O) so
//! it is unit-testable without a live Recall connection.

use base64::Engine as _;
use serde::Deserialize;

/// i16 → f32 scale. Dividing by 32768 keeps the full negative range within [-1, 1].
const I16_SCALE: f32 = 32_768.0;

/// One decoded audio chunk: which participant it came from, and the 16 kHz mono
/// f32 samples (already converted from Recall's s16le PCM).
#[derive(Debug, PartialEq)]
pub struct ParsedAudio {
    pub participant_key: String,
    pub frames: Vec<f32>,
}

// Recall real-time message envelope. Verify against your bot's `recording_config`
// — the raw-audio events nest the base64 buffer under `data.data.buffer`:
//
// { "event": "audio_separate_raw.data",
//   "data": { "data": { "buffer": "<base64 s16le pcm>" },
//             "participant": { "id": 123, "name": "Alice" } } }
//
// Mixed audio (`audio_mixed_raw.data`) omits `participant`.
#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    event: String,
    data: Payload,
}

#[derive(Deserialize)]
struct Payload {
    data: Inner,
    #[serde(default)]
    participant: Option<Participant>,
}

#[derive(Deserialize)]
struct Inner {
    #[serde(default)]
    buffer: String,
}

#[derive(Deserialize)]
struct Participant {
    #[serde(default)]
    id: Option<serde_json::Value>,
    #[serde(default)]
    name: Option<String>,
}

/// Convert little-endian signed-16 PCM bytes to f32 samples in [-1, 1].
pub fn pcm_s16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / I16_SCALE)
        .collect()
}

/// Parse a Recall real-time WebSocket text message.
///
/// Returns `None` for non-audio events (transcripts, status, etc.) and for
/// unparseable or empty payloads, so callers can simply skip them.
pub fn parse_audio_message(text: &str) -> Option<ParsedAudio> {
    let env: Envelope = serde_json::from_str(text).ok()?;

    // Only the raw-audio events carry PCM; ignore everything else.
    if !env.event.contains("audio") || !env.event.contains("raw") {
        return None;
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(env.data.data.buffer.as_bytes())
        .ok()?;
    let frames = pcm_s16le_to_f32(&bytes);
    if frames.is_empty() {
        return None;
    }

    // Per-participant streams carry an identity; the mixed stream does not.
    let participant_key = env
        .data
        .participant
        .and_then(|p| {
            p.id.map(|v| match v {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            })
            .or(p.name)
        })
        .unwrap_or_else(|| "mixed".to_string());

    Some(ParsedAudio {
        participant_key,
        frames,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn decodes_s16le_to_unit_range() {
        // 0 → 0.0 ; 32767 → ~1.0 ; -32768 → -1.0
        let f = pcm_s16le_to_f32(&[0x00, 0x00, 0xff, 0x7f, 0x00, 0x80]);
        assert_eq!(f.len(), 3);
        assert!(f[0].abs() < 1e-6);
        assert!((f[1] - 0.99997).abs() < 1e-3);
        assert!((f[2] + 1.0).abs() < 1e-6);
    }

    #[test]
    fn odd_trailing_byte_is_ignored() {
        // 3 bytes → one full sample, trailing byte dropped (chunks_exact).
        assert_eq!(pcm_s16le_to_f32(&[0x00, 0x00, 0x10]).len(), 1);
    }

    #[test]
    fn parses_separate_audio_with_participant_id() {
        let msg = format!(
            r#"{{"event":"audio_separate_raw.data","data":{{"data":{{"buffer":"{}"}},"participant":{{"id":42,"name":"Alice"}}}}}}"#,
            b64(&[0, 0, 0, 0])
        );
        let p = parse_audio_message(&msg).unwrap();
        assert_eq!(p.participant_key, "42");
        assert_eq!(p.frames.len(), 2);
    }

    #[test]
    fn falls_back_to_name_when_id_absent() {
        let msg = format!(
            r#"{{"event":"audio_separate_raw.data","data":{{"data":{{"buffer":"{}"}},"participant":{{"name":"Bob"}}}}}}"#,
            b64(&[0, 0])
        );
        assert_eq!(parse_audio_message(&msg).unwrap().participant_key, "Bob");
    }

    #[test]
    fn parses_mixed_audio_without_participant() {
        let msg = format!(
            r#"{{"event":"audio_mixed_raw.data","data":{{"data":{{"buffer":"{}"}}}}}}"#,
            b64(&[0, 0])
        );
        let p = parse_audio_message(&msg).unwrap();
        assert_eq!(p.participant_key, "mixed");
        assert_eq!(p.frames.len(), 1);
    }

    #[test]
    fn ignores_non_audio_events() {
        let msg = format!(
            r#"{{"event":"transcript.data","data":{{"data":{{"buffer":"{}"}}}}}}"#,
            b64(&[0, 0])
        );
        assert!(parse_audio_message(&msg).is_none());
    }

    #[test]
    fn ignores_garbage() {
        assert!(parse_audio_message("not json").is_none());
        assert!(parse_audio_message("{}").is_none());
    }
}
