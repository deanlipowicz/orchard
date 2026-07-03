# Development Plan

**What:** orchard — R without the weight. A terminal-native R REPL that pairs
with your text editor for a git-friendly, file-driven workflow. Write your R,
DuckDB, Stan, and C in files; explore interactively in orchard. Built in Rust as
a ground-up rewrite of the Python radian REPL, with IPython-style magic commands,
schema-aware autocomplete, and an in-terminal data inspector. Linux today, macOS
in progress.

**Current state:** 82 registered magic handlers | ~470 tests | Linux only
**v0.7 delivered:** Editor send-code protocol, `]` package mode, `%import`, `%connections`, `%edit -g`, Revise auto-reload, `%repro`
**Next:** v0.8 (quality of life) → v0.9 (platform + packaging) → v1.0
**Vision (v2.0):** Rich output (SVG/HTML), multithreaded R runtime, plugin architecture

---

## Architecture

```
reedline/readline → r_runtime::read_console_interactive
  ├── ; shell mode (persistent or one-shot)
  ├── ! inline shell execution
  ├── ] package mode (renv/pak sub-loop)
  ├── ?/?? object introspection
  ├── ?/? modal help (lines starting with ?/?? → %pdoc/%psource)
  ├── % magic dispatch (82 handlers)
  ├── Editor socket queue drain (JSON-line over Unix socket)
  ├── Auto-reload file watcher queue drain (notify crate)
  ├── + tab: Schema-aware autocomplete (14 backends) + variable selector ✅
  └── R evaluation (via R C API)

r_runtime → magic_registry → MagicHandler::run() → Output
  ├── Output::Text → display in REPL
  ├── Output::Eval → evaluate in R
  └── Output::DisplayAndEval → display + evaluate

Data Inspector (v0.3):
  R → R commands → column metadata + sample rows
  → Rust table formatter → comfy-table/ratatui rendering → TUI output
```

**Key files:**
- `src/r_runtime.rs` — REPL loop, dispatch, R callbacks, ] package mode, editor/auto-reload queue drains
- `src/magic.rs` — registry, MagicHandler trait, MagicLine
- `src/magics/*.rs` — handler modules (82 handlers)
- `src/editor_bridge.rs` — Unix socket server, JSON-line protocol, --send CLI
- `src/auto_reload.rs` — Filesystem watcher for Revise-style auto-reload
- `src/history.rs` — history + snapshot
- `src/prompt.rs` — reedline session, completer, highlighter
- `src/shell.rs` — shell commands, env lock
- `src/completion.rs` — R/package/LaTeX/shell completion, schema-aware ($/@/[[/%>%), magic arg, function arg, formula ~, fuzzy (SkimMatcherV2), frequency boost, spellcheck, static TSV lookup (datasets + packages)
- `src/frequency.rs` — completion frequency tracker with JSON persistence
- `src/data/dataset_schemas.tsv` — 36 common dataset schemas for zero-FFI column completion
- `src/data/package_symbols.tsv` — 10 packages with function names + argument signatures
- `tests/editor_bridge.rs` — Integration tests for editor send-code protocol

**Key decisions:**
- Magic dispatch runs in `read_console_interactive` (Rust side, before returning to R)
- `Arc<dyn MagicHandler>` clone pattern prevents reentrant mutex deadlock
- `eval_string_raw_global` is the safe public API for R evaluation from handlers
- `OnceLock<Mutex<...>>` globals for shared state (CONSOLE, SHELL_STATE, ALIAS_MAP)
- `#![deny(unsafe_op_in_unsafe_fn)]` enforced — all unsafe blocks auditable
- All `unwrap()` calls in production code have safety-rationale comments

---

## Milestone History

| Milestone | Claim | Status | Key Deliverables |
|-----------|-------|--------|-----------------|
| **A** | Minimal embedded R, CLI, callbacks, basic REPL | ✅ Sufficient | Piped smoke test, embedded R test suite |
| **B** | Prompt, settings, profiles, multiline, event loop | ✅ Sufficient | Timer-based event loop, all 5 sub-items |
| **C** | History, shell mode, loaded navigation | ✅ Sufficient | Compatible parser/writer, mode-filtered search, autosuggest |
| **D** | Completion, keybindings, editing polish | ✅ Sufficient | R/package/LaTeX/shell completion, 13 keybindings, custom keymaps |
| **E** | Cross-platform hardening | 🔴 Not started | Code exists in dyld.rs, untested, ungated; no macOS CI |

---

## Release Gates

| Gate | Claim | Status | Blockers |
|------|-------|--------|----------|
| v0.1 | Experimental Linux REPL | ✅ PASS | None |
| v0.2 | Core radian parity on Linux | ✅ PASS | None |
| v0.3 | EDA core + editor loop | ✅ PASS | 59 handlers, comfy-table inspect |
| v0.4 | History replay + reproducibility | ✅ PASS | 66 handlers, cwd-contextual history |
| v0.5 | Debugger + fuzzy completion | ✅ PASS | 77 handlers, 8 debug handlers, ? modal help |
| v0.6 | TUI inspector + inline plots | ✅ PASS | 79 handlers, TUI popup, inline plots (Kitty/Sixel/iTerm2), %dev, %plots |
| v0.7 | Package mode + editor bridge | ✅ PASS | 82 handlers, editor protocol, ] mode, %import, %connections, %edit -g, auto-reload, %repro |
| v0.8 | Quality of life | 🔲 Planned | See roadmap |
| v0.9 | Platform + packaging | 🔲 Planned | macOS hardware |
| v1.0 | Extensions + release candidate | ❌ BLOCKED | All v0.3–v0.9 gates |

---

## Current Feature Set (77 Handlers)

### Core REPL (Python radian parity — all ✅)

| Phase | Function |
|-------|----------|
| 0 | Build skeleton, R discovery, bindgen |
| 1 | CLI parsing, `--vanilla`, `--version`, R env vars |
| 2 | Dynamic loader path repair (Linux/macOS) |
| 3 | Embedded R, callbacks, eval/source helpers |
| 4 | Settings via `options()`, profile loading |
| 5 | Console bridge: stdout/stderr, Ctrl-C, resize, events |
| 6 | Prompt modes: R/Browse/Shell/Unknown |
| 7 | History file compat, filtered search, autosuggest |
| 8 | Completion: R, packages, LaTeX (1983 symbols), shell |
| 9 | Keybindings: auto-pairs, smart backspace, indent |
| 10 | Lexer: string detection, highlighting |
| 11 | Shell: `;` mode, `cd`, env expansion |

### Magic Commands (82 Registered)

All handlers registered in `src/magic.rs::register_all()`.

| Module | Handlers | Count |
|--------|----------|-------|
| Framework | `%lsmagic`, `%magic` | 2 |
| Shell | `%pwd`, `%env`, `%bookmark`, `%cd`, `%ls`, `%sx`, `%pushd`, `%popd`, `%dhist` | 9 |
| Inspect | `%objects`, `%who`, `%whos`, `%who_ls`, `%rm`, `%clear`, `%str`, `%head`, `%skim`, `%dim`, `%names`, `%plot`, `%dev`, `%plots`, `%tidy`, `%View`, `%pdoc`, `%pdef`, `%psource`, `%pfile`, `%inspect`, `%methods`, `%psearch`, `%connections` | 24 |
| Debug | `%tb` (Traceback), `%where`, `%c` (Continue), `%xmode`, `%debug`, `%pdb`, `%debugonce`, `%undebug`, `%browser`, `%n`, `%finish`, `%Q` | 12 |
| Timing | `%time`, `%timeit`, `%prun` | 3 |
| History | `%hist`, `%hist_n`, `%save`, `%rerun`, `%recall` | 5 |
| Config | `%config`, `%colors`, `%alias`, `%unalias`, `%automagic` | 5 |
| Workspace | `%pinfo`, `%pinfo2`, `%store`, `%reset`, `%xdel` | 5 |
| Edit | `%macro`, `%edit` | 2 |
| File | `%run`, `%load`, `%import`, `%repro` | 4 |
| EDA | `%summary`, `%glimpse`, `%describe`, `%missing`, `%corr`, `%freq`, `%compare`, `%sessioninfo` | 8 |
| Logging | `%logstart`, `%logstop`, `%logstate` | 3 |
| **Total** | | **82** |

**Dispatch order:** `;` → `?` → `%` → R

---

## Feature: Configurable Editing Mode (Vim / Emacs)

Orchard supports both vim and emacs-style editing modes, configurable at
runtime via R options with no restart required.

### Configuration

```r
# Switch between "emacs" (default) and "vi":
options(orchard.editing_mode = "vi")

# Show vi mode indicator in prompt: (I) insert, (N) normal:
options(orchard.show_vi_mode_prompt = TRUE)

# Allow emacs keybindings (Ctrl-A, Ctrl-E, etc.) in vi insert mode:
options(orchard.emacs_bindings_in_vi_insert_mode = TRUE)

# Custom Ctrl-key shortcuts:
options(orchard.ctrl_key_map = list(
  list(key = "k", value = "function()"),
  list(key = "l", value = "lm(")
))

# Custom Alt-key shortcuts:
options(orchard.escape_key_map = list(
  list(key = "p", value = "|> "),
  list(key = "d", value = "%>% ")
))
```

### Emacs Mode Default Shortcuts

Provided by reedline's built-in `Emacs` edit mode:

| Shortcut | Action |
|----------|--------|
| `Ctrl-A` | Move cursor to line start |
| `Ctrl-E` | Move cursor to line end |
| `Ctrl-B` / `Ctrl-F` | Move cursor back/forward one char |
| `Alt-B` / `Alt-F` | Move cursor back/forward one word |
| `Ctrl-D` | Delete character at cursor |
| `Ctrl-H` | Delete character before cursor |
| `Ctrl-K` | Kill to end of line |
| `Ctrl-U` | Kill to beginning of line |
| `Ctrl-W` | Delete word backward |
| `Alt-D` | Delete word forward |
| `Ctrl-P` / `Ctrl-N` | Previous/next history entry |
| `Ctrl-R` | Reverse search history |
| `Ctrl-T` | Transpose characters |
| `Ctrl-Y` | Yank (paste killed text) |
| `Ctrl-L` | Clear terminal |
| `Ctrl-C` | Cancel / send interrupt |
| `Tab` | Trigger autocomplete |

### Vi Mode Default Shortcuts

Provided by reedline's built-in `Vi` edit mode:

**Normal mode (press `Esc` to enter):**

| Shortcut | Action |
|----------|--------|
| `h` / `l` | Move cursor left/right |
| `w` / `b` | Move forward/backward one word |
| `0` / `$` | Move to line start/end |
| `dd` | Delete current line |
| `dw` / `db` | Delete word forward/backward |
| `x` | Delete character at cursor |
| `u` / `Ctrl-R` | Undo / Redo |
| `yy` / `Y` | Yank (copy) line |
| `p` / `P` | Paste after/before cursor |
| `/` / `?` | Search forward/backward in history |
| `i` / `I` | Enter insert mode at cursor / line start |
| `a` / `A` | Enter insert mode after cursor / line end |
| `o` / `O` | Open line below / above |
| `v` / `V` | Characterwise / linewise visual mode |

**Insert mode** (`emacs_bindings_in_vi_insert_mode` adds emacs Ctrl-key shortcuts):

| Shortcut | Action |
|----------|--------|
| `Ctrl-A` / `Ctrl-E` | Move to line start/end |
| `Ctrl-K` / `Ctrl-U` | Kill to end/beginning of line |
| `Ctrl-W` | Delete word backward |
| `Ctrl-P` / `Ctrl-N` | Previous/next history |
| `Esc` | Return to normal mode |

### Quick Editing Use Cases

| Task | Emacs Path | Vi Path |
|------|-----------|---------|
| Start of line | `Ctrl-A` | `Esc` `0` `i` |
| End of line | `Ctrl-E` | `Esc` `$` `a` |
| Delete word backward | `Ctrl-W` | `Esc` `db` `i` |
| Delete word forward | `Alt-D` | `Esc` `dw` `i` |
| Kill to end | `Ctrl-K` | `Esc` `D` `i` |
| Kill to start | `Ctrl-U` | `Esc` `d0` `i` |
| Previous command | `Ctrl-P` | `Esc` `k` `i` |

### Custom Keybinding Maps

```r
# Ctrl+K inserts a pipe operator:
options(orchard.ctrl_key_map = list(
  list(key = "k", value = " |> ")
))

# Alt+P inserts the native R pipe:
options(orchard.escape_key_map = list(
  list(key = "p", value = "|> ")
))
```

Reserved keys (cannot be remapped): Ctrl-M, Ctrl-I, Ctrl-H, Ctrl-D, Ctrl-C.

### Implementation

- Editing mode selection in `src/prompt.rs` — `edit_mode()` function returns `Box<dyn EditMode>`.
- Custom bindings applied via `apply_custom_bindings()` in `src/prompt.rs`.
- Vi mode prompt indicator in `src/r_runtime.rs` — prepends `[I]` or `[N]` to prompt.
- Settings loaded from R options in `src/settings.rs`.

---

## Feature: Schema-Aware Autocomplete + Variable Selector

### Current Completer (existing)

| Context | Source | Status |
|---------|--------|--------|
| R code | `utils:::.completeToken()` | ✅ Working |
| Packages | `.packages(all.available = TRUE)` | ✅ Working |
| LaTeX | 1983-entry static table | ✅ Working |
| File paths | `std::fs::read_dir()` | ✅ Working |

### Schema-Aware Extensions (implemented)

| Context | Detection | Completion Source | Priority | Status |
|---------|-----------|-------------------|----------|--------|
| `dataframe$` | Regex: `\w+\$` | R `names(dataframe)` | High | ✅ Done |
| `dataframe@` | Regex: `\w+@` (S4 slots) | R `slotNames(dataframe)` | Medium | ✅ Done |
| `dataframe[[]]` | Regex: `\w+\[\[` | R `names(dataframe)`, quoted `[["col"` | Medium | ✅ Done |
| `dplyr:: %>`% chain | Regex: `%>%\s*\w+$` | R pipe context eval + names() | Medium | ✅ Done |
| `library()` | Within `library(` context | R `.packages()` | ✅ Done | — |
| Magic args | `%name ` after supported magic | file/dir/variable dispatch | Medium | ✅ Done |
| Function args | Inside `fn_name(` context | R `formals()` with defaults display | Medium | ✅ Done |
| R6 / refClass | `obj$` with R6/refClass objects | `ls(envir=obj)` for R6, filters internal names | Low | ✅ Done |
| Spellcheck | Empty completion, prefix ≥3 chars | Levenshtein distance vs ~2000 R names | Low | ✅ Done |
| Dataset TSV fast path | `obj$` on 36 known datasets | Static column names from TSV (no R call) | High | ✅ Done |
| Package symbol TSV | `pkg::fun` context | Static function names + arg signatures from TSV | High | ✅ Done |
| Frequency ranking | All completion backends | Learned from usage history, persisted to JSON | Medium | ✅ Done |
| `data.table` | Regex: `\w+\[,` | R `names(data.table)` | Low | Future |
| Formula `~` | Within `lm(`, `aov(`, etc. | R `names(data)` from `data =` arg, static TSV fast path | Medium | ✅ Done |
| `DBI::dbGetQuery()` | SQL context detection | DBI connection schema | Future | Future |

### Variable Selector (Implemented)

`%rm` magic, `%clear`, `%who`, and the Manual intent (Tab/Ctrl-Space) completer
all use variable-name completion from the global R environment. The Manual intent
path shows type and size metadata alongside each variable name:

```
mtcars  (data.frame, 7.2 Kb)
lm_model  (lm, 4.5 Kb)
```

### Scored Fuzzy Matching + Frequency Ranking (Implemented)

All completion backends use **scored** fuzzy matching via the `fuzzy-matcher` crate
(SkimMatcherV2 — same engine as fzf). Candidates are scored by substring position,
consecutive character runs, and camelCase boundaries, then ranked by:

```
final_score = skim_matcher_score + frequency_boost
  where frequency_boost = min(count * 50, 500)
```

Frequency data is tracked per-session and persisted to
`~/.local/share/orchard/completion_freq.json` via `src/frequency.rs`.
LaTeX and shell path completions remain prefix-only (intentional).

---

## Feature: Intelligent In-Terminal Data Inspector

Renders any R data object as a formatted table:

| Column | Content | Data Source |
|--------|---------|-------------|
| # | Column index | R `ncol()` / `length()` |
| Column Name | Header name | R `names()` / `colnames()` |
| Type | R class / data type | R `class()`, `typeof()` |
| Missing | Null/NA count | R `sum(is.na())` |
| Blank | Empty string count | R `sum(. == "", na.rm = TRUE)` |
| Mean | Numeric mean (when applicable) | R `mean(x, na.rm = TRUE)` |
| Min | Numeric minimum (when applicable) | R `min(x, na.rm = TRUE)` |
| Max | Numeric maximum (when applicable) | R `max(x, na.rm = TRUE)` |
| First Values | First 3-5 sample values | R `head(x, 5)` |

### Cross-Engine Support

| Engine | R Object Class | Detection | Extraction Priority |
|--------|---------------|-----------|---------------------|
| **Vanilla R** | `data.frame` | `is.data.frame()` | P0 |
| **Vanilla R** | `matrix` | `is.matrix()` | P0 |
| **Vanilla R** | `vector` | `is.vector()` | P0 |
| **Vanilla R** | `factor` | `is.factor()` | P0 |
| **Vanilla R** | `list` | `is.list()` | P0 |
| **tidyverse** | `tbl_df` / `grouped_df` / `spec_tbl_df` | `inherits("tbl_df")` | P0 |
| **DuckDB** | `duckdb_relation` / `tbl_duckdb_connection` | `class()` | P1 |
| **Arrow** | `Table` / `RecordBatch` | `inherits("ArrowObject")` | P1 |
| **Stan** | `stanfit` | `inherits("stanfit")` | P2 |
| **Rcpp** | `Rcpp::DataFrame` | Inherits data.frame | P1 |
| **JS/V8** | JS objects | `class()` contains JS | P3 |

### Implementation Strategy

**Phase 1 — Text table (v0.3, comfy-table):**
- [x] Add `comfy-table` dependency
- [x] Implement R metadata extraction (column names, types, stats, sample values)
- [x] Handle cross-engine detection: DuckDB, Arrow, tidyverse, vanilla R, Stan, Rcpp, JS
- [x] Rust-side table layout engine
- [x] `%inspect <name>` handler

**Phase 2 — TUI popup (v0.6, ratatui):**
- [x] Add `ratatui` dependency
- [x] Interactive scroll, sort by column
- [x] Cell value preview for long content
- [x] Responsive column width auto-sizing

**Phase 3 — Inline plots (v0.6):**
- [x] Terminal protocol detection (Kitty, Sixel, iTerm2)
- [x] PNG device capture via `%plot`
- [x] Kitty graphics protocol rendering
- [x] Sixel rendering (via ImageMagick/ffmpeg)
- [x] iTerm2 inline images protocol

---

## Feature: IPython Parity Coverage

IPython feature categories with current orchard coverage:

| Category | Implemented | Deferred | Total |
|----------|------------|----------|-------|
| B1 Magic framework | 2 (lsmagic, magic) | 0 | 2 |
| B2 Shell integration | 9 (pwd, env, bookmark, cd, ls, sx, pushd, popd, dhist) | 0 | 9 |
| B3 Timing/profiling | 3 (time, timeit, prun) | 0 | 3 |
| B4 History magics | 2 (hist, hist_n) | 4 (save, rerun, recall, macro) | 6 |
| B5 Object introspection | 2 (?/??) | 6 (pinfo/pinfo2/pdoc/pdef/psource/pfile — implemented but via `%` prefix, not `?` shortcut) | 8 |
| B6 Namespace inspection | 3 (who/whos/who_ls) | 3 (reset/reset_selective/xdel) | 6 |
| B7 File execution | 2 (run, load) | 0 | 2 |
| B8 Debugger integration | 3 (tb, where, c) | 1 (xmode — planned v0.3) | 4 |
| B9 Config/customization | 4 (config, colors, alias, unalias) | 0 | 4 |
| B10 Session management | 0 | 4 (store, logstart, logstop, logstate) | 4 |
| B11 Extension system | 0 | 3 (load_ext, reload_ext, unload_ext) | 3 |
| **Total** | **30** | **21** | **51** |

Plus R-specific magics from the inspect module (18 handlers) and edit/file modules
(4 handlers), bringing the total to **58 registered handlers**.

### Missing `%%` Cell Magics

The `MagicLine.is_cell` field exists in parser but no handler dispatches on it.
Planned cell magics (v1.0): `%%timeit` (time multi-line blocks), `%%capture`
(suppress/output, `%%script` (sub-interpreter blocks), `%%writefile`.

---

## Feature: Julia REPL Strengths Integration

### Modal Help (`?`) — Delivered v0.5

Julia's `?` at line start enters dedicated help mode. Orchard detects `?name`/`??name`
at line start and routes through `%pdoc`/`%psource`. A modal `?` (pressing `?` at
column 0 enters help mode, backspace exits) would match Julia's discoverability.

### `]` Package Mode — Planned v0.7

Wraps `renv` (project-local library isolation, snapshot/restore) and `pak` (fast
dependency-aware package installation) in a modal prompt:

```r
] status          # show package status (renv::status())
] init            # initialize renv project
] snapshot        # renv::snapshot()
] restore         # renv::restore()
] install pkg     # pak::pak("pkg")
] remove pkg      # remove.packages("pkg")
] update          # pak::pak_update()
```

Enter `]` at line start to switch to package mode, backspace at column 0 to exit.

### `@edit` / `@less` Jump-to-Source — Planned v0.7

Julia's `@edit f(x)` opens `$EDITOR` at the definition line. R supports this via
`srcref` attributes on functions:

```r
# orchard %edit fn_name — opens $EDITOR at source file:line of function definition
# Uses getAnywhere() + getSrcref() to resolve the source location
```

### Revise.jl-Style Auto-Reload — Planned v0.7

Automatically re-source modified R files detected by filesystem watcher
(notify crate). Toggle via `options(orchard.auto_reload = TRUE)`.

---

## Staged Roadmap

### ✅ v0.3 — EDA Core + Editor Loop (Complete)

**Target:** 59 handlers
**Focus:** Daily-use features for statistical computing and exploratory data analysis.

| Handler/Feature | Description | Effort | Status |
|-----------------|-------------|--------|--------|
| `%summary` | Statistical summary via `summary()` | 0.5h | ✅ Done |
| `%glimpse` | Data glimpse via `dplyr::glimpse()` | 0.5h | ✅ Done |
| `%describe` | Skim-style summary via `skimr::skim()` | 0.5h | ✅ Done |
| `%missing` | Missingness patterns via `naniar::miss_summary()` | 0.5h | ✅ Done |
| `%corr` | Correlation matrix via `cor()` + `corrplot` | 0.5h | ✅ Done |
| `%freq` | Frequency tables via `janitor::tabyl()` | 0.5h | ✅ Done |
| `%compare` | Diff two objects via `waldo::compare()` | 0.5h | ✅ Done |
| `%sessioninfo` | Reproducibility metadata via `sessioninfo::session_info()` | 0.5h | ✅ Done |
| `%xmode` | Traceback verbosity control | 0.5h | ✅ Done |
| `%save` | Save history to file | 1h | ✅ Done |
| `%automagic` | Toggle `%` prefix on magic commands | 1h | ✅ Done |
| `$` / `@` column + pipe completion | R `names(obj)` after `obj$`, `[[`, `%>%` | 2h | ✅ Done |
| `%inspect` text table | comfy-table renderer for any R object (Phase 1) | 6h | ✅ Done |
| CI pipeline (Linux) | GitHub Actions | 1h | ✅ Done |

**Status:** All 14 items complete. 59 handlers delivered.

**Architecture changes:**
- Schema-aware completion backend in `src/completion.rs` calling R to resolve object schema
- `%inspect` handler in `src/magics/inspect.rs` with cross-engine detection
- Pipe chain completion (dplyr `%>%`) as stretch goal

### ✅ v0.4 — History Replay + Reproducibility (Complete)

**Target:** 66 handlers (59 + 7)
**Focus:** Session persistence, history replay, and workspace management.

| Handler/Feature | Description | Effort | Status |
|-----------------|-------------|--------|--------|
| `%rerun` | Re-execute history entries by range | 2h | ✅ Done |
| `%recall` | Recall history into input buffer | 2h | ✅ Done |
| `%store` | Session persistence via RDS serialization | 3h | ✅ Done |
| `%logstart` | Start session logging | 1h | ✅ Done |
| `%logstop` | Stop session logging | 0.5h | ✅ Done |
| `%logstate` | Show logging state | 0.5h | ✅ Done |
| `%reset` | Clean workspace (all or selective by pattern) | 0.5h | ✅ Done |
| `%xdel` | Delete variables with `.last_del` backup | 0.5h | ✅ Done |
| Cwd-contextual history | Tag history entries with working directory, `%hist --dir` filter, prioritized reverse search | 3h | ✅ Done |

**Subtotal:** All 10 items complete.

### ✅ v0.5 — Debugger + Fuzzy Completion (Complete)

**Target:** 77 handlers (66 + 11)
**Focus:** Debugger completeness, fuzzy matching, and modal help.

| Handler/Feature | Description | Effort | Status |
|-----------------|-------------|--------|--------|
| `%debug` | Post-mortem debugger entry | 1h | ✅ Done |
| `%pdb` | Toggle automatic debugger on error | 0.5h | ✅ Done |
| `%debugonce` | Set function to debug once | 0.5h | ✅ Done |
| `%undebug` | Remove debugger from function | 0.5h | ✅ Done |
| `%browser` | Invoke `browser()` at current point | 0.5h | ✅ Done |
| `%n` | Debugger step next | 0.5h | ✅ Done |
| `%finish` | Debugger step out | 0.5h | ✅ Done |
| `%Q` | Debugger quit | 0.5h | ✅ Done |
| Variable selector (`Tab` Manual) | Global env variable browser with type/size metadata | ✅ Done |
| Fuzzy matching | Subsequence-based fuzzy match in all completion backends | ✅ Done |
| Schema-aware completion | `$`/`@`/`[[` column/slot completion via R `names()`/`slotNames()` | ✅ Done |
| Pipe chain completion | `%>%` pipe context: eval expression + `names()` | ✅ Done |
| Magic arg completion | Per-magic file/dir/variable completions (30+ magics) | ✅ Done |
| Function arg completion | R `formals()` with default value display | ✅ Done |
| R6 / refClass method completion | R6: `ls(envir=obj)`, refClass: `names()` | ✅ Done |
| Spellcheck | Levenshtein-based "did you mean?" suggestions | ✅ Done |
| Formula ~ completion | Column names from `data =` arg in lm()/glm()/aov() | ✅ Done |
| `?` modal help | Detect `?` at line start, route to pdoc/psource | 1h | ✅ Done |
| `%methods` | S3/S4 dispatch introspection | 0.5h | ✅ Done |
| `%psearch` | Pattern-based object search | 0.5h | ✅ Done |

**Subtotal:** All items complete.

**Architecture changes implemented:**
- `src/completion.rs` expanded from 4 backends to 14: R, packages, LaTeX, shell, `$`/`@`, `[[`, `%>%`, magic args, function args, R6/refClass, variable selector, spellcheck, formula ~, namespace (`pkg::fun`)
- `src/r_runtime.rs` — early-return `?`/`??` dispatch before magic parsing, routing to `%pdoc`/`%psource`
- Context detection via paren-depth backtracking, operator scanning, and prefix parsing
- `fuzzy_match()` implemented inline via SkimMatcherV2 crate
- Frequency tracking with JSON persistence (`src/frequency.rs`)
- Static TSV fast paths (`src/data/dataset_schemas.tsv`, `src/data/package_symbols.tsv`)

### v0.6 — TUI Inspector + Inline Plots ✅ Delivered

**Target:** 79 handlers (77 + 2)
**Focus:** Rich terminal rendering for data and graphics.
**Status:** ✅ PASS — all features delivered (8 commits, ~1400 new lines)

| Feature | Status | Deliverable |
|---------|--------|-------------|
| `%inspect` ratatui TUI popup | ✅ | Interactive scroll, sort, cell preview (Phase 2) — `src/magics/inspect_tui.rs` (693 lines) |
| Inline plot display | ✅ | Terminal protocol detection (Kitty/Sixel/iTerm2) + PNG capture — `src/terminal_graphics.rs` (355 lines) |
| `%dev` | ✅ | Graphics device management: list, switch, close, capture — `src/magics/inspect.rs` |
| `%plots` | ✅ | Plot history: list, save to file, clear — `src/magics/inspect.rs` |
| R device hook at startup | ✅ | `RRuntime::setup_plot_capture()` redirects all R `plot()`/`ggplot()` to PNG — `src/r_runtime.rs` |

**Foundation gaps addressed:**

| Gap | Severity | Status | Notes |
|-----|----------|--------|-------|
| Ratatui dependency | Blocker | ✅ Done | `ratatui = { version = "0.29", optional = true }` with `tui` feature |
| P4 — Dedup magic dispatch | Low | ✅ Done | Shared `handle_magic_output()` eliminates duplicated `match` in `r_runtime.rs` |
| Cross-platform (Milestone E) | High | ❌ Deferred | Blocked on macOS hardware for acceptance testing |
| Tier 2 debug handler tests | Medium | ⏸ Deferred | `%browser`, `%n`, `%finish`, `%pdb`, `%debugonce`, `%undebug` — needs `debugSource()` setup |
| Interactive recover() tests | Medium | ⏸ Deferred | `%debug`, `%where`, `%c`, `%Q` — needs PTY-based approach |
| Test assertion quality | Low | 🔄 Improved | 17 new tests added across TUI inspect, Dev, Plots, terminal_graphics modules |

**Architecture additions:**
- `src/terminal_graphics.rs` — Terminal protocol detection (`GraphicsProtocol::detect()`), rendering for Kitty (chunked base64), Sixel (ImageMagick/ffmpeg), iTerm2 (OSC 1337), manual base64 encoder, `display_png()` API
- `src/magics/inspect_tui.rs` — ratatui terminal setup, `InspectData` model, TSV parser, `InspectData::sort_by()` (numeric-aware), `run_tui_inspect()` event loop, `ui()` rendering, `centered_rect()` popup helper, 10 tests. Feature-gated behind `tui` flag
- `inspect.rs` — `Inspect::run()` has `#[cfg(feature = "tui")]` branch; `Dev` + `Plots` handler structs; `%plot` captures via `png()` device + `display_png()`; `%dev capture` subcommand
- `r_runtime.rs` — `setup_plot_capture()` sets `options(device = function() png(...))`
- `main.rs` — Calls `runtime.setup_plot_capture()` during startup

### ✅ v0.7 — Package Mode + Editor Bridge (Complete)

**Target:** 82 handlers (79 + 3 new handlers, plus 4 infrastructure features)
**Focus:** Terminal+editor IDE integration and reproducible package management.

| Feature | Description | Effort | Status |
|---------|-------------|--------|--------|
| Editor send-code protocol | Unix domain socket server, JSON-line protocol, `orchard --send "expr"` CLI | 4h | ✅ Done |
| `]` package mode | Modal renv/pak sub-loop (mirrors `;` shell mode) | 4h | ✅ Done |
| `%edit -g` srcref jump | Go-to-definition via R srcref → `$EDITOR +<line> <file>` | 3h | ✅ Done |
| `%import` | Smart data loader: sniff extension → readr/readxl/arrow/haven/jsonlite | 2h | ✅ Done |
| `%connections` | DBI connection browser: list, tables, fields | 3h | ✅ Done |
| Revise-style auto-reload | `notify` crate filesystem watcher, auto-source via `source()` | 4h | ✅ Done |
| `%repro` | Bundle `.R` + renv.lock + sessioninfo into zip | 3h | ✅ Done |

**Actual effort:** ~23h
**Result:** 82 handlers, ~470 tests, 2 new crate deps (`notify`, `zip`)

**Architecture changes delivered:**
- `src/editor_bridge.rs` — Unix socket listener, `EditorRequest`/`EditorResponse`/`EditorJob` types, shared `Mutex<VecDeque>`, JSON-line protocol, `send_code()` client
- `src/r_runtime.rs` — `]` line detection → `read_pkg_prompt()` sub-loop; `dispatch_pkg_command()` for 7 subcommands; editor queue drain at top of `read_console_interactive`; auto-reload queue drain
- `src/magics/connections.rs` — New handler module for DBI browsing
- `src/magics/repro.rs` — New handler module for reproducibility bundles
- `src/auto_reload.rs` — `notify::RecommendedWatcher` recursive watcher, shared queue, `start_watcher()`/`try_recv_reload()`
- `src/cli.rs` — `--send <CODE>` flag
- `src/main.rs` — `--send` client mode, socket listener startup, watcher startup

### v0.8 — Quality of Life

**Target:** 86 handlers (82 + 4)
**Focus:** Snippets, navigation, and workflow polish.

| Feature | Description | Effort |
|---------|-------------|--------|
| zsh-abbrev snippet expansion | User-defined code snippets expand on trigger | 2h |
| zoxide `%z` / frecency jumping | Directory jumping by frequency+recency | 1h |
| Auto-time threshold | Show elapsed time automatically for slow expressions | 1h |
| `%copy` | Copy R expression result to system clipboard | 1h |
| Command-not-found | Suggest `install.packages()` on unresolved functions | 1h |
| Notify-on-completion | Desktop notification when long computation finishes | 0.5h |

**Subtotal:** ~6.5h

### v0.9 — Platform + Packaging

**Target:** 87 handlers (no new handlers — infrastructure)
**Focus:** Cross-platform testing, CI, release packaging, and documentation.

| Feature | Description | Effort |
|---------|-------------|--------|
| macOS acceptance | Manual testing on physical Mac hardware | 2h |
| CI matrix | Linux + macOS GitHub Actions with caching | 4h |
| Release packaging | `cargo deb`, binary distribution | 4h |
| User documentation | README, feature guide, migration guide, API docs | 4h |

**Subtotal:** ~14h

### v1.0 — Extensions + Release Candidate

**Target:** 85+ handlers, 300+ tests
**Focus:** Extension system, cell magics, output caching, and release readiness.

| Feature | Description | Effort |
|---------|-------------|-------|
| `%load_ext` | Load extension module | 2h |
| `%reload_ext` | Reload extension module | 1h |
| `%unload_ext` | Unload extension module | 1h |
| `%%` cell magic dispatch | Multi-line block magics (%%timeit, %%capture, %%script) | 3h |
| `In[]`/`Out[]` caching | Numbered input/output history (IPython-style `_`, `__`, `Out[n]`) | 3h |

**Subtotal:** ~10h

### v2.0 — Rich Output + Concurrency + Extensibility

**Target:** 95+ handlers, 500+ tests, Linux + macOS + Windows
**Focus:** Rich rendering (SVG/HTML), multithreaded R runtime, and a plugin system
that lets users ship their own magic handlers and completion backends.

---

#### Rich Output Rendering

orchard currently emits plain text. v2.0 adds a display pipeline that routes
R output through MIME-type-aware renderers, matching IPython's `_repr_svg_`,
`_repr_html_`, and display system.

| Feature | Description | Effort |
|---------|-------------|-------|
| MIME display dispatch | `DisplayOutput` enum with `Text`, `Html`, `Svg`, `Png`, `Latex` variants | 4h |
| HTML rendering | Terminal HTML via `ratatui` inline widgets or an embedded webview popup | 6h |
| SVG rendering | Rasterize SVG to terminal-appropriate ASCII/braille art, or launch image viewer | 5h |
| `repr_*` protocol | R S3 generics `repr.html()`, `repr.svg()`, `repr.png()` mirroring IPython's `_repr_*_` pattern | 3h |
| Plot display integration | Hook into R's `recordPlot()` / `grid.echo()` to capture ggplot2/lattice plots automatically | 4h |
| `%display` magic | Toggle display backends per session: `%display svg`, `%display html`, `%display text` | 1h |
| `%render` magic | Force-render an object with a specific backend: `%render svg my_plot` | 1h |
| Image cache | Disk cache for rendered outputs with LRU eviction, shared across sessions | 2h |

**Subtotal:** ~26h

**Design constraint:** The display pipeline must not block the REPL. Rendered
output is computed in a background thread and displayed when ready, with the
REPL remaining responsive during rendering. Large plots or HTML tables should
stream incrementally when possible.

---

#### Multithreaded R Runtime

Orchard's current architecture is single-threaded: a single R event loop with
`OnceLock<Mutex<...>>` globals for all shared state. This blocks the REPL during
any long-running R evaluation. v2.0 introduces a session model that isolates R
contexts and enables concurrent evaluation.

| Feature | Description | Effort |
|---------|-------------|-------|
| Session pool manager | `RSessionPool` spawning N R child processes via IPC (stdin/stdout pipe protocol) | 8h |
| `%bg` magic | Background evaluation: `%bg long_running_computation()` → returns a future handle | 4h |
| `%jobs` magic | List active background jobs with status, elapsed time, and memory usage | 2h |
| `%cancel` magic | Cancel a running background job | 1h |
| `AsyncEval` output variant | Non-blocking `MagicOutput::AsyncEval(JobId)` that resolves later | 3h |
| Session isolation | Each session has independent `R_GlobalEnv`, search path, and working directory | 5h |
| Shared memory transfer | Zero-copy data transfer between sessions via `R_Serialize`/`R_Unserialize` over shared memory or Unix domain sockets | 5h |
| Thread-safe completion | Completion caches (`SchemaEntry`, `SpellcheckEntry`) upgraded to `RwLock` for concurrent read access | 3h |
| Session-aware history | History entries tagged with session ID; `%hist --session=2` to filter | 2h |
| `future` integration | R `future` package compatibility — orchard sessions as `plan(multisession)` backends | 4h |

**Subtotal:** ~37h

**Design constraint:** The session model should feel transparent to the user. A
single REPL frontend manages N backend R processes. `%bg` and `%jobs` provide
explicit control. The default mode remains single-session, matching current
behavior. Multithreading is opt-in per operation, not a global mode switch.

---

#### Extensibility (Plugin System)

Orchard's 77 magic handlers and 14 completion backends are all hard-coded. v2.0
adds a plugin architecture so users can ship their own handlers, completers, and
display renderers as standalone Rust crates or R packages.

| Feature | Description | Effort |
|---------|-------------|-------|
| Plugin trait system | `Plugin` trait with `init()`, `magics()`, `completers()`, `renderers()` hooks | 4h |
| Dynamic loading | `dlopen`/`libloading` plugin discovery from `~/.orchard/plugins/` directory | 5h |
| R package plugins | R packages with an `orchard.yml` manifest that register magics and completers from R code | 6h |
| Plugin manifest | TOML or YAML manifest: `name`, `version`, `magics`, `completers`, `renderers`, `requires` | 2h |
| `%plugin` magic | List, load, reload, unload plugins: `%plugin list`, `%plugin load myplugin`, `%plugin reload myplugin` | 2h |
| Plugin isolation | Plugins run in their own R environment (`orchard.plugins:<name>`) to avoid namespace conflicts | 3h |
| Completion backend registration | `register_completer(name, priority, callback)` API for third-party completers | 2h |
| Plugin CLI | `orchard plugin init myplugin` scaffolding, `orchard plugin build`, `orchard plugin test` | 3h |
| Plugin registry | Community plugin index (Git repo with curated list + install instructions) | 2h |
| Sandboxing | Optional `--plugin-sandbox` flag that restricts plugin filesystem and network access | 3h |

**Subtotal:** ~32h

**Design constraint:** Plugins should be discoverable and versioned. The manifest
format is stable and semver-aware. Plugin loading failures must never crash the
REPL — they degrade gracefully with an error message. Rust plugins are loaded
via `libloading`; R plugins are loaded via `source()` into an isolated environment.

---

### Release Criteria (v2.0)

| Criterion | Requirement |
|-----------|-------------|
| Rich output | SVG and HTML rendering with `repr_*` protocol in R |
| Multithreading | Background R evaluation via `%bg`/`%jobs` with session pool |
| Plugin system | Loadable plugins from `~/.orchard/plugins/` with magic + completion registration |
| Magic handlers | 95+ (includes plugin-contributed) |
| Tests | 500+ passing, 0 failed |
| CI | Linux + macOS + Windows automated |
| Platform | All three platforms fully supported and tested |
| Documentation | Plugin developer guide, API reference, rich output tutorial |
| Performance | REPL latency < 10ms on idle; background evaluation does not block input |

---

### v2.0 Design Decisions

1. **Session pool over threads.** R is fundamentally single-threaded (no
   `R_GlobalEnv` sharing across threads). The session model spawns child R
   processes connected via pipe IPC. This is the same pattern RStudio and
   Jupyter use — it's battle-tested and avoids the GIL-like constraints of
   R's C API.

2. **Terminal-first rendering.** SVG and HTML are rendered to terminal-safe
   representations by default (braille-art for plots, colored tables for HTML).
   An optional `--gui` flag launches a webview for full-fidelity rendering.
   This keeps orchard's core promise: a REPL that loves living in a terminal.

3. **Rust plugins, not Lua.** Extensions are Rust crates loaded via `libloading`,
   or R packages loaded via the existing `eval_string_raw_global` path. No
   scripting language runtime in between. This is opinionated but consistent
   with orchard's "no dependency tangles" philosophy.

4. **Backward compatibility.** v2.0 is additive. All v1.0 magics, completions,
   and configuration options continue to work unchanged. New features are opt-in.

---


## Feature Count Trajectory

```
v0.2: 47 handlers (baseline)
v0.3: 59 handlers (+8 EDA, +1 xmode, +1 save, +1 automagic, +1 inspect — plus $/@/pipe completion, CI pipeline)
       ➜ Complete
v0.4: 66 handlers (+7 handlers + cwd-contextual history search)
       ➜ Complete
v0.5: 77 handlers (+11: 8 debug handlers, %methods, %psearch, ? modal help)
       ➜ Complete
v0.6: 79 handlers (+2: %inspect TUI popup, %dev, %plots)
       ➜ Complete
v0.7: 82 handlers (+3: %import, %connections, %repro; +4 infrastructure features)
       ➜ Complete
v0.8: 86 handlers (+4: snippets, %z, %copy, notify)
v0.9: 86 handlers (infrastructure: packaging, docs, macOS, CI)
v2.0: 100+ handlers (+14: plugin-contributed, +%bg/%jobs/%cancel, +%display/%render, +%plugin)
```

---

## Missing Features: Terminal + Editor as IDE

### Tier 1 — Required for IDE Viability

These are the features that make a terminal REPL + text editor combination
competitive with RStudio or VSCode-R.

1. **Editor send-code protocol** (v0.7)
   - Unix domain socket, named pipe, or `orchard --send "expr"` CLI
   - Enables neovim (iron.nvim, vim-slime), emacs (ESS), helix, and tmux
     to inject code into the running session
   - This is the single most important missing feature for the IDE goal

2. **`%edit` w/ srcref line jump / go-to-definition** (v0.7)
   - `%edit function_name` opens $EDITOR at defining source file:line
   - Uses R's `srcref` attributes (stored on every parsed function)
   - Julia-equivalent: `@edit f(x)`

3. **Inline plot display** (v0.6)
   - Sixel, kitty graphics protocol, or iTerm2 inline images
   - Fallback: ASCII art or external device
   - Keeps analyst in-flow without switching to an external plot window

### Tier 2 — Strongly Recommended

4. **`]` package mode** (v0.7) — renv + pak modal interface for reproducible environments
5. **Fzf-style fuzzy matching** (v0.5) — completion, history, variable selector, file paths
6. **LSP / lintr / styler integration** (post-v1.0) — diagnostics and formatting via R FFI
7. **Persistent workspace + plot panes via tmux** (post-v1.0) — `%tmux` magic for auto-layout

### Tier 3 — Quality of Life

8. **Cwd-contextual history** (v0.4) — atuin-style: tag entries with directory, prioritize current project
9. **Snippet expansion** (v0.8) — zsh-abbrev style: `gg` → `ggplot(`, `dp` → `dplyr::`
10. **Auto-time display** (v0.8) — print elapsed time for expressions exceeding N seconds
11. **`%import` smart loader** (v0.7) — sniff extension, dispatch to readr/readxl/arrow/data.table
12. **`%connections` DBI browser** (v0.7) — list, show schemas, test queries
13. **`%copy` clipboard** (v0.8) — copy expression result to system clipboard
14. **Command-not-found suggestions** (v0.8) — "Did you mean `install.packages()`?"
15. **`%repro` / `%sessioninfo`** (v0.3/v0.7) — reproducibility metadata

---

## Upstream Python Radian: Out of Scope

These upstream Python radian features are intentionally deferred or excluded:

| Feature | Reason |
|---------|--------|
| Reticulate prompt mode | Requires Python interpreter in-process — not compatible with pure-Rust binary |
| `register_cleanup` on-load hooks | Post-v1.0; no user demand yet |
| Askpass setup | Rare use case in terminal REPL |
| `utils::rc.settings(ipck=TRUE)` | Completion behavior tuning — investigate post-v0.5 |

---

## Known Risks

1. **Manual SIGINT test ignored** (`#[ignore]`) — environment-sensitive, not automated. Needs manual acceptance on Linux + macOS.
2. **macOS not acceptance-tested** — `dyld.rs` code paths may have issues on real hardware.
3. **R_ParseVector malformed expression workaround** — pointer guard (`0x1000` check) handles R 4.6.x edge case but may mask real errors.
4. **Handler-level integration tests limited** — handlers that call R FFI (`%objects`, `%time`, etc.) cannot be fully tested without R initialized; only parse-level and error-path tests exist. The `R_AVAILABLE` guard in `eval_string_raw_global` ensures these tests return a clean error rather than SIGSEGV when R is absent.
5. **No CI pipeline** — no automated build/test on any platform. Fresh `cargo check`/`cargo test` passes manually; automated enforcement for PRs is missing.

---

## Remaining Gaps

### Core REPL

| Gap | Effort | Priority |
|-----|--------|----------|
| macOS acceptance | 2h | High |
| Manual SIGINT acceptance | 1h | Medium |
| CI pipeline (Linux) | 4h (planned v0.3) | Medium |
| CI pipeline (macOS) | 4h (planned v0.9) | Low |

### Code Quality

| Gap | Effort |
|-----|--------|
| Handler-level R integration tests | 4h |
| Integration benchmarks (prompt latency) | 2h |
| User documentation | 4h (planned v0.9) |
| Release packaging | 4h (planned v0.9) |

---

## Verification

```bash
cargo check                             # 0 errors, 0 warnings
cargo test --lib --no-fail-fast         # 381 passed, 1 ignored
cargo test --test magic_framework       # 7 passed
cargo clippy                            # 0 warnings
```

For completion tests:
```bash
cargo test --lib completion             # 50+ tests (schema, fuzzy, magic arg, function arg, spellcheck)
cargo test --lib levenshtein            # Levenshtein distance tests
cargo test --lib test_shell_sx_echo -- --ignored  # R-dependent test
cargo test --lib magics::debug          # 27 tests (debug magics — no R needed after R_AVAILABLE guard)
```

For data inspector integration tests (requires R):
```bash
ORCHARD_TEST_R=1 cargo test --test embedded_r -- --test-threads=1 --nocapture
```
