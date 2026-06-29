# phonix-recall — pure-Linux container image for GKE.
#
# Build context is the WORKSPACE ROOT, and the Silero model must be present in it:
#   ./crates/phonix/models/fetch.sh
#   docker build -f crates/phonix-recall/Dockerfile -t phonix-recall:dev .

FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p phonix-recall

FROM debian:bookworm-slim AS runtime
RUN useradd --system --create-home app
WORKDIR /app
COPY --from=build /src/target/release/phonix-recall /usr/local/bin/phonix-recall
# Silero VAD model. The three OpenWakeWord models are embedded in the binary
# (via oww-rs), so only this file is needed at runtime. It must exist in the
# build context — run crates/phonix/models/fetch.sh before `docker build`.
COPY crates/phonix/models/silero_vad.onnx /app/models/silero_vad.onnx
ENV PHONIX_VAD_MODEL=/app/models/silero_vad.onnx \
    PORT=8080 \
    RUST_LOG=info
USER app
EXPOSE 8080
ENTRYPOINT ["phonix-recall"]
