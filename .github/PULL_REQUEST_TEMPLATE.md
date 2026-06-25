## Summary

<!-- What does this PR do and why? Reference design decisions or tradeoffs where helpful. -->

## Related issue

Closes #

## Test plan

<!-- Which `cargo test` invocations, feature builds, or manual steps were run to verify this change? -->

- [ ] `cargo test` (with `./crates/phonix/models/fetch.sh` run first so model-gated tests execute)
- [ ] `cargo build --features cli`
- [ ] Manual steps: <!-- e.g. `phonix-listen mic --debug`, or a file-mode run on a sample clip -->

## Checklist

- [ ] `cargo fmt --all` — no formatting changes needed
- [ ] `cargo clippy --all-targets --features cli -- -D warnings` — no warnings
- [ ] `cargo test` passes
- [ ] Default (lib-only) build still works: `cargo build`
- [ ] No C/C++ inference dependencies added (`tract`/`oww-rs` only)
- [ ] Docs and/or `CHANGELOG.md` (`[Unreleased]` section) updated if applicable
- [ ] All commits signed off (DCO: `git commit -s`)
