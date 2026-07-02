# CI Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a GitHub Actions CI pipeline for the orchard Rust R REPL project with four parallel jobs: fmt, clippy, test (lib + magic framework), and test-r (embedded R integration).

**Architecture:** Single `.github/workflows/ci.yml` defining four concurrent jobs using `Swatinem/rust-cache@v2` for dependency caching and `dtolnay/rust-toolchain@stable` for toolchain setup. The `test-r` job installs `r-base` via apt for R-dependent integration tests.

**Tech Stack:** GitHub Actions, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, apt `r-base` package.

## Global Constraints

- Rust edition `2024` (stable toolchain)
- vendored `reedline` at `vendor/reedline/` — no special CI handling needed
- `bindgen` + `cc` build dependencies need libclang-dev and C compiler (pre-installed on `ubuntu-latest`)
- `#![deny(unsafe_op_in_unsafe_fn)]` enforced
- No `unwrap()` in production code without safety-rationale comments

---

### Task 1: Create CI Workflow File

**Files:**
- Create: `.github/workflows/ci.yml`

**Interfaces:**
- Consumes: Nothing (first and only task)
- Produces: `.github/workflows/ci.yml` — the complete CI pipeline

**Pre-flight check:** Code is already clean: `cargo check` passes, `cargo clippy -- -D warnings` produces no warnings, `cargo fmt --check` produces no output, 265 lib tests + 7 magic framework tests pass.

- [ ] **Step 1: Create `.github/workflows/ci.yml`**

Write the multi-job workflow with four parallel jobs:

```yaml
name: CI

on:
  push:
    branches: ["**"]
  pull_request:
    branches: ["**"]
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always

jobs:
  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --check

  clippy:
    name: Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo clippy -- -D warnings

  test:
    name: Test (lib + magic framework)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --lib
      - run: cargo test --test magic_framework

  test-r:
    name: Test (embedded R)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Install R
        run: |
          sudo apt-get update
          sudo apt-get install -y --no-install-recommends r-base
      - name: Run R-gated integration tests
        run: ORCHARD_TEST_R=1 cargo test --test embedded_r -- --test-threads=1 --nocapture
```

- [ ] **Step 2: Verify the workflow file is syntactically valid**

```bash
# GitHub Actions YAML is standard YAML — verify with a parser
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('YAML OK')"
```

- [ ] **Step 3: Commit the workflow file**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add GitHub Actions pipeline with four parallel jobs

- fmt: cargo fmt --check
- clippy: cargo clippy -- -D warnings
- test: cargo test --lib + magic_framework
- test-r: install R via apt, embedded R integration tests

Part of v0.3 Quick Wins + CI."
```
