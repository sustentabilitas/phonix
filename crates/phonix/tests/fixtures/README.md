# Regression fixtures

Drop 16 kHz (any rate works; it is resampled) mono/stereo 16-bit WAV clips here:

- `positive/` — clips that SAY the wake word ("Hey Jarvis" now; "Moreni" after
  training). Each must produce at least one detection. Measures **recall**.
- `negative/` — conversational/corporate audio that must NOT trigger, including
  confusables ("company", "many", "money"). Measures **false positives**.

Generate negatives quickly from any meeting recording; generate positives from the
training TTS pipeline (`docs/training/`) or real recordings.
