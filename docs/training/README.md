# Training the `moreni` wake-word model

OpenWakeWord's first two stages (melspectrogram + speech embedding) are shared and
speaker-invariant — they ship inside `oww-rs`. You train only the third stage: a
small classifier over 96-d embeddings. The result is a single `moreni.onnx` you
drop into `crates/phonix/models/` and select with `Config.models.wake =
Some("…/moreni.onnx".into())` — no Rust changes.

This runs in Python/Colab, separate from the Rust crate.

## 1. Environment

```bash
pip install openwakeword piper-tts numpy onnx torch
# openWakeWord training utilities: https://github.com/dscripka/openWakeWord
```

## 2. Synthetic positive dataset

Generate hundreds of "Moreni" and "Hey Moreni" utterances across many voices so the
classifier never keys on one speaker:

- Use Piper (many voices) and/or a cloud TTS (Google/Azure). Vary **gender,
  accent, speaking rate, and pitch**. Aim for 500–2000 clips.
- Augment each clip: room impulse responses (reverb), additive meeting/background
  noise, and SNR sweeps (e.g. 5–20 dB). openWakeWord ships augmentation helpers.

## 3. Negatives and hard negatives

- Use openWakeWord's standard large negative corpus (speech + noise) as the base.
- **Add a targeted confusable set**: "company", "many", "money", "moroni",
  "morning", general corporate chatter. This is what suppresses the false positives
  the vowel-ending "Moreni" is prone to. Generate these via TTS too.

## 4. Train the classifier

Follow openWakeWord's training flow (`notebooks/training_models.ipynb` /
`openwakeword.train`): it precomputes embeddings for every clip (via the shared
melspec+embedding models) and fits the classification head. Typical settings:

- Sequence length: keep the model's default (16 embeddings for short phrases).
- Train/val split with the confusables in the validation set so you can watch the
  false-positive rate directly.
- Iterate on dataset size, augmentation strength, and epochs — these are tuning
  parameters, not fixed values. Stop when val recall is high AND confusable
  false-positives are ~0.

## 5. Export and wire in

The training flow exports an ONNX classifier. Rename it `moreni.onnx`:

```bash
cp moreni_classifier.onnx crates/phonix/models/moreni.onnx
```

Point the detector at it:

```rust
let mut cfg = Config::default();
cfg.models.wake = Some("crates/phonix/models/moreni.onnx".into());
```

## 6. Validate against the regression harness

Record (or TTS-generate) held-out clips into `crates/phonix/tests/fixtures/`:

- `positive/` — "Moreni" / "Hey Moreni" said by voices NOT in training.
- `negative/` — meeting audio + the confusable words.

Then:

```bash
cd crates/phonix && cargo test --test regression
```

`positives_have_recall` and `negatives_have_no_false_positives` are your acceptance
gate. Tune `Config.wake_threshold` to trade recall against false positives; raise it
if confusables slip through, lower it if real triggers are missed.
