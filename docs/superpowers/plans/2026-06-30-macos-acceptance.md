# macOS Acceptance Plan

**Date:** 2026-06-30
**Status:** Plan — not yet executed (requires physical Mac hardware)

## Goal

Validate that `radian-rs` builds, runs, and passes its test suite on macOS
with both Homebrew R and CRAN R installations.

## Prerequisites

- macOS 12+ (Monterey or later)
- R installed via Homebrew (`brew install r`) or CRAN `.pkg` from CRAN
- Rust toolchain (`rustup`)

## Test Checklist

### 1. Build

- [ ] `cargo build --release` succeeds
- [ ] No macOS-specific warnings or linker errors

### 2. R Discovery

- [ ] `./target/release/radian-rs --version` shows R home path
- [ ] Works with Homebrew R (`/usr/local/bin/R` or `/opt/homebrew/bin/R`)
- [ ] Works with CRAN R (`/Library/Frameworks/R.framework/Resources`)

### 3. REPL Smoke Test

- [ ] Basic R expressions evaluate (`1 + 1` → `[1] 2`)
- [ ] Multiline input works (`{\n 1 + 1\n}`)
- [ ] Ctrl-C interrupts long-running R (`Sys.sleep(10)`)
- [ ] `;` shell mode works (`;echo hello`)

### 4. Loader Paths

- [ ] `dyld` path repair works (verify with `DYDD_PRINT_LIBRARIES=1`)
- [ ] BLAS detection finds Accelerate framework or R-supplied BLAS

### 5. Test Suite

- [ ] `cargo test` — all 159 unit tests pass
- [ ] `RADIAN_RS_TEST_R=1 cargo test --test embedded_r` — all 6 tests pass

### 6. LaTeX Completions

- [ ] Type `\alpha` + Tab → inserts `α`

### 7. Autosuggest

- [ ] Set `options(radian.auto_suggest = TRUE)` in `.radian_profile`
- [ ] Type a partial previous command → grayed hint appears

### 8. Custom Keybindings

- [ ] Set `options(radian.ctrl_key_map = list(list(key = "u", value = "unique(", mode = "r")))`
- [ ] Ctrl-U inserts `unique(`

### 9. Matching-Bracket Highlight

- [ ] Type `(` then `)` → '(' highlights yellow briefly

## Known Gaps

- **UTF-8 encoding option:** Upstream Python radian sets R option `encoding`
  to `UTF-8` on Windows for R >= 4.2. This was part of `set_windows_utf8`
  which was removed during the Windows platform drop. macOS R handles
  encoding natively via its framework. No action needed unless a user reports
  encoding issues.
- **CI matrix:** No automated macOS CI build or test job exists.

## Results

| Date | Tester | R Installation | Result |
|---|---|---|---|
| | | | |
