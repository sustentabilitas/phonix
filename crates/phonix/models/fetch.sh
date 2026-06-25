#!/usr/bin/env bash
# Downloads the Silero VAD ONNX model. The OpenWakeWord models (melspectrogram,
# embedding, hey_jarvis) are bundled inside the `oww-rs` crate and need no download.
set -euo pipefail
cd "$(dirname "$0")"

VAD_URL="https://raw.githubusercontent.com/snakers4/silero-vad/master/src/silero_vad/data/silero_vad.onnx"

echo "Downloading silero_vad.onnx ..."
curl -fL -o silero_vad.onnx "$VAD_URL"

echo "Verifying checksums ..."
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c checksums.txt
else
  shasum -a 256 -c checksums.txt
fi
echo "Done."
