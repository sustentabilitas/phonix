# Security Policy

## Supported versions

phonix is pre-1.0; only the latest `0.1.x` line receives security fixes.

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅        |
| < 0.1   | ❌        |

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues, discussions, or pull requests.**

Instead, report them privately via either:

- **GitHub private vulnerability advisories** — open a draft advisory under the repository's **Security → Advisories** tab (preferred; keeps the report and fix coordinated in one place), or
- **Email** — [raj@sustentabilitas.com](mailto:raj@sustentabilitas.com).

Please include, as far as you can:

- the affected version / commit,
- a description of the issue and its impact,
- reproduction steps or a proof of concept,
- any relevant configuration (sample rate / channels, model paths, thresholds) and logs (run the CLI with `--debug`, and redact anything sensitive).

## What to expect

- **Acknowledgement** within **3 business days**.
- An initial assessment and severity triage, and we will keep you updated on progress toward a fix.
- We follow **coordinated disclosure**: we ask that you give us a reasonable window to release a fix before any public disclosure, and we will credit reporters who wish to be acknowledged.

## Scope

This policy covers the `phonix` crate and its `phonix-listen` binary. Vulnerabilities in upstream dependencies (e.g. `tract`, `oww-rs`, `cpal`) should be reported to the respective projects; if a dependency issue affects phonix, we are happy to coordinate.
