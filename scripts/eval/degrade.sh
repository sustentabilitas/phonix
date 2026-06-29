#!/usr/bin/env bash
# Simulate a Teams/Opus meeting leg over a clean WAV corpus, so phonix-eval can
# measure detection on realistic (codec-degraded, optionally multi-speaker) audio.
#
# Usage:
#   scripts/eval/degrade.sh SRC_DIR OUT_DIR [--bitrate 24k] [--babble FILE.wav] [--babble-gain -10dB]
#
# For each *.wav in SRC_DIR:
#   (optional) overlay a looped babble / background-speaker track at --babble-gain,
#   then encode→decode through Opus at --bitrate, producing a 16 kHz mono s16 WAV
#   in OUT_DIR (the format phonix consumes). Typical Teams Opus bitrates are ~24k;
#   drop to 12k to stress-test.
set -euo pipefail

[ $# -ge 2 ] || {
  echo "usage: degrade.sh SRC_DIR OUT_DIR [--bitrate 24k] [--babble FILE] [--babble-gain -10dB]" >&2
  exit 2
}
src=$1
out=$2
shift 2
bitrate=24k
babble=""
gain="-10dB"
while [ $# -gt 0 ]; do
  case "$1" in
    --bitrate) bitrate=$2; shift 2 ;;
    --babble) babble=$2; shift 2 ;;
    --babble-gain) gain=$2; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

command -v ffmpeg >/dev/null || { echo "ffmpeg not found" >&2; exit 1; }
mkdir -p "$out"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

shopt -s nullglob
clips=("$src"/*.wav)
[ ${#clips[@]} -gt 0 ] || { echo "no .wav files in $src" >&2; exit 1; }

for f in "${clips[@]}"; do
  name=$(basename "$f")
  stage="$tmp/stage.wav"
  if [ -n "$babble" ]; then
    # Loop the babble track and overlay it at the requested gain, matched to clip length.
    ffmpeg -hide_banner -loglevel error -y -i "$f" -stream_loop -1 -i "$babble" \
      -filter_complex "[1:a]volume=${gain}[b];[0:a][b]amix=inputs=2:duration=first:normalize=0[a]" \
      -map "[a]" -ar 48000 -ac 1 "$stage"
  else
    ffmpeg -hide_banner -loglevel error -y -i "$f" -ar 48000 -ac 1 "$stage"
  fi
  # Opus encode → decode, then back to 16 kHz mono s16 (what phonix ingests).
  ffmpeg -hide_banner -loglevel error -y -i "$stage" \
    -c:a libopus -b:a "$bitrate" -ar 48000 -ac 1 -f ogg "$tmp/clip.opus"
  ffmpeg -hide_banner -loglevel error -y -i "$tmp/clip.opus" \
    -ar 16000 -ac 1 -c:a pcm_s16le "$out/$name"
done

count=$(find "$out" -maxdepth 1 -name '*.wav' | wc -l | tr -d ' ')
echo "wrote $count clips to $out (bitrate=$bitrate${babble:+, babble=$babble@$gain})"
