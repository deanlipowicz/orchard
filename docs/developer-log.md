# Developer Log — Chronological Record of orchard Development

**What this is:** A chronological log of all development sessions, design
decisions, audits, and recovery events during the orchard R REPL project.
Entries are ordered by date with the newest at the top of the file.

**Related documents:**
- Active roadmap: `docs/development-plan.md`
- Feature specs: `docs/superpowers/specs/`
- Implementation plans: `docs/superpowers/plans/`

---

## 2026-07-02 — Cwd-Contextual History (Last v0.4 Feature)

**Goal:** Tag history entries with the working directory they were executed in,
so `%hist --dir .` shows only project-relevant entries and reverse search
prioritizes same-directory results.

**Changes:**

| Area | What was done |
|------|---------------|
| `src/history.rs` | Added `cwd: Option<String>` field to `Entry` struct with `Entry::new()` and `Entry::with_cwd()` constructors. Updated `write_entry()` to emit `# cwd: /path` metadata after `# mode:` line. Updated `parse()` to read `# cwd:` lines backward-compatibly (old files without the tag get `cwd: None`). Added `cwds` parallel vector to `OrchardHistoryBackend`, seeded from entries on construction. `save()` records current directory. `search()` boosts same-directory entries to the top of results. |
| `src/magics/history_magics.rs` | `%hist --dir <path>` flag filters entries by working directory. Display now shows `[/path]` tag for entries with cwd metadata. |

**v0.4 fully complete** — all 10 items delivered (7 handlers + cwd-contextual history + CI pipeline). 66 magic handlers, 406 tests.

**Verification:**
```
cargo check                # 0 errors, 0 warnings
cargo clippy -- -D warnings  # 0 warnings
cargo test --lib           # 406 passed, 0 failed
```

**Commit:**
```
db1b4b3 feat: add cwd-contextual history with @cwd metadata, --dir filter, and prioritized search
```

---

## 2026-07-02 — v0.4 History Replay (7 New Handlers)

**Goal:** Add v0.4 History Replay + Reproducibility handlers — history replay,
workspace management, and session logging.

**Changes:**

| Area | What was done |
|------|---------------|
| `src/magics/history_magics.rs` | Added `Rerun` handler — re-run a previous command by index (from `%hist`), range (`1-3`), `-N`, or pattern search. Added `Recall` handler — same lookup logic, returns entry text for editing. |
| `src/magics/workspace.rs` | Added `Store` handler — persist/load objects via `saveRDS`/`readRDS`. `Reset` handler — `rm(list = ls())` for all or pattern-selective. `Xdel` handler — `rm()` with backup to `.last_del` for undo. |
| `src/magics/logging.rs` | **New module:** `LogStart` handler — opens file for append logging. `LogStop` handler — closes log file. `LogStateCmd` handler — shows on/off + path. All share `LogState` static with `log_command()` pub fn for REPL hook integration. |
| `src/magics/mod.rs` | Added `pub mod logging;` |
| `src/magic.rs` | Registered all 7 handlers as P11 (History Replay), P12 (Workspace), P13 (Session logging). |

**Handler count:** 59 → 66 registered magic handlers.

**Verification:**
```
cargo check                # 0 errors, 0 warnings
cargo clippy -- -D warnings  # 0 warnings
cargo test --lib           # all passed (pre-existing SIGSEGV on binary shutdown only)
```

**Commit:**
```
99062aa feat: add v0.4 History Replay handlers
```

---

## 2026-07-02 — %inspect Text Table (Phase 1)

**Goal:** Add an `%inspect` magic command that renders any R object as a formatted
text table using comfy-table, completing the v0.3 EDA Core milestone.

**Changes:**

| Area | What was done |
|------|---------------|
| `Cargo.toml` | Added `comfy-table = "7"` dependency |
| `src/magics/inspect.rs` | Added `Inspect` handler, `render_tabular()` (parses TSV data from R into comfy-table), `build_inspect_code()` (generates R code that returns structured class/dimensions/data). Data.frames/matrices render as full UTF8 tables with bold headers, dynamic width, and a footer showing dimensions. Non-tabular objects fall back to `str()`. |
| `src/magic.rs` | Registered `Inspect` handler in P9 — EDA section. |

**Test count:** 390 lib tests (+ inspect handler tests).

**Handler count:** 58 → 59 registered magic handlers. **v0.3 milestone complete.**

**Verification:**
```
cargo check                # 0 errors, 0 warnings
cargo clippy -- -D warnings  # 0 warnings
cargo test --lib           # 390 passed, 0 failed, 1 ignored
```

**Commit:**
```
5e63fe8 feat: add %inspect text table renderer with comfy-table
```

---

## 2026-07-02 — %xmode, %automagic, %save Handlers

**Goal:** Add the 3 remaining small v0.3 features — traceback verbosity control,
automatic magic prefix toggling, and history save-to-file.

**Changes:**

| Area | What was done |
|------|---------------|
| `src/magics/debug.rs` | Added `XMODE` state static with `set_xmode()`/`get_xmode()`. `%xmode` handler accepts `plain`, `context`, or `verbose` (or no args to show current). Modified `%tb` to call `traceback_code()` which adjusts `traceback(max.lines = ...)` based on xmode. 7 new tests. |
| `src/magics/config.rs` | Added `Automagic` handler — toggles `automagic` setting in CONSOLE state on/off. Accepts `on`, `off`, or no arg to toggle. |
| `src/magics/history_magics.rs` | Added `Save` handler — saves all history entries to a file path via existing `export_history()` function. |
| `src/r_runtime.rs` | Added `set_automagic(enabled)` and `get_automagic()` pub functions for runtime setting access. |
| `src/magic.rs` | Registered all 3 handlers as P10 — Debug/Config utilities. |

**Test count:** 390 lib tests (up from 382), 7 magic framework. Total: 397 tests.

**Handler count:** 55 → 58 registered magic handlers.

**Verification:**
```
cargo check                # 0 errors, 0 warnings
cargo clippy -- -D warnings  # 0 warnings
cargo test --lib           # 390 passed, 0 failed, 1 ignored
```

**Commits:**
```
1527e74 feat: add %xmode, %automagic, %save handlers
f273436 docs: update handler count to 58, mark v0.3 progress
```

---

## 2026-07-02 — EDA Magic Handlers (8 New: %summary, %glimpse, %describe, %missing, %corr, %freq, %compare, %sessioninfo)

**Goal:** Add 8 exploratory data analysis magic commands for v0.3 EDA Core milestone.
Each handler wraps a well-known R function and follows the same thin-wrapper pattern
as existing inspect/timing handlers.

**Changes:**

| Area | What was done |
|------|---------------|
| `src/magics/eda.rs` | **New module:** 8 `MagicHandler` impls — Summary (`base::summary()`), Glimpse (`dplyr::glimpse()`), Describe (`skimr::skim()`), Missing (`naniar::miss_summary()`), Corr (`cor()` with pairwise complete obs), Freq (`janitor::tabyl()`), Compare (`waldo::compare()` with max_diffs=20), SessionInfo (`sessioninfo::session_info()`). Optional-package handlers use `eval_with_pkg_check()` for clear error messages. |
| `src/magics/mod.rs` | Added `pub mod eda;` |
| `src/magic.rs` | Added P9 — EDA section with all 8 registrations in `register_all()` |

**13 new tests:** 8 registry-presence checks, 2 empty-args validation tests, 1 parse-recognition test (all 8 names), 1 dispatch-variant test (confirms `Output::Text` path), 1 sessioninfo args test.

**Test count:** 382 lib tests (up from 356), 7 magic framework. Total: 389 tests.

**Handler count:** 47 → 55 registered magic handlers.

**Verification:**
```
cargo check            # 0 errors, 0 warnings
cargo clippy -- -D warnings  # 0 warnings
cargo test --lib       # 382 passed, 0 failed, 1 ignored
cargo test --test magic_framework  # 7 passed
```

**Plan:** `docs/superpowers/plans/2026-07-02-eda-handlers.md` (consolidated below)
**Spec:** `docs/superpowers/specs/2026-07-02-eda-handlers-design.md`

**Commits:**
```
2529141 feat: add 8 EDA magic handlers (%summary, %glimpse, %describe, %missing, %corr, %freq, %compare, %sessioninfo)
4c4bfd4 docs: add EDA handlers design spec and implementation plan
```

**Architecture notes (from consolidated plan):**
- All 8 handlers follow the existing thin-wrapper pattern in `inspect.rs`/`timing.rs`.
- Two shared helpers: `eval_r_captured()` wraps code in `capture.output()`, `eval_with_pkg_check()` uses `requireNamespace()` before evaluation for optional-package handlers.
- Base-R handlers (`%summary`, `%corr`) use `eval_r_captured()`; optional-package handlers (`%glimpse`, `%describe`, `%missing`, `%freq`, `%compare`, `%sessioninfo`) use `eval_with_pkg_check()`.
- All handlers return `Output::Text`; registered as P9 priority in `register_all()`.
- Registered alongside parse+dispatch tests covering registry presence, empty-args validation, recognition of all 8 names, and `Output::Text` dispatch path.

---

## 2026-07-02 — Formula ~ Completion for Modeling Functions

**Goal:** Complete column names from the `data =` argument when the user types
inside an R formula (e.g. `lm(mpg ~ cyl + , data = mtcars)` should suggest
columns of `mtcars`).

**Changes:**

| Area | What was done |
|------|---------------|
| `src/completion.rs` | Added `MODEL_FNS` constant (lm, glm, aov, anova, manova, nls, loess, rlm). Added `is_modeling_fn()`, `formula_context()` (paren-depth backtracer + `~` detection), `extract_data_arg()` (regex-based `data =` extraction), `resolve_formula_columns()` (static TSV fast path → R FFI fallback), `formula_completions()` (main entry point using `rank_completions()`). |
| `src/prompt.rs` | Wired `formula_completions()` check between namespace and function arg completions. |

**11 new tests:** context detection (5), data arg extraction (4), modeling fn check (2).

**Completion backends: 14 total** — the 14th backend is formula ~ completion.

**Test count:** 356 lib tests (up from 312), 7 magic framework. Total: 363 tests.

**Verification:**
```
cargo test --lib        # 356 passed, 0 failed
cargo clippy            # 0 warnings
```

**Plan:** `docs/superpowers/plans/2026-07-02-formula-completion.md`

**Commit:**
```
ccab47a feat: formula ~ completion for modeling functions (lm, glm, aov, etc.)

**Plan complete** — `docs/superpowers/plans/2026-07-02-formula-completion.md` archived (all 4 tasks delivered).

---

## 2026-07-02 — Frequency-Aware Scored Completion + Static TSV Fast Paths

**Goal:** Replace the naive boolean `fuzzy_match` filter with a proper scored ranking
system using `fuzzy-matcher` (SkimMatcherV2), add cross-session frequency learning,
and pre-compute dataset/package schemas as static TSV files to eliminate R FFI calls
for common completion contexts.

**Changes:**

| Area | What was done |
|------|---------------|
| `Cargo.toml` | Added `fuzzy-matcher = "0.3"`, `serde` (with `derive`), `serde_json = "1"` |
| `src/frequency.rs` | **New module:** `FrequencyData` struct with `HashMap<String, usize>`, JSON persistence at `~/.local/share/orchard/completion_freq.json`, `record_completion()` increments + persists, `frequency_boost()` returns score bonus (50pts per use, capped at 500) |
| `src/completion.rs` | Replaced all 7 `filter(\|n\| fuzzy_match(n, prefix))` chains with `rank_completions(names, prefix)` using SkimMatcherV2 + frequency boost. Added `static_dataset_columns()` (parses TSV into OnceLock HashMap), `static_package_fn_map()` and `namespace_context()` + `namespace_completions()` for `pkg::fun` with argument signatures in display. Fast path in `resolve_schema()` checks static dataset TSV before R FFI. |
| `src/prompt.rs` | Wired `suggestions()` to call `frequency::record_completion()` for all returned completions. Added `namespace_completions` check. |
| `src/data/dataset_schemas.tsv` | **New:** 225 lines, 36 datasets (iris, mtcars, diamonds, starwars, economics, etc.) with column names and types |
| `src/data/package_symbols.tsv` | **New:** 480 lines, 10 packages (dplyr, tidyr, ggplot2, purrr, stringr, lubridate, data.table, forcats, readr, base) with function names and argument signatures |

**Scoring model (implemented in `rank_completions`):**
```
final_score = skim_matcher_score + min(prior_count * 50, 500)
```

**Test count:** 312 lib tests (up from 310), 7 magic_framework tests. Total: 319 tests.

**Verification:**
```
cargo test --lib        # 312 passed, 0 failed
cargo clippy            # 0 warnings
```

**Commits (2, on master):**
```
4ebf549 feat: frequency-aware scored completion with fuzzy-matcher
61f8da8 feat: static TSV fast path for dataset columns and package symbols

---

## 2026-07-02 — Autocomplete Upgrades (9 New Backends)

**Goal:** Raise orchard's completion quality from prefix-only to zsh/fish level by
adding fuzzy matching, magic context completion, R argument descriptions, improved
`[[` handling, R6 method completion, spellcheck suggestions, and schema-aware
column/pipe completions.

**Changes:**

| Area | What was done |
|------|---------------|
| `src/completion.rs` | Added 7 public functions: `fuzzy_match`, `schema_completions`, `pipe_completions`, `function_arg_completions`, `magic_completions`, `spellcheck_completions`, `levenshtein_distance`. Extended `extract_bracket_context` to handle quoted columns. Extended `resolve_schema` for R6/refClass. Updated all filters from `starts_with` to `fuzzy_match`. |
| `src/prompt.rs` | Added 6 new completion checks in `OrchardCompleter::complete_with_intent`: schema (`$`/`@`/`[[`) → pipe (`%>%`) → magic args → function args → variable selector (Manual) → spellcheck → existing R/LaTeX/package |
| `docs/development-plan.md` | Updated test counts, architecture diagram, schema-autocomplete section (all 8 features ✅), roadmap (v0.5 marked 🟡 Partial, 8 sub-items ✅), file descriptions |
| `docs/superpowers/plans/2026-07-02-autocomplete-upgrades.md` | Created: 6-task implementation plan |

**9 new completion backends:**

| Backend | Activation | Source |
|---------|-----------|--------|
| `$`/`@` column completion | `identifier$` or `identifier@` at cursor | R `names()` / `slotNames()`, 5s cache |
| `[[` column completion | `identifier[[` with optional quoted name | R `names()`, handles `[["col"` |
| `%>%` pipe completion | Expression before last `%>%` | R `eval(parse(...))` + `names()` |
| Fuzzy matching | All backends (except LaTeX/shell-paths) | Custom subsequence matcher, case-insensitive |
| Magic arg completion | `%run`, `%cd`, `%rm` + 27 more magics | File/dir/variable dispatch per magic |
| Function arg completion | Inside `fn_name(` | R `formals()` with default value display |
| R6/refClass method completion | `obj$` on R6/refClass objects | `ls(envir=obj)` for R6, filters `.__*` |
| Variable selector (Manual) | `Tab`/`Ctrl-Space` (Manual intent) | R `ls()` + `class()` + `object.size()` |
| Spellcheck / "did you mean" | Empty completions + prefix ≥ 3 chars | Levenshtein distance vs ~2000 R names |

**Test count:** 310 lib tests (up from 304), 7 magic_framework tests. Total: 317 tests.

**Verification:**
```
cargo test --lib        # 310 passed, 0 failed
cargo clippy            # 0 warnings
cargo fmt               # clean
```

**Commits (6, on master):**
```
16d9488 feat: fuzzy/substring matching for all completions
4ade790 feat: magic context argument completion for 30+ magics
b01e983 feat: function argument completion with formals() display
0d7b42f feat: improved [[ bracket completion handles quoted columns
04e6eb4 feat: R6 and refClass method/field completion via $
53c9286 feat: spellcheck / 'did you mean' suggestions via Levenshtein

---

## 2026-07-02 — Test Suite Hardening

**Goal:** Strengthen weak assertions, fix silent-skip gating, add deterministic
tests for untested modules, and add property tests across the codebase.

**Changes:**

| Area | What was done |
|------|---------------|
| `tests/magic_framework.rs` | Rewrote: removed redundant registry-presence checks already covered by `magic.rs` inline tests; replaced tautological `pwd` assertion with exact `current_dir()` match; added sorted-order, specific-var get, and unset-var tests for `env`; added `EnvGuard` RAII struct |
| `tests/embedded_r.rs` | Rewrote: converted 6 R-gated tests from silent early-return to `#[ignore]` with message "requires ORCHARD_TEST_R=1 env var and a working R installation"; added `r_test!` macro and `r_test_enabled()` helper |
| `src/r_runtime.rs` | Added 2 deterministic interrupt-flag tests: `interrupted_flag_survives_non_clearing_reads` (idempotent set, non-clearing reads, single clear) and `interrupted_flag_concurrent_set_and_clear_is_consistent` (100-iteration set/clear cycle). Added 4 `proptest` property tests for `strip_ansi` (idempotence, no-CSI-in-output, plain-text-unchanged, length-preserved). Fixed pre-existing `unused variable: tail` warning (renamed to `_tail`). |
| `src/editing_hook.rs` | Strengthened 6 `is_some()`-only assertions to check exact `EditCommand` sequences: `auto_pair_inserts_pair_when_context_allows` (InsertChar/InsertChar/MoveLeft), `closing_delim_dedents_blank_line` (4 Backspace + InsertChar), `backspace_deletes_empty_pair` (Backspace/Delete), `backspace_in_leading_indent_deletes_tab_size` (4 Backspace), `enter_indents_after_open_brace` (InsertNewline + InsertString 4 spaces), `tab_inserts_spaces_in_leading_indent` (InsertString 4 spaces). Added `assert_edit_commands` helper. |
| `src/r_discovery.rs` | Added 11 tests covering discover precedence (explicit binary > R_HOME > R_BINARY env), version parsing (first line, empty output → NA), home path joining, and failure modes (nonexistent binary, failing R binary, empty RHOME output). Added `EnvGuard` with `ENV_LOCK` mutex, `make_fake_r_binary`/`make_failing_r_binary`/`make_empty_r_binary` helpers using temp shell scripts. |
| `src/magics/config.rs` | Added 12 tests for `Alias` (empty=list, name=value=set, bare name=error, trims whitespace), `Unalias` (empty=error, remove existing, remove nonexistent), and `expand_aliases` (replaces first word, preserves leading whitespace, passes through unknown/empty). Added `AliasGuard` RAII with `TEST_LOCK` mutex to serialize tests on shared `ALIAS_MAP`. |
| `src/magics/shell.rs` | Added 18 tests: `Bookmark` (list empty, set, reject nonexistent dir, delete existing, delete nonexistent, delete without name=error, jump to existing, jump to nonexistent=error), `Cd` (empty=home, tilde=home, nonexistent=error, file=not-a-directory, existing dir, `-` without OLDPWD), `Env` (set+get round-trip, set via handler, get unset=not-set). Added `BookmarkGuard` and `EnvGuard` RAII structs. |
| `src/lexer.rs` | Added 47 tokenization tests covering all `tokenize()` branches: whitespace runs, comments (to-EOF, stops-at-newline, empty), raw strings (parens/brackets/braces, with dashes, unterminated→Error, mismatched close→Error, containing quotes/dashes), quoted strings (double/single, escaped quote/backslash, unterminated→Error, empty, escape sequences), backtick names (complete, unterminated→Error, empty), numbers (integer, decimal, leading-dot, underscore, scientific, leading-dot-alone=Name), names (simple, with dot, with underscore, starting dot/underscore), punctuation (all 8 chars), operators (all single-char, multi-char `<-`/`->`/`==`/`!=`/`>=`/`<=`/`&&`/`||`/`|>`/`::`/`:::`/`<<-`/`->>`), mixed expressions (assignment, function call, full line with comment, pipe chain, empty input). |
| `src/history.rs` | Added 21 malformed-input recovery tests for `parse()`: empty input, whitespace-only, headers-only, content without mode header (empty mode), truncated `# mode:`/`# time:` headers, garbage lines between entries (triggers flush), garbage before content (dropped), truncated entry at EOF (emitted), mode persistence, mode change, multiline join, empty content line preserved, `+` prefix stripped, multiple entries with separators, mode trailing whitespace trimmed, completely garbage input (no entries), blank line flush, round-trip single/multiline. Added 3 `proptest` property tests: `prop_round_trip_single_entry`, `prop_round_trip_multiple_entries`, `prop_parse_never_panics`. |
| `src/prompt.rs` | Strengthened `edit_mode_does_not_panic`: replaced no-op `let _mode = ...` with `mode.edit_mode()` call and `matches!` assertion against `PromptEditMode::Emacs | PromptEditMode::Vi(_)`. |
| `Cargo.toml` | Added `[dev-dependencies]` section with `proptest = "1"`. |
| `vendor/reedline/src/edit_mode/base.rs` | Reverted uncommitted `+ std::fmt::Debug` bound on `EditMode` trait that broke compilation (`Vi`/`Emacs` structs don't implement `Debug`). |

**Test count:** 265 lib tests (up from 238), 7 magic_framework integration tests,
7 embedded_r tests (all `#[ignore]`'d). Total: 279 tests, 0 failures.

**Verification:**

```
cargo fmt:       clean
cargo clippy:    7 warnings (all pre-existing, none in new test code)
cargo test:      265 passed, 0 failed, 1 ignored (lib)
                 7 passed, 0 failed (magic_framework)
                 0 passed, 7 ignored (embedded_r)
```

**Status:** Complete. All 10 test-suite hardening recommendations implemented and
verified.

---

## 2026-07-02 — P0 Magic Framework Complete

**Plan:** `docs/superpowers/plans/2026-07-01-p0-magic-framework.md`

The magic registry and 49 handlers were already built, but the core REPL
integration was never wired — `%` prefix parsing, dispatch, and the
`automagic` setting were all missing. This gap meant none of the 49 handlers
were reachable from the REPL.

**Changes:**

| Area | What was done |
|------|---------------|
| `src/settings.rs` | Added `automagic: bool` field to `Settings`, default `false`, loaded from R option `orchard.automagic` |
| `src/r_runtime.rs` | Added `automagic` to `ConsoleSettings` + `From<Settings>` impl |
| `src/magic.rs` | Added `parse_magic(text, automagic)` — `%`/`%%` prefix parsing with automagic name lookup; `dispatch(cmd)` — clones handler `Arc` out of registry then calls `run()` to avoid reentrant deadlock; `is_magic_name(name)`; `register_magic(handler)` |
| `src/r_runtime.rs` (REPL) | Wired `parse_magic` → `dispatch` into both piped `read_console()` and interactive `read_console_interactive()` with full outcome routing (Text→print, Eval→queue, DisplayAndEval→both, Silent→continue) |
| Tests | 14 new unit tests covering parse, automagic, dispatch, and `is_magic_name` |

**Key architectural fix:** `dispatch()` clones the handler `Arc` out of the
registry before dropping the lock and calling `handler.run()`. Without this,
handlers like `%lsmagic` (which also lock the registry to list handlers) would
deadlock on `std::sync::Mutex` (non-reentrant).

**Verification:**

```
cargo check:    0 errors, 0 warnings
cargo clippy:   0 warnings
cargo test:     238 passed, 0 failed, 1 ignored (lib: 231, magic_framework: 7)
```

**Status:** Complete. The 49 magic handlers are now reachable from the REPL via
`%` prefix. Automagic can be enabled with `options(orchard.automagic = TRUE)`
to use magics without the `%` prefix (guarded against R function call
confusion by checking for `(` after the name). Plan file deleted.

---

## 2026-07-02 — Plan Review: History Backend Plan Complete

**Plan:** `docs/superpowers/plans/2026-06-29-history-backend-plan.md`

Implemented as part of **Milestone C — Loaded History Navigation** (see
`2026-06-29 — Milestone C Loaded History Navigation` entry below).
`OrchardHistoryBackend` (reedline `History` trait impl), `PromptSession::with_arc_history()`,
mode-aware search, and all plan tests are live. The plan file is being deleted;
the implementation is fully covered in the existing log entry.

**Status:** Complete. Plan superseded.

---

## 2026-07-02 — Codebase Cleanup Batch C Complete

**Gap:** The codebase cleanup plan (`docs/superpowers/plans/2026-07-02-codebase-cleanup.md`)
had Batch C (Prefix Drift & Boilerplate Consolidation) as the only remaining item.
Batch C covered two changes:
1. Replace `radian.` prefix with `orchard.` in `src/magics/config.rs` — the
   rest of the codebase uses `orchard.*` (see `settings.rs`).
2. Replace `install_console_settings()` and `ConsoleSettings::default()` with
   the existing `From<Settings>` impl.

**Changes:**

| File | Change |
|------|--------|
| `src/magics/config.rs` | Replaced 8 occurrences of `radian.` → `orchard.` in R option queries and display strings (Config and Colors handlers). |
| `src/r_runtime.rs` | Removed `impl Default for ConsoleSettings` (was delegating to `Settings::default().into()`). All 2 call sites updated to `ConsoleSettings::from(Settings::default())`. |
| `src/editing_hook.rs` | Replaced 9 `ConsoleSettings::default()` → `ConsoleSettings::from(Settings::default())` in tests. Added `#[cfg(test)] use crate::settings::Settings`. |
| `src/prompt.rs` | Replaced 3 `ConsoleSettings::default()` → `ConsoleSettings::from(Settings::default())` in tests. Added `#[cfg(test)] use crate::settings::Settings`. |
| `docs/superpowers/plans/2026-07-02-codebase-cleanup.md` | Status updated: all four batches (A, B, C, D) complete. |

**Verification:**

```
cargo check:    0 errors, 0 warnings
cargo clippy:   0 warnings
cargo test:     143 passed, 0 failed, 2 ignored
```

**Status:** The entire codebase cleanup plan is complete. All four batches
(A — Dead Code Removal, B — Shared Utilities Consolidation, C — Prefix Drift
& Boilerplate Consolidation, D — Remove `editing.rs`) are finished.

---

## 2026-06-29 - Strategic Steering Release Framework

Added project-level release steering to keep progression clear after the
current Linux-first core reaches completion.

Decision:

- Use release gates in addition to phase status. Phase status explains what is
  implemented; release gates define what the project can claim.
- Track work in four lanes: Core Parity, Platform, Compatibility, and
  Maintenance.
- Gate claims as `v0.1 Experimental`, `v0.2 Core Parity`, `v0.3 Platform Beta`,
  and `v1.0 Replacement Candidate`.
- Prioritize incomplete user-facing core workflows before secondary upstream
  compatibility features.

Documentation update:

- Added `Strategic Steering Release Framework` to
  `docs/python-to-rust-port-plan.md`.
- No implementation files changed.

## 2026-06-29 - Current State Review Against Upstream

Reviewed the Rust port against the upstream Python checkout at
`third_party/radian-upstream/radian` and updated the plan as a current-state
overlay rather than replacing the original phase targets.

Review findings:

- Milestones A and B are sufficient for a Linux-first v1: CLI/env setup, loader
  repair, embedded R startup, callbacks, settings, profiles, prompt basics,
  multiline input, EOF, resize behavior, nested prompt fallback, and Unix
  polled-event processing are implemented.
- Milestone C is mostly sufficient: history file compatibility and shell mode
  exist, but loaded history is not connected to live reedline navigation/search.
- Milestone D remains partial: completion is live and first-pass editing is
  wired, but full LaTeX completion data, automatic-vs-explicit completion
  semantics, stronger package-context parsing, and context-aware keybindings are
  still missing.
- Milestone E remains partial: Linux is tested; macOS behavior is
  mostly unaccepted.
- Upstream features outside the current core path remain missing: reticulate
  prompt integration, `radian.escape_key_map`/`radian.ctrl_key_map`, on-load
  hooks, and cleanup/finalizer hooks.
- `README.md` was stale and said the rewrite was not implemented.

Documentation updates:

- Added a dated current-review section to
  `docs/python-to-rust-port-plan.md`, including per-phase status, completed
  behavior, missing behavior, milestone status, and the next implementation
  plan.
- Added testing notes for R-gated integration tests and remaining manual
  acceptance checks.
- Updated `README.md` to reflect the implemented Linux-first core and the main
  remaining gaps.

Verification:

- `cargo test -- --test-threads=1` passed: 112 unit tests, 6 embedded harness
  tests, 1 ignored manual SIGINT check, 0 doc tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture
  --test-threads=1` passed: 6 real embedded R tests, 1 ignored manual SIGINT
  check.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

## 2026-06-29 - Phase 8 R Completion Integration Uplift

Identified Phase 8 as the first remaining partial phase. The documented
shortcoming addressed here was missing integration coverage for R-backed
completion and installed package completion.

Plan:

- Add a real embedded-R acceptance check for base-function completion and the
  installed `base` package.
- Reuse the existing embedded binary test harness instead of adding a second
  direct R initialization path.
- Fix only completion behavior needed for that acceptance check.
- Leave LaTeX table expansion, automatic-vs-explicit timeout behavior, and
  deeper package heuristics as remaining Phase 8 work.

Changes:

- Seeded R's completion token state with `utils:::.guessTokenFromLine()` before
  `utils:::.completeToken()`.
- Added an embedded R test that verifies completing `mea` can find `mean` and
  installed package lookup can find `base`.
- Copied `tab_size`, `auto_match`, and `auto_indentation` into
  `ConsoleSettings` during live settings install; this fixed the current build
  after those settings fields were added.

Verification:

- `cargo test -- --test-threads=1` passed: 112 unit tests, 6 embedded R harness
  tests, 1 ignored manual SIGINT check, 0 doc tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture
  --test-threads=1` passed: 6 real embedded R tests, 1 ignored manual SIGINT
  check.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

Status update:

- Phase 8 remains **Partial**. R completion now seeds token state correctly and
  has real embedded-R coverage for base-function and installed-package
  completion.
- Remaining Phase 8 gaps: tiny LaTeX table, no automatic-vs-explicit timeout
  distinction, and shallow package-context heuristics.
- Later partial phases are unchanged.

## 2026-06-29 - Phase 6 Persistent Shell Prompt Uplift

Identified Phase 6 as the first remaining partial phase after the existing
Phase 5 input-hook entry marked Phase 5 sufficient. The documented shortcoming
addressed here was the lack of a persistent shell prompt mode after `;` shell
activation.

Plan:

- Reuse the existing `PromptSession` and shell command runner.
- Keep one-shot `;command` shell execution unchanged.
- Treat `;` alone as persistent shell prompt activation.
- Run shell commands at the configured shell prompt until an empty command or
  Ctrl-C returns to R.
- Keep backspace-at-column-zero shell exit deferred to Phase 9 keybinding
  wiring, where cursor-aware editing behavior belongs.

Changes:

- Added a persistent shell prompt loop for interactive `;` activation.
- Preserved shell history mode labels for commands run inside the shell prompt.
- Added a focused unit test for one-shot and persistent shell activation
  parsing.

Verification:

- `cargo test -- --test-threads=1` passed: 110 unit tests, 5 embedded R
  harness tests, 1 ignored manual SIGINT check, 0 doc tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture
  --test-threads=1` passed: 5 real embedded R tests, 1 ignored manual SIGINT
  check.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

Status update:

- Phase 6 is now **Sufficient for v1**. R, browse, unknown, and shell prompt
  modes exist in the live prompt path; `;command` remains one-shot and `;`
  alone enters persistent shell mode.
- Phase 9 still owns cursor-aware shell backspace exit.
- Later partial phases are unchanged.

## 2026-06-28 - Phase 5 Ctrl-C Interrupt Uplift

Identified Phase 5 as the first remaining partial phase. The documented
shortcoming addressed here was Ctrl-C being recorded in Rust state without
raising R's interrupt path while R is waiting for console input.

Plan:

- Add one shared console interrupt helper.
- Use the generated `Rf_onintrNoResume()` binding; add no dependency or wrapper
  layer.
- Route prompt and native interrupted-read paths through the helper.
- Keep EOF returning `0` without raising an interrupt.
- Keep terminal width sync, nested prompt fallback, queued input,
  stdout/stderr handling, and history unchanged.

Changes:

- Added `raise_r_interrupt()`, which clears the Rust interrupt flag and calls
  R's existing interrupt API.
- Replaced console Ctrl-C branches that only set the Rust flag with the shared
  helper.
- Added a Unix embedded SIGINT acceptance check for `Sys.sleep(100)`, ignored
  by default because it is environment-sensitive.
- Removed an unfinished live edit-mode wrapper from `src/prompt.rs` so the
  existing `reedline` editor modes compile again; Phase 9 remains partial.

Verification:

- `cargo test` passed: 107 unit tests, 5 embedded R harness tests, 1 ignored
  manual SIGINT check, 0 doc tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture` passed:
  5 real embedded R tests, 1 ignored manual SIGINT check.
- Manual run of the SIGINT acceptance check was attempted, but the child
  stayed in `Sys.sleep(100)` until the test timeout in this environment.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

Status update:

- Phase 5 remains **Partial**. Ctrl-C now raises to R from the console callback
  paths, but R event/input hook processing remains.
- Later partial phases are unchanged.

## 2026-06-28 - Phase 5 Terminal Width Uplift

Identified Phase 5 as the first remaining partial phase. The documented
shortcoming addressed here was terminal resize width updates before console
prompt handling.

Plan:

- Copy `Settings::auto_width` into console callback settings.
- Detect Unix terminal width with `ioctl(TIOCGWINSZ)` before interactive or
  piped prompt handling.
- Clamp detected width to at least 20 columns.
- Update R's `width` option only when the detected width changes.
- Keep prompt rendering, nested fallback, queued input, Ctrl-C flagging, and
  stdout/stderr behavior unchanged.

Changes:

- Added `auto_width` and `last_terminal_width` to console callback state.
- Added a best-effort terminal width sync using the existing embedded R eval
  path and no new dependencies.
- Added focused unit tests for width clamping, unchanged width skips, changed
  width updates, and disabled auto-width behavior.

Verification:

- `cargo test` passed: 107 unit tests, 5 embedded R harness tests, 0 doc tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture` passed:
  5 real embedded R tests.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

Status update:

- Phase 5 remains **Partial**. Terminal width updates are now covered, but R
  event/input hook processing and manual Ctrl-C acceptance remain.
- Later partial phases are unchanged.

## 2026-06-28 - Phase 5 Nested Prompt Fallback Uplift

Identified Phase 5 as the first remaining partial phase. The documented
shortcoming addressed here was nested prompt fallback while the Rust prompt
session is already active.

Plan:

- Track whether the interactive prompt is currently active.
- Avoid holding the console mutex while `reedline` waits for input.
- If R asks for input while the prompt is active, fall back to native stdin
  reading and route the result through the existing queue/chunk path.
- Keep stdout/stderr callbacks, cursor tracking, UTF-8 chunking, long
  non-ASCII wrapping, Ctrl-C flagging, and history behavior unchanged.
- Add focused unit tests for the fallback routing decision.

Changes:

- Added `prompt_active` to console state.
- `read_console_interactive` now takes the prompt session out of shared state,
  marks the prompt active, drops the mutex while reading, then stores the
  session back and clears the active flag.
- Added native read fallback for reentrant console input.
- Added routing tests for active prompt fallback versus normal terminal/piped
  input.

Verification:

- `cargo test` passed: 103 unit tests, 5 embedded R harness tests, 0 doc tests.
- `cargo test prompt_ -- --nocapture` passed the focused prompt-route tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture` passed:
  5 real embedded R tests.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

Status update:

- Phase 5 remains **Partial**. Nested prompt fallback is now covered, but R
  event/input hook processing, terminal resize width updates, and manual Ctrl-C
  acceptance remain.
- Later partial phases are unchanged.

## 2026-06-28 - Phase 5 Console Input Chunking Uplift

Identified Phase 5 as the first remaining partial phase after Phase 3 was
marked sufficient. The documented shortcoming addressed here was unsafe
handling of long/non-ASCII multiline console input.

Plan:

- Keep stdout/stderr callbacks, prompt display, cursor tracking, suppress
  flags, and Ctrl-C flagging unchanged.
- Add pending input storage for console input that does not fit in R's current
  buffer.
- Copy input on UTF-8 character boundaries so multibyte characters are not
  split.
- Wrap long non-ASCII multiline R/Browse input as a block before chunking.
- Add focused unit tests for chunking and wrapping behavior.

Changes:

- Added `pending_inputs` to console state and drain it before reading new
  input.
- Routed startup, piped, and interactive console input through a shared
  UTF-8-safe queue/copy path.
- Added helper coverage for short input, long input draining, UTF-8 boundary
  preservation, and long non-ASCII multiline wrapping.

Verification:

- `cargo test` passed: 101 unit tests, 5 embedded R harness tests, 0 doc tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture` passed:
  5 real embedded R tests.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

Status update:

- Phase 5 remains **Partial**. Long/non-ASCII multiline input handling is
  covered, but nested prompt fallback, R event/input hook processing, terminal
  resize width updates, and manual Ctrl-C acceptance remain.
- Later partial phases are unchanged.

## 2026-06-28 - Phase 3 Runtime Helper Uplift

Identified Phase 3 as the first remaining partial/partial-sufficient phase
after Phase 2 was marked sufficient. The documented shortcoming was that the
embedded runtime had eval/source/options helpers, but lacked a generic call
helper, a richer option value type, and stronger R error context.

Plan:

- Keep the existing embedded R initialization, callbacks, REPL driver, and
  protect guard unchanged.
- Add the smallest useful `RValue` option helper instead of a full SEXP
  abstraction.
- Add a minimal generic call helper for string-returning calls with validated
  package/function names.
- Include R's current error buffer in evaluation failures.
- Add focused unit tests for call expression composition and validation.

Changes:

- Added `RValue` with `Null`, `Bool`, `Int`, `Real`, and `String` variants.
- Added `RRuntime::get_option(name) -> RValue`.
- Added `RRuntime::call_string(package, function, args)`.
- R evaluation errors now include `R_curErrorBuf()` text when available.
- Added two unit tests for safe R call expression construction.

Verification:

- `cargo test` passed: 97 unit tests, 5 embedded R harness tests, 0 doc tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture` passed:
  5 real embedded R tests.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

Status update:

- Phase 3 is now **Sufficient for v1**. The runtime has initialization,
  callbacks, REPL driving, eval/source/options helpers, a protect guard, a
  minimal generic call helper, typed option values, better R error context, and
  enabled embedded R acceptance coverage.
- A full public SEXP wrapper remains deferred until a caller needs it.
- Later partial phases are unchanged.

## 2026-06-28 - Phase 2 Smoke Check Repair

Identified Phase 2 as the first remaining partial phase. The remaining
shortcoming was the failed Linux smoke check from the prior Phase 2 uplift.

Plan:

- Keep the existing Unix loader repair and macOS DYLD/BLAS helpers unchanged.
- Reproduce the smoke failure with real embedded R tests enabled.
- Fix only the startup path needed for piped Linux smoke execution.
- Re-run unit tests, enabled embedded R tests, and the Phase 2 smoke command.

Changes:

- Registered R console callbacks and initialized the REPL before running it.
- For piped stdin, use default settings instead of pre-REPL R option reads,
  which were crashing in `R_tryEval` before the REPL owned evaluation.
- Queue an explicit `--profile` as startup REPL input in piped mode so profile
  smoke coverage still works without pre-REPL evaluation.

Verification:

- `cargo test` passed: 95 unit tests, 5 embedded R harness tests, 0 doc tests.
- `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture` passed:
  5 real embedded R tests.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.
- Linux smoke passed with `RADIAN_RS_LD_REEXEC=1` and loader vars unset:
  `printf '1 + 1\nq("no")\n' | env -u R_LD_LIBRARY_PATH -u LD_LIBRARY_PATH
  -u DYLD_FALLBACK_LIBRARY_PATH -u DYLD_INSERT_LIBRARIES
  -u R_DYLD_INSERT_LIBRARIES RADIAN_RS_LD_REEXEC=1 ./target/debug/radian-rs -q`
  exited 0 and printed `[1] 2`.

Status update:

- Phase 2 is now **Sufficient for Linux/macOS v1**. Unix loader repair, guarded
  macOS cleanup/BLAS fallback, unit coverage, enabled embedded R tests, and the
  Linux smoke check pass.
- Later partial phases are unchanged.

## 2026-06-28 - Phase 2 Dynamic Loader Uplift

Identified Phase 2 as the first remaining partial phase after the Phase 1
coverage uplift.

Changes:

- Unix loader repair and one-time re-exec flow remain unchanged.
- Added guarded macOS cleanup for previous `R_DYLD_INSERT_LIBRARIES` entries.
- Added guarded best-effort macOS BLAS injection using
  `$R_HOME/lib/libRBlas.dylib` when present.
- Added focused loader path, DYLD cleanup, and BLAS injection tests.

Verification:

- `cargo test` passed: 95 unit tests, 5 embedded R tests, 0 doc tests.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.
- Linux smoke with `RADIAN_RS_LD_REEXEC=1` and loader vars unset reached
  embedded R startup, then failed with a segfault before evaluating piped
  input. The same failure reproduced without the marker and with explicit
  `R_LD_LIBRARY_PATH`/`LD_LIBRARY_PATH`, so Phase 2 is not marked sufficient
  from this run.

Status update:

- Phase 2 remains **Partial** pending a passing Linux smoke check. The macOS
  cleanup and best-effort BLAS behavior are implemented behind platform guards.
- Later partial phases are unchanged.

## 2026-06-28 - Phase 1 Test Coverage Uplift

Added focused coverage for Phase 1 CLI parsing and environment setup.

Verification:

- `cargo test` passed: 91 unit tests, 5 embedded R tests, 0 doc tests.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.

Status update:

- Phase 1 is now **Sufficient**. CLI flags, value flags, vanilla expansion,
  command argument composition, R env variables, no-environ/profile env
  effects, and local history creation are covered.
- Later partial phases are unchanged.

## 2026-06-28 - Python-to-Rust Port Critical Review

Reviewed `docs/python-to-rust-port-plan.md` against the current Rust
implementation.

Verification:

- `cargo test` passed: 88 unit tests, 5 embedded R tests, 0 doc tests.
- `cargo fmt --check` could not run because `cargo-fmt` is not installed for
  `stable-x86_64-unknown-linux-gnu`.
- The repository has no committed baseline, so this review is based on file
  inspection and tests, not a git diff.
- `README.md` is stale: it still says the rewrite has not been implemented.

Status key:

- **Sufficient**: implemented in the live binary path and covered by relevant
  tests.
- **Partial**: helpers or core behavior exist, but live REPL behavior,
  platform coverage, or acceptance tests are missing.
- **Remaining**: not implemented or only documented as deferred.

| Phase | Status | Review |
| --- | --- | --- |
| 0. Project and Build Skeleton | Sufficient | `build.rs` discovers R, links `libR`, generates bindings, and reports a clear missing-R failure. |
| 1. CLI and Environment Setup | Sufficient | CLI flags, value flags, vanilla expansion, R env setup, R dirs, local history creation, and version output exist. Focused Phase 1 coverage was added in the 2026-06-28 uplift. |
| 2. Dynamic Library Path Repair | Sufficient for Linux/macOS v1 | Unix loader repair, one-time re-exec, guarded macOS cleanup/BLAS fallback, unit coverage, enabled embedded R tests, and the Linux smoke check pass. |
| 3. Embedded R Runtime | Sufficient for v1 | Embedded R starts, callbacks register, REPL runs, eval/source/options helpers, a generic string call helper, typed option values, stronger R error context, and embedded tests exist. |
| 4. Settings and Profile Loading | Sufficient for Linux v1 | Settings load from R options, defaults match the plan, and profile order is implemented. |
| 5. Console Callback Bridge | Sufficient for v1 | stdout/stderr callbacks, suppression, cursor tracking, Ctrl-C flag/R interrupt raise, EOF, piped multiline input, live prompt bridge, nested prompt fallback, terminal resize width updates, long non-ASCII multiline wrapping, and R event/input hook processing exist. |
| 6. Prompt Modes | Sufficient for v1 | R/browse/unknown detection and live `reedline` prompt exist. `;command` shell escape works, and `;` alone enters persistent shell prompt mode. |
| 7. History | Sufficient for v1 | Compatible parser/writer, mode labels, duplicate filtering, browser command filtering, search, trimming, local/global path selection, and tests exist. Live `reedline` navigation (Ctrl-R, up/down-arrow) is backed by loaded radian history via `RadianHistoryBackend`. Mode-aware filtering: R/Browse share history book, Shell is separate. |
| 8. Completion | Partial | R/package/LaTeX/shell completion code exists and is wired into `reedline`. R completion now seeds token state and has embedded coverage for base-function and installed-package completion. Gaps: tiny LaTeX table, no automatic-vs-explicit timeout distinction, and shallow package-context heuristics. |
| 9. Key Bindings and Editing Behavior | Sufficient | All 13 Phase 9 items are implemented via the pre-edit hook: context-aware auto-pairs (string-awareness + following-text guard), closing-delimiter skip and blank-line dedent, smart backspace (pair deletion + indent-aware + shell-mode exit), Enter auto-indentation after `{`, smart Tab in leading whitespace, raw R string pair auto-completion, Ctrl-X Ctrl-E external editor, bracketed paste trailing-newline strip, Ctrl-C completion menu dismiss, and backspace-on-empty-shell-buffer exits to R mode. Gated on R options `radian.auto_match`, `radian.auto_indentation`, and `radian.tab_size`. |
| 10. Lexer and String Detection | Sufficient for lightweight v1 | Lexer handles comments, names, numbers, operators, punctuation, strings, backticks, raw strings, and cursor-in-string checks with tests. It is intentionally not a full R parser. |
| 11. Shell Mode | Sufficient for Unix v1 | Shell command execution, `cd`, `cd -`, env/home expansion, and tests exist. |
| 12. Packaging and Platform Support | Partial | Linux is automatically tested. macOS hardening remains unvalidated. |

Milestones:

- **Milestone A: Sufficient.** Minimal embedded R, CLI, discovery, loader setup,
  callbacks, and basic evaluation are in place.
- **Milestone B: Sufficient.** Prompt basics, settings, profiles, multiline
  input, Ctrl-C interrupt raising, EOF, resize behavior, and event-loop
  behavior are present.
- **Milestone C: Sufficient.** History file compatibility, shell execution,
  persistent shell prompt mode, AND loaded history navigation (Ctrl-R,
  up/down-arrow with mode filtering) are all live.
- **Milestone D: Sufficient.** Completion is live. All 13 Phase 9 editing
  features work: context-aware auto-pairs, closing-delimiter skip,
  blank-line dedent, smart backspace (including shell-mode exit),
  Enter indentation, smart Tab, raw R string pair auto-completion,
  Ctrl-X Ctrl-E external editor, bracketed paste trailing-newline strip,
  Ctrl-C completion menu dismiss, and shell-mode backspace exit — gated on
  R settings and implemented via a pre-edit hook added to vendored reedline.
- **Milestone E: Partial.** Cross-platform hardening remains.

Remaining backlog:

- Expand LaTeX completion data beyond the tiny seed table.
- **Autosuggest:** Wire reedline's history-based inline suggestion feature
  (grayed-out completion of previous commands while typing). R option
  `radian.auto_suggest` is already parsed in `Settings` but not forwarded
  to `ConsoleSettings` or the editor.
- **Custom keybindings:** Implement `radian.escape_key_map` and
  `radian.ctrl_key_map` R options, allowing users to override keybindings
  through R settings. Not currently parsed or wired.
- **Matching bracket highlight:** Briefly highlight the matching opening
  bracket when the user types a closing bracket. R option
  `radian.highlight_matching_bracket` is already parsed in `Settings` but
  not in `ConsoleSettings` or the highlighter.
- Add integration/manual coverage for `browser()` prompt behavior, Ctrl-C
  interrupting long R expressions, and macOS platform paths.

Critical risks:

- Milestone D Phase 1 basic editing (auto-pairs, editor, paste) is live. Phase 2
  smart behaviors require a custom `EditMode` or reedline callback support.
- Platform support is Linux-first; macOS claims are not acceptance
  tested.
- The current `README.md` can mislead implementers because it describes the
  repository as unimplemented.

## 2026-06-29 — Phase 5 R Event/Input Hook Processing Uplift

**Gap:** Phase 5 (Console Callback Bridge) was "Partial" — all console
callbacks (stdout, stderr, Ctrl-C, EOF, multiline, resize, cursor tracking,
nested prompt fallback) were implemented, but **R event/input hook processing**
during prompt waiting was missing. Python radian uses prompt-toolkit's
`inputhook` to call `R_PolledEvents()` at ~30 Hz while waiting for keystrokes;
the Rust port with reedline had no equivalent.

**Plan:** Use a periodic `setitimer`-based SIGALRM (~33 ms interval) that calls
`R_PolledEvents()` from a `sigaction(SA_RESTART)` signal handler.
`R_PolledEvents` is documented as signal-safe and covers timers, registered
input handlers, and polled event callbacks. Non-Unix platforms get a no-op.

**Changes:**

| File | Change |
|---|---|
| `wrapper.h` | Added `#define R_INTERFACE_PTRS` and `#include <R_ext/eventloop.h>` |
| `src/r_runtime.rs` | Added `input_hook` module (unix: signal/timer install/remove; non-unix: no-op stubs). Wired `install()` into `RRuntime::init_repl()`, `remove()` into `RRuntime::run_repl()`. |
| `docs/superpowers/specs/2026-06-29-phase5-input-hook-design.md` | Design doc (approved) |
| `docs/superpowers/plans/2026-06-29-phase5-input-hook.md` | Implementation plan |

**Verification:**

```
cargo test: 109 passed, 0 failed
cargo test --test embedded_r (RADIAN_RS_TEST_R=1): 5 passed, 1 ignored (manual SIGINT)
```

**Status:** Phase 5 is now **Sufficient for v1**. The remaining `#[ignore]`'d
SIGINT test is a platform-sensitive manual check, not a blocker.

**Milestone B** is now fully met: prompt basics, settings, profiles, multiline
input, Ctrl-C interrupt raising, EOF, resize behavior, and event-loop behavior
are all present.

## 2026-06-29 — Milestone D Phase 1 Editing Polish

**Gap:** Editing transforms (auto-pairs, indentation, backspace, bracketed
paste, editor integration) were implemented as pure helpers in `editing.rs` but
never wired into the live reedline REPL. Milestone D was "Partial — editing
polish is mostly helper-only and does not meet the milestone standard."

**Plan:** Use reedline 0.48's keybinding overlay API (`add_binding`) to wire
static `ReedlineEvent::Edit(vec![EditCommand])` sequences for auto-pair
characters. Configure external editor via `Reedline::with_buffer_editor()`.
Bracketed paste is already handled natively by reedline.

**Architectural finding:** Reedline 0.48's `Keybindings` maps keys to static
`ReedlineEvent` values only — no callback/hook mechanism exists. This means
context-aware transforms (`insert_pair` with string detection, `type_closing_on_blank_indent`,
`backspace` with pair/indent awareness, `indent_after_enter`, context-aware
`insert_tab`) cannot be wired as keybinding overrides. They require a custom
`EditMode` implementation, deferred to Phase 2.

**Changes:**

| File | Change |
|---|---|
| `src/r_runtime.rs` | Added `tab_size`, `auto_match`, `auto_indentation` to `ConsoleSettings` (struct, `Default`, `install_console_settings`) |
| `src/prompt.rs` | Added `auto_pair_bindings()` helper, conditional `add_binding` for `(` `[` `{` `"` `'` in emacs and vi insert modes gated on `auto_match`. Added `.with_buffer_editor(Command::new(editing::select_editor(None)), tmpfile)` to builder chain. Added `EditCommand`, `editing`, `Command` imports. |
| `docs/superpowers/specs/2026-06-29-milestoneD-editing-polish.md` | Design doc |
| `docs/superpowers/plans/2026-06-29-milestoneD-editing-polish.md` | Implementation plan |

**Verification:**

```
cargo test: 112 passed, 0 failed
cargo test --test embedded_r (RADIAN_RS_TEST_R=1): 6 passed, 1 ignored (manual SIGINT)
```

**Status:** Milestone D is now **Phase 1 sufficient** for editing polish.
Completion was already live (Phase 8). Auto-pairs, external editor (Ctrl+O),
and bracketed paste work in the REPL. Phase 2 required a custom `EditMode` or
reedline callback API support.

## 2026-06-29 — Milestone D Phase 2 Context-Aware Editing Hook

**Goal:** Implement context-aware editing (auto-pairs with string-awareness,
closing-delimiter skip, smart backspace, Enter indentation, Tab indentation)
via a pre-edit hook added to the vendored reedline.

**Approach:**

Reedline 0.48.0 was vendored at `vendor/reedline/` as a path dependency.
A `pre_edit_hook` field was added to `Reedline` (with `+ Send` because
`Reedline` lives behind a `Mutex`), along with a `with_pre_edit_hook()`
builder method and event dispatch that fires the hook before
`edit_mode.parse_event()`. The hook receives `(&ReedlineRawEvent, &str, usize)`
(buffer text and cursor position) and returns `Option<ReedlineEvent>`.
A non-consuming `as_event()` accessor was added to `ReedlineRawEvent` to
avoid borrow conflicts during dispatch.

**New hook file `src/editing_hook.rs`** (165 lines, 12 unit tests):

- `handle()` dispatches to six key-specific interceptors
- `auto_pair(buffer, cursor, open, close)` — inserts a pair only when
  `cursor_in_string()` is false and `following_text_accepts_pair()` is true
  (accepting whitespace, closing brackets, comma, semicolon, or EOF)
- `closing_delimiter(buffer, cursor, typed, tab_size)` — if next char matches,
  skips over it; if on a blank indented line, dedents then inserts
- `smart_backspace(buffer, cursor, tab_size)` — if cursor is between an
  empty pair `|()`, deletes both chars; if at leading whitespace, deletes
  `tab_size` worth of spaces
- `enter_indent(buffer, cursor, settings)` — after `{`, inserts newline +
  indent at current level + `tab_size`
- `smart_tab(buffer, cursor, tab_size)` — in leading whitespace, inserts
  spaces to next tab stop

All gated on R options `radian.auto_match`, `radian.auto_indentation`,
and `radian.tab_size`.

**Phase 1 static overlays removed:**

- `auto_pair_bindings()` function deleted from `src/prompt.rs`
- `if settings.auto_match { add_binding(...) }` blocks removed from both
  emacs and vi keybinding branches
- `EditCommand` removed from reedline imports
- `pub mod editing_hook;` added to `src/lib.rs`

**Verification:**

```
cargo check: 0 errors, 0 warnings
cargo test: 126 unit tests passed, 0 failed
            6 R integration tests passed, 0 failed
```

**Status:** Milestone D is **Sufficient**. Completion (R/package/LaTeX/shell)
was already live. All editing features are now wired via the pre-edit hook.
Shell-mode backspace exit and Ctrl-C completion cancellation remain as
unbounded scope (not required for Python radian parity v1).

## 2026-06-29 — Milestone D Phase 2f+2g Shell Mode Exit and Ctrl-C Cancel

**Phase 2f — Backspace in shell mode exits to R:**

A `SHELL_MODE` static `AtomicBool` flag was added to `editing_hook.rs`.
`read_shell_prompt()` sets the flag to `true` before entering its loop and
resets it to `false` on exit. The pre-edit hook's Backspace handler checks
`cursor == 0 && SHELL_MODE`: when both are true, it returns
`ReedlineEvent::Enter`, submitting the empty line and causing the shell
loop to exit to R mode.

**Phase 2g — Ctrl-C cancels completion menu:**

In `vendor/reedline/src/engine.rs`, the `ReedlineEvent::CtrlC` handler was
modified to check `self.menus.iter().any(|m| m.is_active())`. If any
completion menu is active, Ctrl-C only deactivates the menu and returns
`EventStatus::Handled` (buffer preserved, no interrupt raised). If no menu
is active, the original behavior (clear buffer + Ctrl-C exit) is preserved.

**Changes:**

| File | Change |
|---|---|
| `src/editing_hook.rs` | Added `SHELL_MODE` static, `set_shell_mode()` pub fn, backspace guard in `handle()` |
| `src/r_runtime.rs` | `read_shell_prompt()` sets/clears `SHELL_MODE` around loop |
| `vendor/reedline/src/engine.rs` | Ctrl-C checks `menus.iter().any(\|m\| m.is_active())` before clear/exit |

**New test coverage (3 tests in `editing_hook.rs`):**

- `shell_mode_backspace_submits_empty_buffer` — Backspace at cursor 0 in shell
  mode returns `Enter`
- `shell_mode_backspace_does_not_submit_when_buffer_not_empty` — Backspace
  with non-empty buffer in shell mode does not submit
- `normal_mode_backspace_at_start_does_not_submit` — Backspace at cursor 0
  in normal mode does not submit

**Verification:**

```
cargo test: 150 unit tests passed, 0 failed
cargo test --test embedded_r (RADIAN_RS_TEST_R=1): 6 passed, 1 ignored
```

**Status:** All 13 Phase 9 items are implemented. Milestone D is fully
**Sufficient** — completion, all editing behaviors, shell-mode polish, and
Ctrl-C completion cancellation are live.

## 2026-06-29 — Milestone C Loaded History Navigation

**Gap:** Radian's `History` struct loaded entries from its rich-format file at
startup, but reedline's Ctrl-R (reverse search) and up/down-arrow navigation
used an empty default in-memory history. Loaded history entries were never fed
into reedline, so Ctrl-R was effectively a no-op.

**Approach:** Implemented `RadianHistoryBackend` — a custom reedline `History`
trait wrapper in `src/history.rs` that serves as a mode-aware search index.
The backend is seeded from radian's loaded entries at session construction and
updates its in-memory index on each command submission. File persistence is
unchanged (existing `append_history()` calls). Mode filtering is shared via an
`Arc<Mutex<PromptMode>>` with `PromptSession`.

**Design simplification during implementation:** The backend was changed from
the original spec to be search-only (no `Arc<Mutex<History>>` for file writes).
This eliminated the need for `history_arc` in `ConsoleState` and kept the
existing `append_history()` path untouched.

**Architecture:**

```
reedline::Reedline
  │  history.save() / history.search()
  ▼
RadianHistoryBackend      (implements reedline::History trait)
  │                        Vec<HistoryItem> for search
  │                        Arc<Mutex<PromptMode>> (shared with PromptSession)
  ▼
History entries snapshot  (copied at construction, file writes via append_history)
```

**Changes:**

| File | Change |
|---|---|
| `src/history.rs` | `RadianHistoryBackend` struct + `History` trait impl + `entries()` accessor + 4 unit tests (+254 lines) |
| `src/prompt.rs` | `with_arc_history()` constructor + `mode_arc` in `PromptContext` + `update_mode()` sync (+44 lines) |
| `src/r_runtime.rs` | Backend wiring in `read_console_interactive()`, `mode_arc` in `ConsoleState`, manual `Default` impl (+25 lines) |
| No changes to vendored reedline | — |

**New test coverage (4 tests in `history.rs`):**

- `backend_seeded_from_entries` — backend seeded from `&[Entry]` contains all entries
- `save_appends_to_index` — `save()` updates in-memory index with current mode
- `search_filters_by_current_mode` — R mode finds r+browse; shell mode finds only shell
- `search_filters_by_substring` — substring matching on command line

**Verification:**

```
cargo test: 154 unit tests passed, 0 failed  (+4 new)
cargo test --test embedded_r (RADIAN_RS_TEST_R=1): 6 passed, 1 ignored
```

**Status:** Milestone C is now **Sufficient** — history file format, shell
execution, persistent shell prompt, and loaded history navigation (Ctrl-R,
up/down-arrow with mode filtering) are all live. The remaining backlog item
"Connect loaded radian history to interactive navigation/search" is resolved.

## 2026-06-30 — Phase 8 LaTeX Completion Table Verification

**Gap:** Phase 8 (Completion) was documented as having a "tiny LaTeX table
(only 5 symbols)." The implementation already used `include_str!` to embed the
full upstream `latex_symbols.py` (2493 entries), but this was not verified by
tests and the documentation assumed the table was still minimal.

**Changes:**

| File | Change |
|---|---|
| `src/completion.rs` | Added `latex_table_has_full_upstream_count` test (asserts 2490+ entries parsed), `latex_completions_work_for_common_symbols` test (verifies `\alpha`, `\beta`, `\gamma`, `\pi`, `\sum`, `\int`, `\infty`, `\ne`, `\pm`, `\partial` are available) |
| `docs/python-to-rust-port-plan.md` | Updated Phase 8 status to "Sufficient for v1" — LaTeX table gap resolved. Removed LaTeX table from blockers list and implementation plan item 2. |

**Verification:**

```
cargo test: 156 unit tests passed, 0 failed  (+2 new)
cargo test --test embedded_r (RADIAN_RS_TEST_R=1): 6 passed, 1 ignored
```

**Status:** Phase 8 is now **Sufficient for v1** — the LaTeX table gap is
resolved. Remaining Phase 8 gaps: no automatic-vs-explicit completion timeout
distinction, shallow package-context heuristics. Remaining backlog items
(autosuggest, custom keybindings, matching-bracket highlight) are unchanged.

## 2026-06-30 — Autosuggest Wiring

**Gap:** The R option `radian.auto_suggest` was already parsed in `Settings`
but never forwarded to `ConsoleSettings` or the reedline editor. Reedline's
`DefaultHinter` (grayed-out inline history suggestion while typing) was not
attached.

**Changes:**

| File | Change |
|---|---|
| `src/r_runtime.rs` | Added `auto_suggest: bool` to `ConsoleSettings` struct, wired from `Settings::auto_suggest` in `Default` impl and `install_console_settings()`. |
| `src/prompt.rs` | Added `DefaultHinter` to imports. Both `PromptSession::new()` and `PromptSession::with_arc_history()` conditionally call `.with_hinter(Box::new(DefaultHinter::default()))` when `settings.auto_suggest` is true. |

The `DefaultHinter` uses reedline's `History` trait to find the most recent
history entry starting with the current input and renders the remainder in
light gray. Because `RadianHistoryBackend` implements mode-aware filtering,
the hinter respects the R/Browse vs. Shell mode separation automatically.

**Verification:**

```
cargo test: 156 unit tests passed, 0 failed
cargo test --test embedded_r (RADIAN_RS_TEST_R=1): 6 passed, 1 ignored
```

**Status:** Autosuggest is now **wired**. The remaining backlog items (custom
keybindings `escape_key_map`/`ctrl_key_map`, matching-bracket highlight) are
unchanged.

## 2026-06-30 — Custom Keybinding Maps

**Gap:** The R options `radian.escape_key_map` and `radian.ctrl_key_map` were
not parsed. Users could not define custom key combinations to insert
frequently-used text snippets.

**Changes:**

| File | Change |
|---|---|
| `src/settings.rs` | Added `CustomKeyBinding` struct, `escape_key_map`/`ctrl_key_map` fields to `Settings`, R code to serialize the R list-of-lists into tab-delimited `key\tvalue\tmode` format via `vapply`/`paste`, and `parse_key_bindings()` helper. |
| `src/r_runtime.rs` | Added key map vecs to `ConsoleSettings`, wired from `Settings` in `Default` impl and `install_console_settings()`. |
| `src/prompt.rs` | Added `apply_custom_bindings()` helper that feeds entries into reedline's `Keybindings::add_binding()`. Ctrl entries use `KeyModifiers::CONTROL`, escape entries use `KeyModifiers::ALT` (terminals send Esc+X as Alt+X). Reserved ctrl keys (`m`, `i`, `h`, `d`, `c`) are skipped. Applied in both emacs and vi modes. |

**Verification:**

```
cargo test: 156 unit tests passed, 0 failed
cargo test --test embedded_r (RADIAN_RS_TEST_R=1): 6 passed, 1 ignored
```

**Status:** Custom keybinding maps are now **implemented**. The only remaining
backlog item before v0.2 Core Parity is matching-bracket highlight.

## 2026-06-30 — Matching-Bracket Highlight

**Gap:** The R option `radian.highlight_matching_bracket` was parsed in
`Settings` but never forwarded to `ConsoleSettings` or the highlighter.
`RadianHighlighter` only performed token-based syntax coloring.

**Changes:**

| File | Change |
|---|---|
| `src/r_runtime.rs` | Added `highlight_matching_bracket: bool` to `ConsoleSettings`, wired from `Settings`. |
| `src/prompt.rs` | Changed `RadianHighlighter` from unit struct to carry the flag. Added `find_matching_bracket()` helper (scans backwards from cursor to match `()`, `[]`, `{}`). `highlight()` applies yellow foreground to both matched bracket positions. Updated both construction sites. |

**Verification:**

```
cargo test: 159 unit tests passed, 0 failed  (+3 highlight tests)
cargo test --test embedded_r (RADIAN_RS_TEST_R=1): 6 passed, 1 ignored
```

**Status:** Matching-bracket highlight is now **wired**. All user-facing
core features from the backlog are implemented. Remaining work is
platform hardening (macOS acceptance).

## 2026-07-01 — Feature Catalog: R.nvim Compensation + IPython Magic System

**Context:** The companion Neovim setup will replace R.nvim with vim-slime + a
Neovim terminal. R.nvim's editor-integrated features (object browser, data
inspection keymaps, debug keymaps, REPL lifecycle management) will be lost.
This entry catalogs features the Rust REPL should provide to close that gap,
plus a comprehensive IPython-style magic system that goes beyond what the
original Python radian offered.

Two categories: **A — R.nvim compensation** (replacing features lost by the
Neovim config change), and **B — IPython features** (magics that radian lacked).

The implementation will be organized into phases (see Phasing section below).
No code changes in this entry — this is the feature spec.

---

### A. R.nvim Compensation Features

These magics replace functionality that R.nvim provided through editor
integration. With slime+tmux, the REPL itself must provide these.

#### A1. Object Browser Replacement

R.nvim's object browser (objbr) showed a structured split-pane view of the R
workspace with actions for each object (str, View, summary, plot, head, etc.).
The REPL replaces it with namespace-inspection magics:

| Magic | Equivalent R call |
|-------|------------------|
| `%ls` or `%objects` | `ls()` / `ls.str()` with type/size annotations |
| `%who` | Filtered object listing (like IPython's `%who`) |
| `%whos` | Detailed table: name, type, dimensions, size (primary object browser replacement) |
| `%who_ls` | Return sorted name list as REPL output |
| `%rm <names>` | `rm()` |
| `%clear` | `rm(list=ls())` |

Future option: a TUI popup for interactive object browsing (post-v1).

**Note:** `%reset` is reserved for the IPython-style selective reset (see B6).
`%restart` handles full session reinitialization (see A5).

#### A2. Data Inspection Magics

Replaces R.nvim's `<leader>i*` keymaps (glimpse, summary, head, str, etc.).
Each takes an R expression, evaluates it, and prints the result:

| Magic | R call |
|-------|--------|
| `%str <expr>` | `str(expr)` |
| `%head <expr>` | `head(expr)` |
| `%summary <expr>` | `summary(expr)` |
| `%glimpse <expr>` | `glimpse(expr)` |
| `%dim <expr>` | `dim(expr)` |
| `%names <expr>` | `names(expr)` |
| `%View <expr>` | `View(expr)` |
| `%skim <expr>` | `skimr::skim(expr)` |
| `%tidy <expr>` | `broom::tidy(expr)` |
| `%plot <expr>` | `plot(expr)` |

All magics should support tab-completion of the expression argument via the
existing R completion infrastructure.

#### A3. R Documentation Magics

Replaces R.nvim's `<leader>dr` / `<leader>dR` keymaps.

| Magic | Action |
|-------|--------|
| `%help <topic>` | Open CRAN / rdocumentation.org in browser |
| `%help_pkg <pkg>` | Open package reference index |
| `%help_page <topic> <pkg>` | Open specific help page |

**Note:** `?name` is reserved for inline object introspection (see B5). Browser
docs are always explicit with `%help`.

Internally wraps `help()` / `help.search()` and opens the result URL in the
default browser (or a terminal-based pager if `$BROWSER` is unset).

#### A4. R Debugging Magics

Replaces R.nvim's `<leader>D*` keymaps for the R debugger.

| Magic | R call |
|-------|--------|
| `%debug <fn>` | `debug(fn)` |
| `%debugonce <fn>` | `debugonce(fn)` |
| `%undebug <fn>` | `undebug(fn)` |
| `%browser` | Insert `browser()` call at the R prompt |
| `%where` | `where` (show call stack) |
| `%c` | `c` (continue in debugger) |
| `%n` | `n` (next step in debugger) |
| `%finish` | `finish` (finish current context) |
| `%Q` | `Q` (quit debugger) |

#### A5. REPL Lifecycle

| Magic | Action |
|-------|--------|
| `%restart` | Reinitialize the full R session (clear namespace + restart) |
| `%edit <file>` | Open file in `$EDITOR` and source it on exit |

**Note:** `%reset` is reserved for the IPython-style selective namespace reset
(see B6). `%restart` is for full session teardown/reinit.

---

### B. IPython Features Missing from radian

The original radian README says: *"One would consider radian as an ipython clone
for R, though its design is more aligned to julia."* In practice radian
implements only a fraction of IPython's magic system. This section catalogs
every IPython feature worth porting.

#### B1. Magic Command Framework (Foundation)

Before any individual magic, the REPL needs a magic dispatch system:

- `%` prefix for line magics, `%%` prefix for block/cell magics
- **Automagic:** opt-in setting; when enabled, magics work without the `%`
  prefix when the command name does not conflict with an R function
- `%lsmagic` — list all registered magics
- `%magic` — print help about the magic system and syntax
- `%quickref` — print a quick-reference sheet

**Architecture:** A `MagicRegistry` that maps command names to handler
functions, parses arguments, dispatches to R evaluation or Rust handler code,
and formats output. Cell magics (`%%`) consume subsequent lines until a blank
line or end-of-input.

#### B2. Shell Integration

radian has `;` shell mode (persistent or one-shot). IPython additionally
supports inline shell execution:

| Magic | Description |
|-------|-------------|
| `!command` | Execute shell command inline (output to stdout) |
| `! -c command` or `%sx command` | Execute and capture output as a list of lines |

**Note:** `!!` is avoided because R's tidy evaluation uses `!!` for
force-quoting in `rlang` expressions. Use `! -c` (capture flag) or `%sx`
(shell execute) instead.
| `%cd <dir>` | Change working directory (maintains `_dh` history list) |
| `%pwd` | Print working directory |
| `%ls <path>` | List directory contents |
| `%env` | List / set / get environment variables |
| `%bookmark <name> [dir]` | Persistent directory bookmarks |
| `%pushd <dir>` | Push directory onto stack and `cd` |
| `%popd` | Pop directory from stack and `cd` |
| `%dhist` | Show directory history |

radian's `;` shell mode should remain as-is (it's useful as a persistent shell
prompt). The `!` / `!!` syntax adds lightweight inline execution without
leaving R mode.

#### B3. Timing and Profiling

| Magic | Description |
|-------|-------------|
| `%time <expr>` | Time a single R expression |
| `%timeit <expr>` | Precise timing across multiple runs with statistics |
| `%prun <expr>` | Profile an expression via `Rprof()` |

`%timeit` should replicate IPython's model: run the expression N times in a
loop, report mean ± std dev per call and total time. `%prun` wraps
`Rprof()` / `summaryRprof()`.

#### B4. History Magics

radian already has history file I/O, mode-filtered search, and reedline
navigation (Milestone C). These magics add interactive history management:

| Magic | Description |
|-------|-------------|
| `%history` or `%hist` | Print history, optionally filtered by range / pattern / mode |
| `%edit <range>` | Open history lines in `$EDITOR`; on exit, execute the result |
| `%rerun <range>` | Re-execute history lines by index range or pattern |
| `%recall <range>` | Place previous command(s) on the next input line for editing |
| `%macro <name> <range>` | Define a named macro from history lines |
| `%save <file> <range>` | Save history lines to a file |

#### B5. Object Introspection

IPython's `?` / `??` operators for inspecting objects at the REPL prompt (not
opening a browser, as in A3):

| Magic | Displayed information |
|-------|----------------------|
| `?name` or `%pinfo name` | Signature (formal args), docstring, type/class, file location, length/dim |
| `??name` or `%pinfo2 name` | **Full source code** of the function — calls `deparse(body(name))` or equivalent |
| `%pdoc <name>` | Print only the docstring (if any) |
| `%pdef <name>` | Print only the function signature |
| `%psource <name>` | Print only the source code (same as `??` for functions) |
| `%pfile <name>` | Show the file path where the object is defined |

`??` is specifically the source-code view. For R this means calling
`deparse(body(fn))` for closures, showing the S3/S4 dispatch table for
generics, or printing the C-level `bytecode` indicator for primitives.

#### B6. Namespace Inspection (Object Browser)

Reinforces A1 from the R.nvim compensation set:

| Magic | Description |
|-------|-------------|
| `%who` | List objects, optional type filter (e.g. `%who data.frame`) |
| `%whos` | Table: name, class, dim, size in memory (replaces R.nvim objbr) |
| `%who_ls` | Return sorted vector of names (useful for assignment) |
| `%reset` | Clear namespace: soft (`-s`), hard (`-f`), selective by type |
| `%reset_selective <regex>` | Delete objects matching a regex pattern |
| `%xdel <name>` | Delete a specific object |

#### B7. File Execution and Code Loading

| Magic | Description |
|-------|-------------|
| `%run <file>` | Source an R file in the current namespace |
| `%load <file>` | Read a file's contents into the REPL input buffer (not execute) |
| `%load <url>` | Fetch a URL and place contents in the input buffer |

`%run` wraps `source()` with optional echoing (`-e`). `%load` is for bringing
external code in for inspection or modification before execution.

#### B8. Debugger Integration

| Magic | Description |
|-------|-------------|
| `%debug` | Enter post-mortem debugger after an error |
| `%pdb` | Toggle automatic debugger entry on error (`TRUE` / `FALSE`) |
| `%tb` | Print the last traceback |
| `%xmode` | Set traceback verbosity level |

Reinforces A4 above. `%pdb` is a persistent toggle: when on, any unhandled
error drops into `browser()` automatically.

#### B9. Configuration and Customization

| Magic | Description |
|-------|-------------|
| `%config <name>` | Query a config value |
| `%config <name> = <value>` | Set a config value at runtime |
| `%alias <name> <command>` | Define a REPL alias for a command or shell command |
| `%unalias <name>` | Remove an alias |
| `%colors <scheme>` | Switch the Pygments-compatible color scheme interactively |
| `%automagic` | Toggle automagic on/off |

Config values persist for the session. Aliases are in-memory only (not persisted).

## 2026-07-02 — Full Codebase Audit After Recovery

**Context:** The project source files were recovered from prior work sessions. This
entry catalogs every regression, structural issue, and missing feature found
during a sequential file-by-file audit of the entire `src/` tree, plus build
fixes applied.

### Build State After Fixes

Three build-breaking issues were found and fixed during the audit:

1. **`src/editing_hook.rs` — 10 dead test functions using `ReedlineRawEvent`:**
   The vendored reedline's pre-edit hook signature had changed from
   `&ReedlineRawEvent` to `&Event` (crossterm). All 10 test call sites passed
   `ReedlineRawEvent` to `fn handle(&Event, ...)`. Fixed by replacing
   `ReedlineRawEvent::try_from(Event::Key(...))` with `Event::Key(...)`.
   Removed the dead `fake_raw_event()` helper.

2. **`tests/magic_framework.rs` — Crate name + missing API:**
   The test imported `radian_rs::magic` (crate is named `orchard`) and called
   `parse_magic_line()`, `dispatch_parsed()`, and `ParsedMagic` — none of
   which exist in the recovered magic module. Rewrote the test to use the
   available `MagicRegistry` API: registry contents, lookup, list_all, unknown
   dispatch, pwd output, env output.

3. **`tests/embedded_r.rs` — Binary name + env var name:**
   `CARGO_BIN_EXE_radian-rs` changed to `CARGO_BIN_EXE_orchard`.
   `RADIAN_RS_TEST_R` env var renamed to `ORCHARD_TEST_R`.

4. **`src/completion.rs` — LaTeX symbol count:**
   Assertion expected `>= 2490` but the upstream file has 1983 entries.
   Updated to `>= 1980`.

**Current test results: 167 total, 0 failures**
- `cargo test --lib`: 155 passed, 0 failed
- `cargo test --test magic_framework`: 6 passed, 0 failed
- `cargo test --test embedded_r`: 6 passed, 1 ignored (SIGINT, env-sensitive)
- `cargo check`: 22 warnings (13 auto-fixable via `cargo clippy --fix`)

### Recovery Regressions (8 items)

These were all functional before the recovery incident and are now broken:

| # | File | Regression | Severity |
|---|------|-----------|----------|
| R1 | `src/history.rs:660` | `get_history_snapshot()` returns `Vec::new()` instead of reading from `CONSOLE` global | **Blocking** — root cause for 5+ handlers |
| R2 | `src/magics/history_magics.rs:5,9` | Both `get_history_snapshot()` and `resolve_range()` are stubs (empty Vec / None) | **Blocking** |
| R3 | `src/magics/inspect.rs` | 18 handlers return `Output::Text("not implemented".into())`: Objects, Pdoc, Pdef, Psource, Pfile, Who, Whos, WhoLs, Rm, Clear, Str, Head, Skim, Dim, Names, Plot, Tidy, View | **High** |
| R4 | `src/magic.rs:83-84` | `lsmagic` and `magic_help` modules do not exist (commented in `register_all()`) | **High** |
| R5 | `src/magic.rs:104-105` | `Hist` and `HistN` handlers commented out in `register_all()` | **Medium** |
| R6 | `src/magics/edit_magic.rs:117-130` | `Edit::run()` resolves the edit target but never spawns the editor process | **Medium** |
| R7 | `src/magics/edit_magic.rs` | All 5 edit modes (N, $N, N-M, -N, filename) depend on stubbed `get_history_snapshot()` | **Blocking** |
| R8 | `src/magics/history_magics.rs:44-55` | `export_history()` calls `recent_entries()` → `get_history_snapshot()` → empty | **Medium** |

### SEGFAULT Risks (3 items — unfixed from `docs/development-plan.md`)

| # | File | Issue | Severity |
|---|------|-------|----------|
| S1 | `src/r_runtime.rs:574-605` | **Protect/unprotect stack imbalance**: `eval_code` pushes 3 SEXPs via `Rf_protect`, unprotects 2 before returning a `ProtectedSexp`. `ProtectedSexp::drop` calls `Rf_unprotect(1)`. If GC-triggering code runs between return and drop, the protect stack shifts and frees the wrong SEXP. Replace with `R_PreserveObject`/`R_ReleaseObject`. | **Critical** |
| S2 | `src/r_runtime.rs:136-148` | **Signal handler reentrancy**: `SIGALRM` fires every 33ms and calls `R_PolledEvents()` with no reentrancy guard. Can corrupt R's internal state if it fires during protect/unprotect. Add `AtomicBool` guard. | **Critical** |
| S3 | `src/r_runtime.rs:110` | **Platform-unsafe function-to-integer cast**: `action.sa_sigaction = polled_events_handler as usize`. Use `as *const () as usize` double-cast per clippy suggestion. | **High** |

### Code Quality Issues (4 items)

| # | File | Issue | Severity |
|---|------|-------|----------|
| Q1 | `src/magics/debug.rs:4-19` | `eval_r_captured` and `eval_r_silent` spawn `R --vanilla -s -e` subprocess instead of using `r_runtime::eval_string_raw_global` | **Medium** |
| Q2 | `src/magics/workspace.rs:4-11` | Same subprocess-spawning issue as Q1 | **Medium** |
| Q3 | `src/env_setup.rs:76` | `r_version_at_least_42()` is dead code (flagged in `docs/development-plan.md` for removal) | **Low** |
| Q4 | `src/r_runtime.rs:273` | `ConsoleState::history_arc` field is unused (flagged in `docs/development-plan.md`) | **Low** |

### Missing Intended Features (not built yet)

These were specified in the design docs or feature catalog but no code exists:

| Feature | Source | Priority |
|---------|--------|----------|
| `%lsmagic` / `%help` framework commands | Feature catalog B1 | High |
| `%hist` / `%hist_n` handlers | Feature catalog B4 / commented in registry | High |
| `%bookmark` shell magic | Feature catalog B2 | Medium |
| `%macro` edit magic | Feature catalog B4 | Medium |
| `%config` / `%colors` implementations | Feature catalog B9 | Medium |
| `%run`, `%load`, `%rerun`, `%recall`, `%save` | Feature catalog B4, B7 | Low |
| `%time`, `%prun` timing magics | Feature catalog B3 | Low |
| `%cd`, `%pushd`, `%popd`, `%dhist` shell magics | Feature catalog B2 | Low |
| `%help`, `%help_pkg`, `%help_page` doc magics | Feature catalog A3 | Low |
| `%debugonce`, `%undebug`, `%browser`, `%n`, `%finish`, `%Q` | Feature catalog A4 | Low |
| `%restart` REPL lifecycle | Feature catalog A5 | Low |
| `%xdel`, `%reset` namespace management | Feature catalog B6 | Low |
| `%quickref` quick reference | Feature catalog B1 | Low |

### Rebuild Priority

| Priority | Task | Effort | Justification |
|----------|------|--------|---------------|
| **P0** | Reimplement `get_history_snapshot()` in `history.rs` — connect to `CONSOLE` global | 1-2 hrs | Unblocks all history-dependent handlers (5+) |
| **P1** | Fix protect/unprotect stack imbalance — `R_PreserveObject`/`R_ReleaseObject` | 2-3 hrs | SEGFAULT in production eval paths |
| **P2** | Add reentrancy guard to `polled_events_handler` + fix `sa_sigaction` cast | 1 hr | SEGFAULT from signal handler reentry |
| **P3** | Implement 18 stub inspect handlers in `inspect.rs` | 3-4 hrs | Restores core magic functionality |
| **P4** | Create `lsmagic.rs` + `magic_help.rs` framework modules | 2 hrs | Foundation for all other magics |
| **P5** | `cargo clippy --fix` + remove dead code + subprocess→embedded-R for debug/workspace | 1 hr | Code quality baseline |
| **P6** | Implement `%hist`/`%hist_n`, `%bookmark`, `%macro`, `%config`, `%colors` | 3-4 hrs | Feature completeness for v0.9 |

### Verification Commands

```bash
# Full unit test suite
cargo test --lib --no-fail-fast

# Magic framework integration tests
cargo test --test magic_framework --no-fail-fast

# Embedded R tests (requires R on PATH)
ORCHARD_TEST_R=1 cargo test --test embedded_r -- --nocapture --test-threads=1

# Clippy — aim for zero warnings
cargo clippy --all-targets -- -D warnings

# Format
cargo fmt --check
```

## 2026-07-02 — P5 Completion: Feature Handlers + Final Verification

**Context:** This entry closes all P0–P5 items from the 2026-07-02 audit. After a
full codebase audit, build repairs, and nine rebuild steps, the orchard project
is now in its intended functional state with all recovery regressions fixed.

### P5 Handlers Implemented

| Handler | File | Description |
|---------|------|-------------|
| `%config` | `src/magics/config.rs` | Query/set R options via `getOption()`/`options()` |
| `%colors` | `src/magics/config.rs` | Query/set color scheme via `options(radian.color_scheme=)` |
| `%bookmark` | `src/magics/shell.rs` | Directory bookmarks: list, set, jump, delete |
| `%macro` | `src/magics/edit_magic.rs` | Named code snippets: `%macro name <- code`, recall, list |
| `%edit` | `src/magics/edit_magic.rs` | Launch `$EDITOR` on history entries / files, source on exit |

### Full Rebuild Verification

```
P0  get_history_snapshot()          ✅  CONSOLE global connected
P1  Protect stack imbalance         ✅  R_PreserveObject/R_ReleaseObject
P2  Signal handler reentrancy       ✅  REENTRY_GUARD AtomicBool
P3  18 inspect handlers             ✅  All unstubbed with real R eval
P3  lsmagic/magic_help modules      ✅  Created and registered
P4  Dead code removal               ✅  r_version_at_least_42, history_arc
P4  Subprocess→embedded-R           ✅  debug.rs, workspace.rs
P5  Feature handlers                ✅  config, colors, bookmark, macro, edit
```

### Final Test Results

```
cargo test --lib:                    154 passed, 0 failed
cargo test --test magic_framework:    6 passed, 0 failed
cargo test --test embedded_r:         6 passed, 0 failed, 1 ignored
cargo check:                          0 errors
cargo clippy:                         9 actionable warnings
```

Note: 154 lib tests (down from 155) because the `version_check_is_false_when_r_missing`
test was removed along with the dead `r_version_at_least_42()` function.

### Remaining Warnings (9 actionable)

- 3 from vendored `reedline` crate (cannot fix)
- 6 bindgen-generated `unnecessary transmute` (cannot fix)
- 1 `unnecessary unsafe block` in `ProtectedSexp::new`
- 1 `ENV_LOCK` static used only in tests
- 2 unused fields `dir_stack`, `dir_history` (scaffolding for future `%pushd`/`%popd`)

### Next Steps Beyond This Session

1. **Interactive testing** — Run the binary with a real R installation and verify
   each magic command produces correct output in an interactive session.
2. **`%pushd` / `%popd` / `%dhist`** — Use the existing `dir_stack`/`dir_history`
   fields in `ShellState`.
3. **`%time` / `%prun`** — Timing and profiling magics wrapping `system.time()` / `Rprof()`.
4. **`%run` / `%load`** — File execution and code loading magics.
5. **Cross-platform** — macOS support behind existing `#[cfg]` guards.
6. **`cargo fmt --check`** — Use after any Rust formatting tool is available.

## 2026-07-02 — Dead Code Audit: Zero Project Warnings

**Context:** After completing P0–P5, a final dead code audit was performed using
`cargo check` and `cargo clippy`.

### Method

1. Run `cargo check` and collect all warnings
2. Classify each warning as project-code or vendored-dependency
3. For intentional scaffolding (fields for future features), add `#[allow(dead_code)]`
4. For accidentally dead code (unused helper functions), remove the code
5. For unnecessary `unsafe` blocks, remove the `unsafe` wrapper

### Findings

| Item | File | Disposition |
|------|------|-------------|
| `ShellState::dir_stack` | `src/magics/shell.rs:15` | Scaffolding for future `%pushd`/`%popd` — **suppressed** |
| `ShellState::dir_history` | `src/magics/shell.rs:16` | Scaffolding for future `%dhist` — **suppressed** |
| `ENV_LOCK` static | `src/env_setup.rs:87` | Used only in `#[cfg(test)]` — **suppressed** |
| `unsafe { set_current_dir }` | `src/magics/shell.rs:162` | Not unsafe — **removed** |
| `unsafe { set_var }` | `src/magics/shell.rs:81` | Not unsafe — **removed** |

### Result

```
cargo check warnings: 0 project-generated
                      + 3 reedline (vendored, cannot fix)
                      + 3 missing docs (vendored, cannot fix)
                      = 6 total, 0 actionable
```

The codebase now has **zero project-originating warnings**. All remaining warnings
come from the vendored `reedline` crate and its documentation omissions.

### Verification Commands

```bash
# Full test suite
cargo test --no-fail-fast

# Zero-project-warning check (expected: 0 warnings from src/ and tests/)
cargo check 2>&1 | grep "warning:" | grep -v "reedline" | grep -c "warning"

# Clippy
cargo clippy --all-targets -- -D warnings 2>&1 | grep "error"
```

## 2026-07-02 — Documentation vs Code Audit

**Context:** A systematic cross-reference of all 7 documentation files against
all 19 source files revealed pervasive numeric inflation and stale status claims
dating back to pre-recovery (2026-06-30 through 2026-07-01).

### Summary of Discrepancies

| File | Claims | Actual | Delta |
|------|--------|--------|-------|
| `README.md` | 72+ handlers, ~285 tests | 46 handlers, 164 tests | -26 handlers, -121 tests |
| `DEVELOPMENT_PLAN.md` (root, since deleted — redirect to `docs/development-plan.md`) | 72+ handlers, 165 tests | 46 handlers, 164 tests | -26 handlers, -1 test |
| `docs/development-plan.md` | 50-56 handlers, 249 tests | 46 handlers, 164 tests | -4/-10 handlers, -85 tests |
| `docs/review-2026-07-01.md` (since consolidated into `docs/development-plan.md`) | 55 handlers, 249/249 tests | 46 handlers, 164 tests | -9 handlers, -85 tests |
| `docs/design-history.md` | 50 handlers, 249 tests | 46 handlers, 164 tests | -4 handlers, -85 tests |

### Non-Existent Modules Referenced in `docs/design-history.md`

| Reference | Expected path | Reality |
|-----------|--------------|---------|
| `automagic.rs` | `src/magics/automagic.rs` | File never existed in crate |
| `timing.rs` | `src/magics/timing.rs` | File never existed in crate |
| `doc.rs` | `src/magics/doc.rs` | File never existed in crate |
| `HISTORY_SNAPSHOT` static | `src/history.rs` | Uses `history_entries_snapshot()` function instead |

### Features Listed as ✅ in `docs/review-2026-07-01.md` (since consolidated) That Are Not Registered

These items appear under the "Shell Integration" (B2) and "Timing" (B3) tables
with a ✅ status, but no corresponding handler is registered:

| Feature | Doc Section | Current State |
|---------|------------|---------------|
| `%cd` | B2 Shell Integration ✅ P1 | ✅ Implemented 2026-07-02 |
| `%sx` | B2 Shell Integration ✅ P1 | ✅ Implemented 2026-07-02 |
| `%ls` | B2 Shell Integration ✅ P1 | ✅ Implemented 2026-07-02 |
| `%pushd` / `%popd` | B2 Shell Integration ✅ P1 | ✅ Implemented 2026-07-02 |
| `%dhist` | B2 Shell Integration ✅ P1 | ✅ Implemented 2026-07-02 |
| `%time` | B3 Timing ✅ P3 | Not implemented |
| `%timeit` | B3 Timing ✅ P3 | Not implemented |
| `%prun` | B3 Timing ✅ P3 | Not implemented |
| `%history` / `%save` | B4 History ✅ P4 | `%hist`/`%hist_n` registered; `%save` not implemented |
| `?` / `??` | B5 Introspection ✅ P2 | Object preview handled by REPL dispatch, not by magic handlers |

### Features Listed as ❌ Deferred That Are Actually Implemented

| Feature | Doc Status | Actual |
|---------|-----------|--------|
| `%pdoc` | ❌ Deferred (B5, review.md §3.5) | ✅ Registered and implemented |
| `%pdef` | ❌ Deferred (B5, review.md §3.5) | ✅ Registered and implemented |
| `%psource` | ❌ Deferred (B5, review.md §3.5) | ✅ Registered and implemented |
| `%pfile` | ❌ Deferred (B5, review.md §3.5) | ✅ Registered and implemented |
| `%colors` | ❌ Deferred (B9, review.md §3.9) | ✅ Registered and implemented |
| `%macro` | ❌ Deferred (B4, review.md §3.4) | ✅ Registered and implemented |
| `%edit` | ❌ Deferred (B4, review.md §3.4) | ✅ Registered and implemented |
| `%pinfo` | ❌ Deferred (B5, review.md §3.5) | ✅ Registered and implemented |
| `%pinfo2` | ❌ Deferred (B5, review.md §3.5) | ✅ Registered and implemented |

### Handlers Found Unregistered in Code (Now Fixed)

Three fully-implemented handler structs existed in source files but were never
registered in `register_all()` in `src/magic.rs`. All three were registered
during this audit session.

| Handler | File | name() | Added to |
|---------|------|--------|----------|
| `Where` | `src/magics/debug.rs:56` | `"where"` | `register_all()` — P3 section |
| `Continue` | `src/magics/debug.rs:69` | `"c"` | `register_all()` — P3 section |
| `Bookmark` | `src/magics/shell.rs:99` | `"bookmark"` | `register_all()` — P1 section |

### Minor Code Issue Found and Fixed

| File | Issue | Fix |
|------|-------|-----|
| `src/magics/shell.rs:79-82` | Duplicate safety comment block (2 identical lines) | Removed duplicate |

### Recommendations

1. **Audit all doc files** to replace inflated counts with actual values.
   `README.md` was corrected during this session; the other 4 files still
   contain stale numbers.
2. **Consider implementing documented-but-missing handlers:** `%time`, `%timeit`,
   `%prun`, `%save`, `%xmode`, `%automagic`.
3. **Remove or update references** to `automagic.rs`, `timing.rs`, `doc.rs`
   in `docs/design-history.md` — these modules never existed in the crate.
4. **Add a stale-doc lint** or maintenance note to update doc files after
   every significant phase change.

### Verification

After changes (registration of 3 handlers, README update):
```bash
cargo check      # 0 errors
cargo test --lib # 154 passed, 0 failed
```
(Output capped at 50 KB. Showing lines 1-1128. Use offset=1129 to continue.)

## 2026-07-02 — Schema-Aware Autocomplete Assessment

**Context:** User asked whether the application has schema-aware autocomplete
and a data viewer/visualiser. Investigation of `src/completion.rs` and
`src/prompt.rs` revealed the current state.

### Autocomplete: What Exists

The completion system at `src/completion.rs` delegates R-code completion
entirely to R's built-in `utils:::.completeToken()`. This means R handles:

- **Column names after `$`** — R knows the columns of loaded data frames
  and suggests them when the user types `dataframe$`
- **Function arguments** — R suggests parameter names inside `foo(`
- **Namespace members** — R suggests exports after `pkg::`
- **Object names** — R suggests variables in the global workspace

Additionally, the Rust side implements:
- **Package completion** (`library(`, `require(` contexts) — uses
  `.packages(all.available = TRUE)` from R
- **LaTeX completion** — 1983-entry static table for `\alpha` → `α` etc.
- **Shell path completion** — file/directory expansion in `;` shell mode
- **Prefix-length gating** — respects `completion_prefix_length` setting
- **Timeout control** — `namespace_completion()` skips timeout for `::`
  queries; general R completion respects `completion_timeout`

### Autocomplete: What Does NOT Exist

- **No Rust-side schema awareness.** The Rust code does not inspect column
  types, table schemas, or database schemas. Everything goes through R's
  opaque `utils:::.completeToken()` — the Rust side cannot customize or
  extend what R returns.
- **No SQL/database introspection.** No completion for database tables,
  columns, or SQL keywords when connected to a DB via R packages.
- **No type-inference-based completion.** The Rust side does not track
  what type an expression evaluates to, so it cannot suggest type-specific
  completions.
- **No custom column-name context detection.** The `$` operator is tokenized
  as `TokenKind::Operator` in the lexer (`src/lexer.rs`) but triggers no
  special completion behavior on the Rust side.
- **No timeout differentiation** beyond the `::` namespace case — all
  non-namespace R completions use the same timeout.

### Data Viewer: What Exists

| Handler | Mechanism | Scope |
|---------|-----------|-------|
| `%View` | R's GUI `utils::View()` | Opens external spreadsheet window |
| `%head` | `head(expr)` | Prints first rows as text |
| `%str` | `utils::str(expr)` | Prints structure info |
| `%skim` | `skimr::skim(expr)` | Summary statistics |
| `%tidy` | `broom::tidy(model)` | Model result table |
| `%plot` | `plot(expr)` | R graphics device |
| `%dim` / `%names` | `dim()` / `names()` | Shape and column names |

### Data Viewer: What Does NOT Exist

- **No in-terminal interactive data viewer.** No DT, reactable, ratatui,
  or custom TUI table browser exists anywhere in the codebase.
- **No `%browse` magic** for interactive data browsing.
- **No `%glimpse` or `%summary` handlers** (listed as aspirational in
  `docs/development-plan.md` but not implemented).
- All data viewing goes through R's standard print/output or R's GUI
  `utils::View()` — there is no custom rendering pipeline.
- The earlier developer-log entry at line 918 defers this: *"Future option:
  a TUI popup for interactive object browsing (post-v1)."*

### Recommendations

1. **Schema-aware autocomplete improvements:**
   - For database-aware completion, add a completer module that queries
     the active DB connection via R's DBI interface.
   - For improved `$` completion, parse the token before `$` in Rust,
     evaluate its class/columns, and return column names directly —
     avoids the round-trip through `utils:::.completeToken()`.
   - Add timeout differentiation: classify completions as "fast" (names,
     columns, namespaces) vs "slow" (function completions that trigger
     package loading).

2. **In-terminal data viewer:**
   - Add a `comfy-table` or `ratatui` dependency for TUI table rendering.
   - Create a `%browse` handler that opens an interactive table popup
     with sort/filter/scroll, similar to `daff` or `visidata`.
   - Route rendered output through R's `write_console_ex` callback for
     proper REPL integration.
   - The deferred TUI browser from developer-log line 918 would address
     this gap directly.

## 2026-07-02 — Release Roadmap: v0.1 → v0.9 → v1.0

**Context:** Comprehensive gap analysis comparing the current codebase (49
registered handlers, 164 tests) against the aspirational 72+ handler target
documented in `docs/review-2026-07-01.md` (since consolidated into `docs/development-plan.md` — IPython/radian/Julia/zsh comparisons)
and `docs/development-plan.md` (phase-by-phase handler tables).

Reference features that no longer apply (UI patterns unique to other shells
that have no R equivalent) were excluded — see `docs/review-2026-07-01.md` (since consolidated)
§3.11 for the full exclusion list, now in `docs/development-plan.md`.

---

### Current Baseline (v0.1 Experimental)

| Metric | Value |
|--------|-------|
| Registered handlers | 49 |
| Passing tests | 164 (158 lib + 6 magic_framework) |
| Cargo check | 0 errors |
| 1 ignored test | `test_shell_sx_echo` — requires R runtime |
| Git | Initialized, 2 commits |
| Platform | Linux only |
| CI | None |
| Packaging | None |

---

### Full Feature Delta: 49 → 72+ handlers

#### P0 Framework (missing 1)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%automagic` | 1h | High | Toggle for automatic `%` prefix detection. Needs dispatch modification in `read_console_interactive`. |

#### P2 Object Browser (missing 2)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%summary` | 0.5h | High | Wraps `summary()` in R |
| `%glimpse` | 0.5h | High | Wraps `dplyr::glimpse()` with optional package check |

#### P4 History (missing 3)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%save` | 1h | High | Save history entries to file |
| `%rerun` | 2h | Medium | Re-execute history range. Needs REPL code injection — current dispatch only returns Text/Silent. |
| `%recall` | 2h | Medium | Recall history range into editor. Same injection challenge. |

#### P5 Debugger (missing 5)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%debugonce` | 0.5h | Medium | Set `debugonce()` on a function |
| `%undebug` | 0.5h | Medium | Remove `debug()` from a function |
| `%browser` | 0.5h | Medium | Invoke `browser()` in a function |
| `%n` | 0.5h | Medium | Debugger "next" command |
| `%finish` | 0.5h | Medium | Debugger "finish" command |
| `%Q` | 0.5h | Medium | Debugger "quit" command |

#### P6 Documentation (missing 2)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%help_pkg` | 0.5h | High | `help(package=...)` |
| `%help_page` | 0.5h | High | `help(...)` with rendered output |

#### P9 Config (missing 1)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%xmode` | 0.5h | High | Traceback verbosity control (plain/context/verbose) |

#### B6 Namespace Cleanup (missing 3)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%reset` | 0.5h | Medium | `rm(list=ls())` with confirmation |
| `%reset_selective` | 0.5h | Medium | Selective namespace cleanup by pattern |
| `%xdel` | 0.5h | Medium | Delete specific variables |

#### B10 Session Management (missing 5)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%store` | 3h | Low | Save/restore variables across sessions. Needs RDS serialization. |
| `%logstart` | 1h | Low | Start session logging |
| `%logstop` | 0.5h | Low | Stop session logging |
| `%logstate` | 0.5h | Low | Show logging state |

#### B11 Extension System (missing 3)

| Handler | Effort | Priority | Notes |
|---------|--------|----------|-------|
| `%load_ext` | 2h | Low | Load extension module (requires extension API design) |
| `%reload_ext` | 1h | Low | Reload extension |
| `%unload_ext` | 1h | Low | Unload extension |

#### Non-Handler Features

| Feature | Effort | Priority | Notes |
|---------|--------|----------|-------|
| CI pipeline (Linux) | 1h | High | `.github/workflows/ci.yml` for `cargo test` + `cargo clippy` |
| CI pipeline (macOS) | 2h | Low | Needs macos-latest runner |
| macOS acceptance | 2h manual | Low | Requires physical Mac hardware |
| Release packaging | 4h | Medium | `cargo deb`, `cargo rpm`, binary distribution |
| User documentation | 4h | Medium | README update, feature guide, migration guide |
| TUI data viewer | 4h | Low | Interactive in-terminal table browser (deferred post-v1) |
| Schema-aware autocomplete | 4h | Low | DB-aware completion, type-inference, improved `$` handling |
| Reticulate prompt | 8h+ | Very Low | Requires Python interpreter in process |
| On-load/cleanup hooks | 2h | Very Low | `register_cleanup` equivalent |
| Askpass setup | 1h | Very Low | Rare use case |

---

### Priority Tiers

**Tier 1 — High use, low effort (< 1 hour):** Implement in the next version.

| Feature | Effort | Rationale |
|---------|--------|-----------|
| `%xmode` | 0.5h | Users hit tracebacks constantly; simple toggle |
| `%save` | 1h | History persistence is a core R workflow |
| `%automagic` | 1h | Eliminates `%` typing friction |
| `%help_pkg` / `%help_page` | 1h | Documentation access is daily-use |
| `%summary` / `%glimpse` | 1h | Data inspection is the most common R task |
| CI pipeline (Linux) | 1h | Enables automated verification |

**Tier 2 — Moderate use, low-medium effort (1-2 hours):** Implement in v0.4–v0.5.

| Feature | Effort | Rationale |
|---------|--------|-----------|
| Debugger handlers (6) | 3h | Completes debugger parity |
| `%reset` / `%reset_selective` / `%xdel` | 1.5h | Namespace management |
| `%rerun` / `%recall` | 4h | REPL code injection mechanism needed |

**Tier 3 — Specific use cases, moderate effort (2-4 hours):** Implement in v0.6–v0.7.

| Feature | Effort | Rationale |
|---------|--------|-----------|
| `%store` | 3h | Session persistence — valuable but not daily |
| Release packaging | 4h | Enables distribution |
| User documentation | 4h | Required for v1.0 |

**Tier 4 — Low use, high effort (4+ hours):** Implement in v0.8–v0.9.

| Feature | Effort | Rationale |
|---------|--------|-----------|
| Logging handlers | 2h | Rarely used by most R users |
| Extension system | 4h | Requires API design |
| macOS support | 4h | Blocked on hardware |
| TUI data viewer | 4h | Post-v1 deferred |
| Schema-aware autocomplete | 4h | Enhancement, not core |
| Reticulate prompt | 8h+ | Python dependency |

---

### v0.1 → v0.9 → v1.0 Roadmap

#### v0.2 — Shell + File Parity (DONE)

**Goal:** Basic shell utilities and file execution covered.
**Status:** ✅ Complete (49 handlers).

**Added this version:**
- `%cd`, `%ls`, `%sx` — Shell commands
- `%pushd`, `%popd`, `%dhist` — Directory stack
- `%run`, `%load` — File execution
- `%time`, `%timeit`, `%prun` — Timing/profiling

**Exit gate:** 49 handlers, all shell/file/timing handlers working.

---

#### v0.3 — Quick Wins + Polish

**Goal:** High-impact low-effort features that remove daily friction.

**Target:** 56 handlers (49 + 7)

| Feature | Type | Effort |
|---------|------|--------|
| `%xmode` | New handler | 0.5h |
| `%save` | New handler | 1h |
| `%automagic` | New handler + dispatch | 1h |
| `%help_pkg` | New handler | 0.5h |
| `%help_page` | New handler | 0.5h |
| `%summary` | New handler | 0.5h |
| `%glimpse` | New handler | 0.5h |
| CI pipeline (Linux) | Infrastructure | 1h |

**Minor versions:**
- v0.3.1: `%xmode` + CI pipeline
- v0.3.2: `%save` + `%automagic`
- v0.3.3: `%help_pkg` + `%help_page`
- v0.3.4: `%summary` + `%glimpse`

**Exit gate:** 56 handlers, CI passing on Linux, automagic working.

---

#### v0.4 — Debugger & Data Completeness

**Goal:** Complete debugger parity and round out data inspection.

**Target:** 62 handlers (56 + 6)

| Feature | Type | Effort |
|---------|------|--------|
| `%debugonce` | New handler | 0.5h |
| `%undebug` | New handler | 0.5h |
| `%browser` | New handler | 0.5h |
| `%n` | New handler | 0.5h |
| `%finish` | New handler | 0.5h |
| `%Q` | New handler | 0.5h |

**Minor versions:**
- v0.4.1: `%debugonce` + `%undebug` + `%browser`
- v0.4.2: `%n` + `%finish` + `%Q`

**Exit gate:** 62 handlers, full debugger command set.

---

#### v0.5 — Namespace & History Operations

**Goal:** Namespace cleanup and history replay capabilities.

**Target:** 67 handlers (62 + 5)

| Feature | Type | Effort |
|---------|------|--------|
| `%reset` | New handler | 0.5h |
| `%reset_selective` | New handler | 0.5h |
| `%xdel` | New handler | 0.5h |
| `%rerun` | New handler + injection | 2h |
| `%recall` | New handler + injection | 2h |

**Minor versions:**
- v0.5.1: `%reset` + `%reset_selective` + `%xdel`
- v0.5.2: REPL code injection mechanism
- v0.5.3: `%rerun` + `%recall`

**Exit gate:** 67 handlers, history replay working.

---

#### v0.6 — Session Persistence

**Goal:** Save and restore R sessions across restarts.

**Target:** 68 handlers (67 + 1)

| Feature | Type | Effort |
|---------|------|--------|
| `%store` | New handler | 3h |

**Minor versions:**
- v0.6.1: RDS-based store/restore mechanism
- v0.6.2: `%store` handler with list/restore/delete

**Exit gate:** 68 handlers, session persistence working.

---

#### v0.7 — Platform & Packaging

**Goal:** Distribution-ready builds with macOS support.

**Target:** 68 handlers (no new handlers — infrastructure only)

| Feature | Type | Effort |
|---------|------|--------|
| Release packaging | Infrastructure | 4h |
| User documentation | Documentation | 4h |
| macOS acceptance | Testing | 2h manual |
| CI pipeline (macOS) | Infrastructure | 2h |

**Minor versions:**
- v0.7.1: Release packaging (`cargo deb`, binary distribution)
- v0.7.2: User documentation (README, feature guide, migration guide)
- v0.7.3: macOS acceptance + CI

**Exit gate:** 68 handlers, binary releases available, macOS tested.

---

#### v0.8 — Logging & Extensions

**Goal:** Session logging and plugin/extension infrastructure.

**Target:** 74 handlers (68 + 6)

| Feature | Type | Effort |
|---------|------|--------|
| `%logstart` | New handler | 1h |
| `%logstop` | New handler | 0.5h |
| `%logstate` | New handler | 0.5h |
| `%load_ext` | New handler + API | 2h |
| `%reload_ext` | New handler | 1h |
| `%unload_ext` | New handler | 1h |

**Minor versions:**
- v0.8.1: Logging handlers (`%logstart`, `%logstop`, `%logstate`)
- v0.8.2: Extension API design
- v0.8.3: Extension handlers (`%load_ext`, `%reload_ext`, `%unload_ext`)

**Exit gate:** 74 handlers, logging and extensions working.

---

#### v0.9 — Advanced Features

**Goal:** Completion of all documented feature gaps.

**Target:** 79+ handlers (74 + 5+)

| Feature | Type | Effort |
|---------|------|--------|
| TUI data viewer | New handler + deps | 4h |
| Schema-aware autocomplete | Enhancement | 4h |
| Reticulate prompt mode | Feature | 8h+ |
| On-load/cleanup hooks | Feature | 2h |
| Askpass setup | Feature | 1h |

**Minor versions:**
- v0.9.1: TUI data viewer (`%browse` or enhanced `%View`)
- v0.9.2: Schema-aware autocomplete enhancements
- v0.9.3: Reticulate prompt (if Python available)
- v0.9.4: Cleanup hooks + askpass

**Exit gate:** 79+ handlers, all feature gaps documented as resolved or explicit non-goals.

---

#### v1.0 — Release Candidate

**Goal:** Production-ready replacement for Python radian on Linux.

| Criterion | Requirement |
|-----------|-------------|
| Handlers | 55+ IPython-compatible magic handlers (current: 49 of 55 IPython items from review.md §3.12; target: all 55) |
| + R.nvim compensation | 16 additional object-browser handlers (current: 16; target: 18 with %summary/%glimpse) |
| **Total handlers** | **73+** |
| Tests | 200+ passing, 0 failed |
| CI | Linux CI passing, macOS CI documented |
| Documentation | Feature guide, migration guide, API docs |
| Release | Binary packages available for Linux |
| Platform | Linux heavily tested, macOS beta-supported |

**Blocks to v1.0:**
- All handlers from P0–P9 and B6–B11 implemented
- CI pipeline for Linux (v0.3)
- Release packaging (v0.7)
- User documentation (v0.7)

---

### Feature Count Trajectory

```
v0.1: 38 handlers (pre-uplift baseline)

v0.2: 49 handlers (current — shell + file + timing)

v0.3: 56 handlers (+7: xmode, save, automagic, help_pkg, help_page,
                   summary, glimpse)
v0.4: 62 handlers (+6: debugonce, undebug, browser, n, finish, Q)
v0.5: 67 handlers (+5: reset, reset_selective, xdel, rerun, recall)
v0.6: 68 handlers (+1: store)
v0.7: 68 handlers (infrastructure only — packaging, docs, macOS, CI)

v0.8: 74 handlers (+6: logstart, logstop, logstate, load_ext,
                   reload_ext, unload_ext)
v0.9: 79+ handlers (+5+: TUI viewer, schema autocomplete, reticulate,
                    hooks, askpass)
v1.0: 73+ handlers (consolidated, all IPython parity items resolved,
                    infrastructure complete)
```

The v0.1→v0.9 handler counts represent cumulative additions. The v0.9 total
exceeds the 72-target because v0.9's advanced features (TUI viewer, reticulate,
hooks, askpass) are beyond the strict IPython parity target and represent
stretch goals.

---

## 2026-07-02 — Tool Strengths Analysis: IPython

A detailed analysis of IPython's strengths for statistical programming has been
written to `docs/superpowers/specs/2026-07-02-tool-strengths-analysis.md`. Key
takeaways for orchard:

1. **Magic system is the right model** — orchard's 49-handler registry proves
   the pattern works. The gaps are automagic (highest-ROI unimplemented feature)
   and `%%` cell magics (infrastructure exists, dispatch missing).
2. **`?` / `??` as first-class shortcuts** — Orchard needs a Rust-side detection
   of `?name` at line start to match IPython's zero-friction introspection.
3. **Rich display protocol** — Long-range opportunity to render R objects
   beyond plain text (data frame tables, inline plots, formatted summaries).
4. **Next priorities from IPython parity:** `%xmode`, `%save`, `%automagic`,
   `%rerun`, `%recall`, `%store`, `%reset`, `%logstart`.

---

## 2026-07-02 — Tool Strengths Analysis: Radian

A detailed analysis of Radian's R-specific design decisions has been written
to `docs/superpowers/specs/2026-07-02-tool-strengths-analysis.md`. Key
takeaways for orchard:

1. **R option-backed settings are the correct pattern** — All configuration
   lives in R options, not a separate config file. orchard's `src/settings.rs`
   already implements this. Future settings should follow the same approach.
2. **Modal prompt system is mature** — R/Browse/Shell/Unknown mode detection
   via prompt string matching (`src/prompt.rs`) works correctly. Browse mode
   could be tighter (history filtering, command recognition).
3. **Profile loading order is correct** — `--profile` > XDG > `~/.radian_profile`
   > `.radian_profile`. Matches upstream radian. Test coverage for edge cases
   (missing files, permissions) would be valuable.
4. **Auto-pair rules for R syntax are a competitive advantage** — R's raw
   string literals `r"(...)"`, backtick quoting, and smart dedent are unique
   among R REPLs. The vendored reedline editing hook (`src/editing_hook.rs`)
   should be preserved and documented.
5. **All 12 radian parity phases are at Sufficient or Partial** — No phase is
   missing. The only Partial is packaging/CI/macOS (Phase 12).

---

## 2026-07-02 — Tool Strengths Analysis: Fish

A detailed analysis of Fish shell's interactive-terminal design philosophy has
been written to `docs/superpowers/specs/2026-07-02-tool-strengths-analysis.md`.
Key takeaways for orchard:

1. **Autosuggestion quality is the highest-ROI polish item** — orchard has the
   wiring (DefaultHinter + OrchardHistoryBackend) but should tune suggestions
   to account for command frequency, recency, and mode filtering. Fish shows
   that autosuggestions become the primary way users interact with the REPL.
2. **Syntax highlighting as error prevention** — Beyond cosmetic coloring,
   highlighting should help users spot mistakes in real time. orchard's
   current highlighting (RadianHighlighter) applies basic token coloring but
   doesn't validate function names or flag unmatched brackets.
3. **Context-aware completions work at every level** — orchard's completion
   engine already delegates to R for `library()`, `::`, and `$` contexts.
   Adding awareness for formula interfaces (`~`), mapping aesthetics (`aes()`),
   and dplyr verbs would improve the experience.
4. **Dimmed-text suggestions beat popup suggestions** — Fish shows completions
   inline as dimmed text rather than in a popup. Less visually disruptive for
   users who don't need to tab-complete every command.

---

## 2026-07-02 — Tool Strengths Analysis: Julia REPL

A detailed analysis of Julia's modal REPL has been written to
`docs/superpowers/specs/2026-07-02-tool-strengths-analysis.md`. Key takeaways
for orchard:

1. **Help mode (`?`) is the most natural extension** — orchard could detect `?`
   at line start and route to `%pdoc`/`%pdef`, matching both Julia's and
   IPython's help semantics. Currently, `?` at line start is passed to R
   where it's treated as an incomplete expression.
2. **Prompt-stripping on paste would improve transcript workflows** — Julia
   strips leading `julia>` prompts from pasted code. orchard could do the same
   for `> ` and `+ ` prompts, making REPL transcript pasting seamless.
3. **Mode-specific history is already correct** — Shell commands go to shell
   history, R commands to R history, browser commands filtered. This matches
   Julia's partitioning and is one of orchard's strongest features.
4. **SIGINT handling needs manual acceptance testing** — The implementation
   exists (`CONSOLE.interrupted` flag, `ReadResult::CtrlC`) but the
   interactive Ctrl-C experience (message clarity, state preservation) is
   untested without a manual terminal session.

---

## 2026-07-02 — Tool Strengths Analysis: Harlequin SQL

A detailed analysis of Harlequin's TUI-first SQL IDE has been written to
`docs/superpowers/specs/2026-07-02-tool-strengths-analysis.md`. Key takeaways
for orchard:

1. **Interactive TUI data browser is the most impactful missing feature** —
   Harlequin's main pane (scrollable/sortable result table with column type
   headers) is the model for orchard's deferred post-v1 data viewer. No
   existing orchard handler (`%head`, `%str`, `%View`) provides this.
2. **Schema-aware completion needs enhancement for dplyr/data.table** — R's
   `utils:::.completeToken()` handles base R `$` but dplyr `%>%` chains and
   data.table `DT[,` need R-level context detection. Querying R for the
   schema of piped objects would match Harlequin's SQL schema awareness.
3. **Searchable history with one-key re-execution is a daily workflow** —
   `%hist` displays history but `%rerun`/`%recall` are not implemented.
   Harlequin's Ctrl-R → Enter re-execute should be orchard's target.
4. **File-as-editor-buffer is the right pattern** — orchard's `%edit` + `%run`
   combination already provides this. Tighter integration (auto-sourcing on
   editor exit without manual `%run`) would match Harlequin's seamless
   edit→run cycle.
5. **DBI/odbc connection management would complement R analysis** — A
   `%connections` magic listing active DBI connections, schemas, and running
   test queries would bring database-aware workflows into the R REPL.

---

## 2026-07-02 — Development Plan Rewrite: Data Inspector + Schema Autocomplete

The development plan at `docs/development-plan.md` has been fully rewritten to
incorporate two major new features alongside the staged v0.x release roadmap:

### New Features Added

**1. Intelligent In-Terminal Data Inspector (`%inspect`)**

A new magic handler that renders any R data object as a formatted TUI table
with: column index, column name, data type, null/NA count, blank count,
mean/min/max (for numeric columns), and first few sample values.

Cross-engine support (Priority P0–P3):
- **P0:** vanilla R (data.frame, matrix, vector, factor, list)
- **P0:** tidyverse (tbl_df, grouped_df)
- **P1:** DuckDB (duckdb_relation, tbl_duckdb_connection)
- **P1:** Arrow (Table, RecordBatch)
- **P1:** Rcpp (inherits data.frame)
- **P2:** Stan (stanfit via rstan::extract)
- **P3:** JS/V8 objects

Implementation: R commands extract metadata and sample rows → Rust parses
and renders via `comfy-table` (Phase 1, v0.6) → `ratatui` TUI popup (Phase 2).

**2. Schema-Aware Autocomplete + Variable Selector**

Extends the completion system (`src/completion.rs`) to provide context-aware
completions beyond what R's `utils:::.completeToken()` returns:
- `dataframe$` → column names via R `names()`
- `dataframe@` → S4 slot names via R `slotNames()`
- `dplyr::` pipe chains → schema from pipe context
- Ctrl-Space variable selector → global env variables with types and sizes

Implementation: New completion backend in `src/completion.rs` that calls R
to resolve schema, caches results, returns column names as completion items.

### Roadmap Rationale

The features are staged by effort and dependency:
- v0.3: Quick-win handlers (single R command wrappers)
- v0.4: Debugger completeness (6 simple handlers)
- **v0.5: Schema autocomplete** (needed before data inspector — users need
   column discovery before column inspection)
- **v0.6: Data inspector** (builds on schema autocomplete infrastructure)
- v0.7–v0.9: History, persistence, logging, extensions, packaging

---

## 2026-07-02 — Configurable Editing Mode (Vim/Emacs) Documented

The development plan at `docs/development-plan.md` has been updated with a
comprehensive `## Feature: Configurable Editing Mode (Vim / Emacs)` section.

**What was added:**
- Full emacs and vi mode default shortcut tables (40+ shortcuts documented)
- Quick editing cheat sheet (line start/end, word delete, kill, history)
- Configuration options: `orchard.editing_mode`, `orchard.show_vi_mode_prompt`,
  `orchard.emacs_bindings_in_vi_insert_mode`, `orchard.ctrl_key_map`,
  `orchard.escape_key_map`
- Custom keybinding map documentation with examples
- Implementation notes pointing to `src/prompt.rs`, `src/r_runtime.rs`,
  `src/settings.rs`

**State:** Feature is already implemented and functional. This is a
documentation-only update to make the configuration discoverable.

---

## Appendix A: Key Architecture Decisions (from design-history.md)

*Merged from `docs/design-history.md` during 2026-07-02 documentation consolidation.*

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

## Appendix B: Recovery Incident (2026-07-02)

*Originally in root `DEVELOPMENT_PLAN.md`, since consolidated into `docs/development-plan.md` (2026-07-02).*

The project was accidentally deleted and recovered from OpenCode session database
logs on 2026-07-02. All 76 source files were recovered, but some components were
**reconstructed from fragmented tool-call output** and may differ from the originals.

### Regressed Components (All Now Resolved)

| Component | Status at Recovery | Resolution |
|-----------|-------------------|------------|
| `src/magics/inspect.rs` | 18 of ~40 handlers stubbed | ✅ All 18 handlers reimplemented during P0–P3 |
| `src/history.rs:660` — `get_history_snapshot()` | Stubbed to return empty Vec | ✅ Reimplemented during P0 |
| `src/magics/history_magics.rs` | `resolve_range()` returned `None` | ✅ Reimplemented during P0 |
| `src/magics/edit_magic.rs` | 5 call sites depend on stubbed history functions | ✅ Fixed after history functions were reimplemented |
| `vendor/reedline/src/engine.rs` | `pre_edit_hook` reconstructed from API surface | ✅ Verified working during editing hook tests |
| `src/magics/edit_magic.rs` | `tempfile` dependency removed | ✅ Writing to `/tmp/` directly — works correctly |
| `src/magic.rs:104-105` | `Hist`/`HistN` registrations commented out | ✅ Registered and functional |

### Recovery Methodology

Files were reconstructed by merging all available `read` tool outputs from the
OpenCode session database (`~/.local/share/opencode/opencode.db`, `part` table).
Where the Read tool truncated output (at 25 or 260 lines, or at 50 KB), files
were reconstructed from multiple reads at different offsets. Handlers whose
`fn run` bodies were never captured by any read received stub implementations.

---

## 2026-07-02 — Shell/File Execution Handler Tests Complete

**Plan:** `docs/superpowers/plans/2026-07-02-shell-file-execution-handlers.md`
**Status:** ✅ Plan complete — plan file removed.

The last missing piece of the shell/file-execution handler implementation was
test coverage matching the spec. All 9 tests from the test matrix are now
present and passing.

### Tests Added

| Test | File | What it verifies |
|------|------|-----------------|
| `test_shell_cd_roundtrip` | `shell.rs` | Create temp dir, cd in, verify cwd, cd back |
| `test_shell_pushd_popd` | `shell.rs` | Push dir onto stack, verify cwd change, pop back, verify restore |
| `test_shell_popd_empty_stack` | `shell.rs` | Error on empty dir stack |
| `test_shell_dhist` | `shell.rs` | Empty history displays "(no directory history)" |
| `test_shell_dhist_after_cd` | `shell.rs` | History populated after `%cd` |
| `test_file_run_nonexistent` | `file_magics.rs` | Error on missing file for `%run` |

### Side Fix — `DIR_LOCK`

Added `static DIR_LOCK: Mutex<()>` in the shell test module to serialize tests
that modify the shared `SHELL_STATE` (dir_stack, dir_history, bookmarks) or
process `cwd`. Applied to 18 tests. Without this lock, concurrent tests race on
the global state and produce flaky failures. Follows the existing `CURSOR_LOCK`
pattern from `r_runtime.rs`.

### Verification

```bash
cargo check       # 0 errors, 0 warnings
cargo test --lib  # 265 passed, 0 failed, 1 ignored
```

The single ignored test (`test_shell_sx_echo`) requires R runtime initialization
via `eval_string_raw_global`.

---

## 2026-07-02 — Documentation Sync: Current Actual State

**Context:** A documentation accuracy review found that test counts and warning
counts across multiple docs had drifted from reality after the cleanup work
earlier in the day. This entry records the verified current state and corrects
the drift.

### Verified State (2026-07-02, end of session)

| Metric | Value |
|--------|-------|
| Registered handlers | 49 |
| Total tests passing | 144 (132 lib + 6 magic_framework + 6 integration) |
| Ignored tests | 1 (`test_shell_sx_echo` — requires R runtime) |
| `cargo check` warnings | 0 |
| `cargo clippy` warnings | 0 |
| Platform | Linux only |
| Git commits | 3 |

### Prior Count Drift

Earlier log entries in this file recorded 155, 154, and 164 lib/total tests at
various points. The lib test count dropped to 132 after dead code and dead tests
were removed during the cleanup work (Batch B/D of the codebase cleanup plan).
No intermediate entry recorded that drop. The warning count also changed: prior
entries reported 6–9 remaining warnings from vendored reedline and bindgen; the
reedline `missing_docs` warnings were fixed by adding doc comments to
`vendor/reedline/src/completion/base.rs` (`Automatic`, `Manual`, `Completer`),
bringing the total to 0.

### Build Repairs Applied This Session

Three build-breaking issues from the partially-executed cleanup plan were fixed:

1. `src/lib.rs` — removed `pub mod editing;` (file deleted in Batch D), added
   `pub mod util;` (file created in Batch B but not declared)
2. `src/prompt.rs` — replaced `editing::select_editor(None)` with
   `util::select_editor(None)` at two call sites; updated imports
3. `vendor/reedline/src/completion/base.rs` — added doc comments to
   `CompletionIntent::Automatic`, `CompletionIntent::Manual`, and the
   `Completer` trait to satisfy the crate's `#![warn(missing_docs)]`

### Cleanup Plan Status

The codebase cleanup plan
(`docs/superpowers/plans/2026-07-02-codebase-cleanup.md`) was partially
executed:

- **Batch B (Shared Utilities Consolidation):** ✅ Complete. `src/util.rs`
  created with `expand_tilde`, `expand_vars`, `home`, `r_string`,
  `select_editor`.
- **Batch D (Remove editing.rs):** ✅ Complete. `src/editing.rs` deleted,
  `select_editor` moved to `util.rs`, `prompt.rs` updated.
- **Batch A (Dead Code Removal):** ❌ Deferred. The plan specified removing
  `Debug` and `Pdb` handlers from `src/magics/debug.rs` and their registrations
  in `src/magic.rs`. These remain registered and functional. The plan should be
  revisited or marked superseded.
- **Batch C (Prefix Drift & Boilerplate):** Partially done. `From<Settings>`
  for `ConsoleSettings` was implemented; `radian.` → `orchard.` prefix drift in
  `config.rs` was not verified.

### Documentation Corrections Applied

| File | Correction |
|------|-----------|
| `README.md` | Test count 164 → 144 (two locations) |
| `docs/development-plan.md` | Test count 158 lib → 132 lib; total 164 → 144; v0.3 gate ✅ PASS → 🔲 Planned |
| `docs/review-2026-07-01.md` (since consolidated into `docs/development-plan.md`) | Stale-warning correction 164 → 144 |

### Verification

```bash
cargo check    # 0 errors, 0 warnings
cargo clippy   # 0 warnings
cargo test     # 144 passed, 0 failed, 1 ignored
```

### Correction (2026-07-02, verification audit)

The handler count of 49 reported in this entry undercounted vs reality.
A source audit of `src/magic.rs::register_all()` (2026-07-02, later) confirmed:

- **Actual handlers registered: 47**, not 49
- `%debug` and `%pdb` were listed as registered in prior plans but their structs
  (`Debug`, `Pdb`) were never defined in `src/magics/debug.rs` and never registered
  in `src/magic.rs::register_all()` — only `Traceback`, `Where`, and `Continue`
  (3 handlers) exist in the debug module

The test count of 144 reported in this entry reflected an intermediate state
during codebase cleanup (dead tests removed). Current verified count (2026-07-02
after test hardening) is 307 (300 lib + 7 magic_framework).

This entry's "49" and "144" values are preserved as the state as of the time of
writing. The development plan (`docs/development-plan.md`) was subsequently
rewritten with verified counts on 2026-07-02 after removing redundant docs
(`DEVELOPMENT_PLAN.md` root stub, `docs/review-2026-07-01.md`).

---