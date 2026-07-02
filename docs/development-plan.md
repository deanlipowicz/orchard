# Development Plan

> **⚠️ STALE DOCUMENT — 2026-07-02 Audit**
> Handler and test counts in this file (56 handlers, 249 tests) are pre-recovery
> estimates and do not match the current codebase (38 registered handlers,
> ~160 tests passing). The handler table below is aspirational — it lists target
> features from the original Python radian port plan, not all of which are
> implemented. See `docs/developer-log.md` § 2026-07-02 — Documentation vs Code
> Audit for the full discrepancy catalog.

**What:** Rust rewrite of `radian`, the R terminal REPL, with IPython-style magic
commands. Replaces upstream Python radian on Linux (macOS pending acceptance).

**Verification:** `cargo test -- --test-threads=1` (249 pass, 1 ignored) — **stale**;
actual count is ~160 tests (154 lib + 6 magic_framework).

---

## Release Gates

| Gate | Claim | Status | Blockers |
|------|-------|--------|----------|
| v0.1 Experimental | Linux Rust REPL usable for basic sessions | ✅ PASS | None |
| v0.2 Core Parity | Core Python radian workflows matched on Linux | ✅ PASS | None |
| v0.3 Platform Beta | macOS beta-supported | 🚫 ABANDONED | No physical Mac hardware available |
| v1.0 Replacement Candidate | Recommended replacement for supported workflows | ❌ BLOCKED | Packaging + docs + deferred features |

---

## Architecture

```
readline/readline → r_runtime::read_console_interactive
  ├── ; shell mode (persistent or one-shot)
  ├── ! inline shell execution
  ├── ?/?? object introspection
  ├── % magic dispatch (47 handlers)
  └── R evaluation (via R C API)
```

**Key files:** `src/r_runtime.rs` (REPL loop, dispatch), `src/magic.rs` (registry, parse),
`src/magics/*.rs` (handler modules), `src/history.rs` (history + snapshot),
`src/prompt.rs` (reedline session), `src/shell.rs` (shell commands).

**Key decisions:**
- Magic dispatch runs in `read_console_interactive` (Rust side, before returning to R)
- `Arc<dyn MagicHandler>` prevents reentrant mutex deadlock (clone Arc, drop lock, call handler)
- `eval_string_raw_global` is the safe public API for R evaluation from handlers
- `HISTORY_SNAPSHOT` and `ALIAS_MAP` use `OnceLock<Mutex<...>>` globals (same pattern as `ShellState`)
- `#![deny(unsafe_op_in_unsafe_fn)]` enforced — all unsafe blocks auditable
- All `unwrap()` calls in production code have safety-rationale comments

---

## Implemented Features (50 handlers, 5 prefixes)

Dispatch order: `;` → `!` → `?`/`??` → `%` → R

### Core REPL (Python radian parity)

| Phase | Function | Status |
|-------|----------|--------|
| 0 | Build skeleton, R discovery, bindgen | ✅ |
| 1 | CLI parsing, `--vanilla`, `--version`, R env vars | ✅ |
| 2 | Dynamic loader path repair (Linux/macOS) | ✅ |
| 3 | Embedded R, callbacks, eval/source helpers | ✅ |
| 4 | Settings via `options()`, profile loading | ✅ |
| 5 | Console bridge: stdout/stderr, Ctrl-C, resize, events | ✅ |
| 6 | Prompt modes: R/Browse/Shell/Unknown | ✅ |
| 7 | History file compat, filtered search, autosuggest | ✅ |
| 8 | Completion: R, packages, LaTeX (2493 symbols), shell | ✅ |
| 9 | Keybindings: auto-pairs, smart backspace, indent, etc. | ✅ |
| 10 | Lexer: string detection, highlighting | ✅ |
| 11 | Shell: `;` mode, `cd`, env expansion | ✅ |

### Magic Commands by Phase

| Phase | Handlers | Description |
|-------|----------|-------------|
| P0 | `%lsmagic`, `%magic`, `%automagic` | Framework: registry, `%%` cell magics, automagic |
| P1 | `!`, `%cd`, `%pwd`, `%ls`, `%env`, `%sx`, `%bookmark`, `%pushd`, `%popd`, `%dhist` | Shell integration |
| P2 | `%objects`, `%who`, `%whos`, `%who_ls`, `%rm`, `%clear`, `%str`, `%head`, `%summary`, `%dim`, `%names`, `%glimpse`, `%skim`, `%tidy`, `%View`, `%plot`, `?`/`??` | Object browser + data inspection |
| P3 | `%time`, `%timeit`, `%prun` | Timing + profiling |
| P4 | `%history`, `%save` | History magics |
| P5 | `%debug`, `%debugonce`, `%undebug`, `%browser`, `%where`, `%c`, `%n`, `%finish`, `%Q`, `%pdb`, `%tb` | Debugger integration |
| P6 | `%help`, `%help_pkg`, `%help_page` | Documentation |
| P7 | `%config`, `%alias`, `%unalias` | Configuration |
| P8 | `%pdoc`, `%pdef`, `%psource`, `%pfile` | Object introspection (deferred) |
| P9 | `%xmode` | Traceback verbosity |
| P10 | `%colors` | Theme switching (default, monokai, solarized, none) |

**Total: 56 handlers** — 3 P0 + 9 P1 + 16 P2 + 3 P3 + 2 P4 + 11 P5 + 3 P6 + 3 P7 + 4 P8 + 1 P9 + 1 P10

---

## Remaining Work

**Required for v1.0:**
- CI pipeline (Linux) — ✅ done
- Release packaging
- User documentation

**Deferred magic features:**
- `%pdoc` / `%pdef` / `%psource` / `%pfile` — additional object introspection
- `%xmode` — traceback verbosity
- `%colors` — theme switching
- `%rerun` / `%recall` — history re-execution (needs REPL code injection)
- `%run` / `%load` — file execution
- `%store` — session persistence
- `%reset` / `%reset_selective` — namespace cleanup
- `%macro` / `%edit` — history macros
- `%load_ext` / `%reload_ext` / `%unload_ext` — extension system
- `%logstart` / `%logstop` / `%logstate` — session logging

**Upstream Python radian gaps (low priority):**
- Reticulate prompt integration (needs Python in process)
- Cleanup/finalizer hooks
- Askpass setup
