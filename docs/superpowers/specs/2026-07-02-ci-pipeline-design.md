# CI Pipeline Design

**Date:** 2026-07-02
**Status:** Approved (pre-implementation)
**Version:** v0.3 — Quick Wins + CI

## Overview

A GitHub Actions continuous integration pipeline for the orchard Rust R REPL
project. Provides automated quality checking and testing on every code push
and pull request.

## Provider & Platform

- **Provider:** GitHub Actions
- **Runner:** `ubuntu-latest` (GitHub-hosted)
- **Platform target:** Linux (macOS pipeline planned separately for v0.8)

## Triggers

- `push` to any branch
- `pull_request` targeting any branch
- `workflow_dispatch` for manual ad-hoc runs

Concurrency cancels in-progress runs on the same branch for non-PR pushes
(e.g., a force-push cancels the previous run on that branch).

## Architecture: Multi-Job Parallel

Four independent jobs run concurrently, each with its own `Swatinem/rust-cache`:

```
┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐
│   fmt   │  │ clippy  │  │  test   │  │ test-r  │
│ ─────── │  │ ─────── │  │ ─────── │  │ ─────── │
│ fmt     │  │ clippy  │  │ lib     │  │ install │
│ --check  │  │ -Dwarn  │  │ tests   │  │ R (apt) │
│         │  │         │  │ magic   │  │ embed-  │
│         │  │         │  │ frame.  │  │ ded R   │
│         │  │         │  │         │  │ tests   │
└─────────┘  └─────────┘  └─────────┘  └─────────┘
   ~30s        ~3min       ~5min        ~8min
```

No inter-job dependencies — all jobs start immediately and run in parallel.
All four must pass for a green overall status.

## Job Definitions

### `fmt`

| Field | Value |
|-------|-------|
| Steps | `checkout` → `rust-toolchain@stable` (fmt component) → `cargo fmt --check` |
| Caching | Not needed (fmt only processes source files) |
| Expected | ~30s |
| Failure | Blocks PR — formatting must be fixed |

### `clippy`

| Field | Value |
|-------|-------|
| Steps | `checkout` → `rust-toolchain@stable` (clippy component) → `Swatinem/rust-cache@v2` → `cargo clippy -- -D warnings` |
| Caching | Yes — caches `~/.cargo` and `target/` |
| Expected | ~2-3 min (with cache) |
| Failure | Blocks PR — all clippy warnings must be resolved (`-D warnings`) |

### `test` (lib + magic framework)

| Field | Value |
|-------|-------|
| Steps | `checkout` → `rust-toolchain@stable` → `Swatinem/rust-cache@v2` → `cargo test --lib` → `cargo test --test magic_framework` |
| Caching | Yes |
| Expected | ~3-5 min (with cache) |
| Notes | No external dependencies needed; pure Rust unit tests |

### `test-r` (embedded R integration)

| Field | Value |
|-------|-------|
| Steps | `checkout` → `rust-toolchain@stable` → `Swatinem/rust-cache@v2` → Install `r-base` via apt → `ORCHARD_TEST_R=1 cargo test --test embedded_r -- --test-threads=1 --nocapture` |
| Caching | Yes |
| Expected | ~5-8 min (includes apt install R) |
| Notes | `--test-threads=1` because tests spawn real R subprocesses |

## System Dependencies

### Required for all jobs (pre-installed on `ubuntu-latest`)

- `libclang-dev` — needed by `bindgen` (build dependency)
- C compiler toolchain — needed by `cc` (build dependency)
- Rust stable toolchain via `dtolnay/rust-toolchain`

### Required for `test-r` only

- `r-base` — R statistical computing runtime, installed via apt:
  ```yaml
  - name: Install R
    run: |
      sudo apt-get update
      sudo apt-get install -y --no-install-recommends r-base
  ```

## Vendored Dependencies

The project vendors `reedline` at `vendor/reedline/`. Cargo resolves path
dependencies from the local filesystem — no special handling needed in CI.

## Error Handling

- All jobs are **required** (no `continue-on-error`)
- A failure in any job yields a red status on the PR
- `fmt` and `clippy` failures must be fixed before merge
- Test failures block merge until resolved

## Caching Strategy

`Swatinem/rust-cache@v2` with default settings:
- Keyed by `Cargo.lock` hash
- Caches `~/.cargo` registry, git db, and `target/` directory
- Each job has its own cache (separate `target/` dirs per job)
- First run on a new branch is uncached (~5-10 min); subsequent runs
  restore in ~30s

## Edge Cases

| Scenario | Behavior |
|----------|----------|
| Branch push without PR | Full pipeline runs, all 4 jobs |
| PR from fork | Pipeline runs (standard GitHub Actions behavior) |
| Dependabot/renovate PR | Pipeline runs normally |
| Force-push | Previous run on branch is cancelled (concurrency) |
| R install failure | `test-r` job fails; other 3 jobs unaffected |
| Empty commit (docs only) | Pipeline still runs (no path filtering) — currently acceptable |

## Future Extensions (v0.8+)

- macOS CI runner (`macos-latest`)
- Release-on-tag workflow (`cargo build --release`, GitHub Release creation)
- `cargo deny` / `cargo audit` for dependency security scanning
- Code coverage reporting (tarpaulin or nextest + grcov)
- Path filtering to skip CI on pure-documentation changes

---

## Implementation Plan

1. Create `.github/workflows/ci.yml` with all four jobs
2. Run `cargo check` locally to confirm no pre-existing issues
3. Run `cargo clippy -- -D warnings` locally
4. Run `cargo fmt --check` locally
5. Commit the workflow file
