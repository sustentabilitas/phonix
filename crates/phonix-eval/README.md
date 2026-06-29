# phonix-eval

Offline **recall / false-positive** evaluation for the phonix wake-word detector on
realistic, **codec-degraded, multi-speaker** audio — the de-risking step that tells
you whether detection is good enough before deploying into live meetings.

It runs your corpus through the real `Detector` across a **threshold sweep** and
reports, per threshold: detection rate over positives, how many negatives falsely
fired, and the **false-positives-per-hour** rate (the number that actually matters in
a long meeting).

## Workflow

```bash
# 1. Assemble a clean corpus (your own recordings, or TTS from docs/training/):
#      eval/clean/positive/*.wav   ← clips that SAY the wake word
#      eval/clean/negative/*.wav   ← conversation, confusables (company/many/money), noise

# 2. Degrade it to meeting conditions (Opus codec + optional background speaker):
./scripts/eval/degrade.sh eval/clean/positive eval/opus24k/positive --bitrate 24k
./scripts/eval/degrade.sh eval/clean/negative eval/opus24k/negative --bitrate 24k \
    --babble eval/clean/negative/meeting_chatter.wav --babble-gain -8dB

# 3. Score it:
cargo run -p phonix-eval -- \
    --positive eval/opus24k/positive --negative eval/opus24k/negative \
    --thresholds 0.3,0.5,0.7
```

Example output:

```
threshold   recall (pos)     neg clips fired   FP events   FP/hour
---------   --------------   ---------------   ---------   -------
0.30         2/2 (100.0%)     1/4                   1        ...
0.50         2/2 (100.0%)     0/4                   0        0.0
0.70         1/2 ( 50.0%)     0/4                   0        0.0
```

Compare the same corpus **clean vs `--bitrate 24k` vs `24k + babble`** to see exactly
how much the codec and overlapping speech cost you, and pick the `wake_threshold` that
holds recall while driving FP/hour to near zero. That threshold goes straight into
`Config.wake_threshold` / `PHONIX_WAKE_THRESHOLD`.

## Notes

- Requires `ffmpeg` (with `libopus`) for `degrade.sh`; the scorer itself is pure Rust.
- A handful of clips gives a rough read; for a production go/no-go, use **dozens+** of
  positives across voices/accents and a long negative reel (ideally real meeting audio
  containing the confusables) so FP/hour is meaningful.
- The corpus is yours — don't commit third-party or recorded audio you can't redistribute.
