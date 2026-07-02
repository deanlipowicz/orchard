# Design History

> **⚠️ STALE DOCUMENT — 2026-07-02 Audit**
> Handler count (50), test count (249), module references (`automagic.rs`,
> `timing.rs`, `doc.rs`), and the `HISTORY_SNAPSHOT` static name do not match
> the current codebase. References to "P3 — timing/profiling" describe modules
> that were never created in the crate. See `docs/developer-log.md` § 2026-07-02
> — Documentation vs Code Audit for the full discrepancy catalog.

Major milestones and key design decisions during the radian-rs Rust rewrite.
For a full upstream comparison, see `docs/review-2026-07-01.md`.

---

## Timeline

### 2026-06-28: Foundation
- Phase 1 CLI parsing and environment setup uplift — sufficient for v1
- Phase 2 dynamic loader path repair + macOS BLAS injection — sufficient after smoke fix
- Phase 3 runtime helpers (RValue, call_string, error context) — sufficient
- Phase 5 console input chunking, nested prompt fallback, terminal width, Ctrl-C interrupt
- Phase 3 test coverage and smoke check repair

### 2026-06-29: Core Parity
- Phase 5 R event/input hook processing — timer-based SIGALRM for `R_PolledEvents()`
- Milestone D editing polish: auto-pairs, external editor, bracketed paste
- Milestone D Phase 2: vendored reedline pre-edit hook for context-aware editing
  - `insert_pair` with `cursor_in_string` + `following_text_accepts_pair` guards
  - `closing_delimiter` skip/smart-dedent, `smart_backspace`, `enter_indent`, `smart_tab`
- Milestone D Phase 2f+2g: shell-mode backspace exit, Ctrl-C completion menu cancel
- Milestone C: `RadianHistoryBackend` — mode-aware reedline History trait wrapper
- Phase 8 LaTeX table verified (2493 entries) — sufficient

### 2026-06-30: Feature Completeness
- Autosuggest wiring (`DefaultHinter` conditioned on `radian.auto_suggest`)
- Custom keybinding maps (`escape_key_map`, `ctrl_key_map` parsed from R options)
- Matching-bracket highlight (`RadianHighlighter` carries highlight flag)
- macOS acceptance plan (checklist at `docs/superpowers/plans/2026-06-30-macos-acceptance.md`)

### 2026-07-01: Segfault Fixes
- **Crash 1:** `--no-readline` restored + noop history callbacks (infinite C stack overflow from `savehistory`)
- **Crash 2:** Deferred `sync_terminal_width` via atomic (`PENDING_WIDTH`) — avoids re-entering R parser
  from within `read_console` callback
- **Crash 3:** Replaced `R_ParseVector` with manual call construction via `Rf_lang2` + `Rf_cons` (malformed
  SEXP from R 4.6.1 parser, pointer `0x1`)
- **Crash 4:** Pointer validation in `eval_code` (`0x1000` guard) + `eval_get_option`/`eval_set_option`/`eval_source_file`
  replace all parser-dependent evaluation paths

### 2026-07-01: Magic Command Implementation (P0–P7)
All 7 phases implemented in one extended session. 50 handlers total.

| Phase | Files | Key decisions |
|-------|-------|---------------|
| P0 | `src/magic.rs`, `src/magics/lsmagic.rs`, `magic_help.rs`, `automagic.rs` | `Arc<dyn MagicHandler>` for reentrant safety; `parse_magic_line_automagic` with `R_existsVarInFrame` FFI conflict check |
| P1 | `src/magics/shell.rs`, `src/shell.rs` | `ShellState` via `OnceLock<Mutex<...>>`; `!` dispatch after `;`, before `%` |
| P2 | `src/magics/inspect.rs` | `eval_r_captured()` helper wraps `capture.output` + `eval_string_raw_global`; optional package check via `requireNamespace` |
| P3 | `src/magics/timing.rs` | `%timeit` runs 7 iterations, reports min/mean/max |
| P4 | `src/magics/history_magics.rs`, `src/history.rs` | `HISTORY_SNAPSHOT` static mirrors ShellState pattern |
| P5 | `src/magics/debug.rs` | `PDB_ENABLED` AtomicBool toggle + `options(error = browser)` |
| P6 | `src/magics/doc.rs` | Text help via `help(..., help_type="text")` |
| P7 | `src/magics/config.rs` | `ALIAS_MAP` static with `expand_aliases()` called before dispatch |

---

## Key Architecture Decisions

| Decision | Rationale |
|----------|-----------|
| `Arc<dyn MagicHandler>` not `Box<dyn MagicHandler>` | Prevents reentrant mutex deadlock when `%lsmagic` calls `list_all()` on the same registry |
| Magic dispatch in `read_console_interactive` | Runs on Rust side before returning input to R; avoids R FFI reentrancy issues |
| `eval_string_raw_global` as public API | Replaces unsafe `eval_code`/`sexp_to_string` with safe wrapper for handler use |
| `OnceLock<Mutex<T>>` for shared state | Matches codebase pattern (ShellState, HISTORY_SNAPSHOT, ALIAS_MAP); no special init needed |
| `capture.output()` in R for handler output | Keeps output in R's console pipeline; `write_magic_output` routes through `write_console_ex` |
| Vendored reedline with pre-edit hook | Reedline 0.48 lacks callback API; vendoring allows `pre_edit_hook` field for context-aware editing |
| No full R parser in lexer | `cursor_in_string` heuristic sufficient for editing transforms; full parser is O(n²) risk with no benefit |
| Flat `src/*.rs` layout | Avoids premature modularization; split only when ownership boundary justifies it |

---

## Code Quality Baselines

| Metric | Standard |
|--------|----------|
| Clippy | `cargo clippy --all-targets -- -D warnings` — 0 in-crate errors/warnings |
| Tests | `cargo test -- --test-threads=1` — 249 pass (172 unit + 71 integration + 6 embedded R) |
| Safety | `#![deny(unsafe_op_in_unsafe_fn)]` — all unsafe blocks documented |
| Unwraps | 25 production `unwrap()` calls — each has safety rationale comment |
| Dependencies | `cargo audit` — 0 vulnerabilities |
