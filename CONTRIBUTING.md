# Contributing to phonix

Thank you for your interest in contributing to phonix! This is an open-source,
AGPL-3.0-licensed project and we welcome issues, bug reports, feature requests, and pull
requests from the community.

Before you begin, please read our [Code of Conduct](CODE_OF_CONDUCT.md) and
[Security Policy](SECURITY.md). By participating you agree to abide by the Code of Conduct.

---

## Table of Contents

1. [Getting started](#getting-started)
2. [Before you submit](#before-you-submit)
3. [Development workflow](#development-workflow)
4. [Commit messages](#commit-messages)
5. [Developer Certificate of Origin (DCO)](#developer-certificate-of-origin-dco)
6. [Pull requests](#pull-requests)
7. [Releasing](#releasing)
8. [License](#license)

---

## Getting started

### Prerequisites

- [Rust stable toolchain](https://rustup.rs/) (the current stable release)
- `cargo fmt` and `cargo clippy` (both ship with `rustup`)
- **On Linux:** the ALSA dev headers — `oww-rs` hard-depends on `cpal`, so the build
  needs them even though the core opens no audio device:
  `sudo apt-get install -y libasound2-dev pkg-config` (macOS needs nothing — CoreAudio)

### Clone and build

```bash
git clone https://github.com/sustentabilitas/phonix.git
cd phonix

# The Silero VAD model is needed for the model-gated tests
# (the OpenWakeWord models ship bundled inside oww-rs).
./crates/phonix/models/fetch.sh

# Default build (the pure, I/O-free detection library)
cargo build

# Run the test suite
cargo test
```

### Feature matrix

| Command | What it enables |
|---|---|
| `cargo build` | Default features: the sync, I/O-free detection library |
| `cargo build --features cli` | The `phonix-listen` binary (`cpal` mic + `hound` file modes) |

The core library is **sync and I/O-free** — `cpal`/`hound`/`clap`/`tracing` must stay behind
the `cli` feature, and no neural inference may use anything other than `tract`/`oww-rs`
(**no `ort`, `onnxruntime`, `webrtc-vad`, or other C/C++ dependencies**).

---

## Before you submit

Run all of the following locally before opening a pull request. CI enforces each gate and
a red build blocks merging.

```bash
# 1. Format check (must produce no diff)
cargo fmt --all --check

# 2. Lint (zero warnings, warnings treated as errors)
cargo clippy --all-targets --features cli -- -D warnings

# 3. Test suite (fetch the model first so the model-gated tests run)
./crates/phonix/models/fetch.sh
cargo test

# 4. Confirm the lean default library still builds without the cli deps
cargo build
```

Also:

- **Update `CHANGELOG.md`**: add a line under the `## [Unreleased]` section describing your
  change under the appropriate heading (`Added`, `Changed`, `Fixed`, `Removed`, `Security`).
- **Update documentation**: if your change affects the public API, configuration, or CLI
  flags, update the relevant docs (README, files under `docs/`, or inline rustdoc).

---

## Development workflow

Non-trivial contributions — new features, significant refactors, changes to the public
library API or the detection pipeline — should follow the **spec → plan → implement** flow
used in this project.

1. **Write a spec** under `docs/superpowers/specs/`. Describe *what* and *why*: the problem,
   proposed behaviour, edge cases, and acceptance criteria. Keep it concise.

2. **Write a plan** under `docs/superpowers/plans/`. Break the work into small, reviewable,
   test-driven steps. The plan references the spec.

3. **Implement with TDD**: write the failing test first (in `tests/` or a `#[cfg(test)]`
   module in the relevant source file), then write the minimal implementation that makes it
   pass, then refactor. Where a model is involved, prefer trait-injected fakes so the logic
   is testable without a real `.onnx` file.

4. **Open a PR** that links to the spec and plan documents so reviewers have full context.

Small bug fixes and documentation improvements do not need a full spec; use your judgement.

---

## Commit messages

- Use an **imperative, present-tense subject line** (e.g. `add device selection`, not
  `added` or `adding`).
- Keep the subject under **72 characters**.
- Use conventional-commit prefixes when applicable:

  | Prefix | Use for |
  |---|---|
  | `feat:` | New feature or behaviour |
  | `fix:` | Bug fix |
  | `docs:` | Documentation changes only |
  | `refactor:` | Code restructuring without behaviour change |
  | `test:` | Adding or updating tests |
  | `chore:` | Maintenance, dependency updates, tooling |
  | `perf:` | Performance improvements |
  | `ci:` | CI/CD pipeline changes |

- Optionally include a body (blank line after subject) explaining *why* the change was made.
- Reference related issues or PRs at the bottom: `Closes #42`.

---

## Developer Certificate of Origin (DCO)

**Every commit must carry a `Signed-off-by` trailer.**

By signing off you certify that you have the right to submit the contribution under the
project's AGPL-3.0 license, as defined by the Developer Certificate of Origin at
<https://developercertificate.org/>.

Add the sign-off automatically with the `-s` flag:

```bash
git commit -s -m "feat: your change description"
```

> **Pull requests that contain unsigned commits will not be merged.** If you forget to sign
> off on earlier commits, you can amend them:
>
> ```bash
> git commit --amend -s --no-edit          # most recent commit
> git rebase --signoff HEAD~<N>            # all commits in the branch
> ```

---

## Pull requests

1. **Fork** the repository and create a feature branch from `main`.
2. Follow the [development workflow](#development-workflow) and
   [commit guidelines](#commit-messages).
3. Ensure **all CI checks pass** before requesting review.
4. Open a pull request with a clear title, a description of *what* and *why*, links to any
   spec/plan docs, and `Closes #<issue>` if applicable.
5. Address reviewer feedback promptly. One approving review from a maintainer is required
   to merge.

---

## Releasing

Releases are tag-driven. Maintainers only:

1. Bump `version` in `crates/phonix/Cargo.toml`.
2. Move the `CHANGELOG.md` `[Unreleased]` entries under a new `[X.Y.Z]` heading (with the date).
3. Commit, then tag and push:
   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

Pushing the `vX.Y.Z` tag triggers [`.github/workflows/release.yml`](.github/workflows/release.yml),
which verifies the tag matches `Cargo.toml`'s version, `cargo publish`es the `phonix` crate
to **crates.io**, and builds + pushes the `phonix-recall` Docker image to **Docker Hub** as
`sustentabilitas/phonix-recall:X.Y.Z` and `:latest`. Every push to `main` also publishes a
rolling `sustentabilitas/phonix-recall:edge` image (no crate publish).

Required repository secrets (Settings → Secrets and variables → Actions):
`CARGO_REGISTRY_TOKEN`, `DOCKERHUB_USERNAME`, `DOCKERHUB_TOKEN`.

---

## License

By submitting a contribution you agree that your work is licensed under the
[GNU Affero General Public License v3.0](LICENSE) (AGPL-3.0), the same license as the rest
of the project.

If you have any questions, feel free to open a discussion or reach out via the contact in
[SECURITY.md](SECURITY.md).
