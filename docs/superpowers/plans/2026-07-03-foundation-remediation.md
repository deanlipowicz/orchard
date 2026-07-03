# Foundation Remediation Plan

**Date:** 2026-07-03
**Branch:** main (commit directly)
**Target:** P0-P8, sequential phases

## P0 — Bug Fixes (~40m)

Three correctness bugs in magic handlers:

1. **Dead code in `resolve_edit_target`** (`src/magics/edit_magic.rs:76-99`)
   - `starts_with('-')` branch unreachable because `contains('-')` catches it first
   - Tighten condition so negative-index handler executes

2. **False "History is empty" in `%edit`** (`src/magics/edit_magic.rs:41-46`)
   - `saturating_sub(1)` yields 0 when `entries.len() == 1`, but history isn't empty
   - Test `entries.is_empty()` instead

3. **Panic-capable `unwrap()` in history replay** (`src/magics/history_magics.rs:309,355`)
   - `.find().unwrap()` can panic REPL on snapshot/entries mismatch
   - Return `MagicError` instead

**Verification:** All tests pass. Add regression test for each bug.

## P1 — Extract Shared R-Eval Primitives (~1h 45m)

Four magic modules define private copies of `eval_r_captured` with different behavior.
Create `src/magics/r_utils.rs` with canonical implementations.
Update all modules to import from `super::r_utils`.
Audit for other duplicated R-eval patterns.

## P3 — Split Overgrown Modules (~2h 30m)

1. `completion.rs` (1894 lines) → `src/completion/` submodules
2. `magics/shell.rs` (1114 lines) → `src/magics/shell/` submodules

No public API changes.

## P4 — Deduplicate Magic Dispatch (~30m)

Extract duplicated `match MagicOutput` block in `r_runtime.rs` into shared function.

## P5 — Remove Dead Code (~20m)

1. Remove `emacs_bindings_in_vi_insert_mode` from settings
2. `cargo clippy` dead_code audit

## P6 — Fill Test Gaps (~2h 45m)

Add tests to: `edit_magic.rs`, `workspace.rs`, `timing.rs`, `logging.rs`, `magic_help.rs`, `lsmagic.rs`, `settings.rs`

## P7 — Documentation Sync (~1h 5m)

Update README counts, add Installation/Configuration sections.
Update CONTRIBUTING.md.

## P8 — CI & Safety Audit (~1h 10m)

Add doc-tests and coverage to CI. Audit unwrap() calls.

## Summary

| Phase | Type | Est. |
|-------|------|------|
| P0 | Bug fixes | 40m |
| P1 | Shared primitives | 1h 45m |
| P3 | Split modules | 2h 30m |
| P4 | Dedup dispatch | 30m |
| P5 | Dead code | 20m |
| P6 | Test gaps | 2h 45m |
| P7 | Docs sync | 1h 5m |
| P8 | CI & safety | 1h 10m |
| **Total** | | **~10h 45m** |
