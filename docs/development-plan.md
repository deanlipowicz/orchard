# Development Plan

**What:** Rust rewrite of `radian`, the R terminal REPL, with IPython-style magic
commands, an intelligent in-terminal data inspector, and schema-aware autocomplete.
Replaces upstream Python radian on Linux (macOS pending acceptance).

**Current state:** 49 registered magic handlers | 272 tests (265 lib + 7 magic framework) | Linux only

---

## Architecture

```
reedline/readline → r_runtime::read_console_interactive
  ├── ; shell mode (persistent or one-shot)
  ├── ! inline shell execution
  ├── ?/?? object introspection
  ├── % magic dispatch (49 handlers)
  ├── + tab: Schema-aware autocomplete + variable selector
  └── R evaluation (via R C API)

r_runtime → magic_registry → MagicHandler::run() → Output
  ├── Output::Text → display in REPL
  ├── Output::Eval → evaluate in R
  └── Output::DisplayAndEval → display + evaluate

Data Inspector (post-v0.5):
  R → R commands → column metadata + sample rows
  → Rust table formatter → comfy-table/ratatui rendering → TUI output
```

**Key files:**
- `src/r_runtime.rs` — REPL loop, dispatch, R callbacks
- `src/magic.rs` — registry, MagicHandler trait, MagicLine
- `src/magics/*.rs` — handler modules (49 handlers)
- `src/history.rs` — history + snapshot
- `src/prompt.rs` — reedline session, completer, highlighter
- `src/shell.rs` — shell commands, env lock
- `src/completion.rs` — R/package/LaTeX/shell completion

**Key decisions:**
- Magic dispatch runs in `read_console_interactive` (Rust side, before returning to R)
- `Arc<dyn MagicHandler>` clone pattern prevents reentrant mutex deadlock
- `eval_string_raw_global` is the safe public API for R evaluation from handlers
- `OnceLock<Mutex<...>>` globals for shared state (CONSOLE, SHELL_STATE, ALIAS_MAP)
- `#![deny(unsafe_op_in_unsafe_fn)]` enforced — all unsafe blocks auditable
- All `unwrap()` calls in production code have safety-rationale comments

---

## Release Gates

| Gate | Claim | Status | Blockers |
|------|-------|--------|----------|
| v0.1 | Experimental Linux REPL | ✅ PASS | None |
| v0.2 | Core radian parity on Linux | ✅ PASS | None |
| v0.3 | Quick wins + CI | 🔲 Planned | 7 handlers + CI not implemented |
| v0.4 | Debugger + data completeness | 🔲 Planned | See roadmap |
| v0.5 | TUI data inspector + schema autocomplete | 🔲 Planned | See spec |
| v0.6 | Session persistence + history replay | 🔲 Planned | See roadmap |
| v0.7 | Platform + packaging | 🔲 Planned | macOS hardware |
| v0.8 | Logging + extensions | 🔲 Planned | API design needed |
| v0.9 | Advanced features | 🔲 Planned | See roadmap |
| v1.0 | Production replacement | ❌ BLOCKED | All v0.3–v0.9 gates |

---

## Current Feature Set (49 Handlers)

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

---

## Feature: Configurable Editing Mode (Vim / Emacs)

Orchard supports both vim and emacs-style editing modes, configurable at
runtime via R options with no restart required. This is critical for users
who touch-type and expect consistent editing shortcuts across their tools.

### Configuration

```r
# Switch between "emacs" (default) and "vi":
options(orchard.editing_mode = "vi")

# Show vi mode indicator in prompt: (I) insert, (N) normal:
options(orchard.show_vi_mode_prompt = TRUE)

# Allow emacs keybindings (Ctrl-A, Ctrl-E, etc.) in vi insert mode:
options(orchard.emacs_bindings_in_vi_insert_mode = TRUE)

# Custom Ctrl-key shortcuts (inserts text or triggers commands):
options(orchard.ctrl_key_map = list(
  list(key = "k", value = "function()"),
  list(key = "l", value = "lm(")
))

# Custom Alt-key shortcuts (escape key map):
options(orchard.escape_key_map = list(
  list(key = "p", value = "|> "),
  list(key = "d", value = "%>% ")
))
```

### Emacs Mode Default Shortcuts

Provided by reedline's built-in `Emacs` edit mode:

| Shortcut | Action | Emacs Name |
|----------|--------|------------|
| `Ctrl-A` | Move cursor to line start | beginning-of-line |
| `Ctrl-E` | Move cursor to line end | end-of-line |
| `Ctrl-B` | Move cursor back one char | backward-char |
| `Ctrl-F` | Move cursor forward one char | forward-char |
| `Alt-B` | Move cursor back one word | backward-word |
| `Alt-F` | Move cursor forward one word | forward-word |
| `Ctrl-D` | Delete character at cursor | delete-char |
| `Ctrl-H` | Delete character before cursor | backward-delete-char |
| `Ctrl-K` | Kill to end of line | kill-line |
| `Ctrl-U` | Kill to beginning of line | backward-kill-line |
| `Ctrl-W` | Delete word backward | backward-kill-word |
| `Alt-D` | Delete word forward | kill-word |
| `Ctrl-P` | Previous history entry | previous-history |
| `Ctrl-N` | Next history entry | next-history |
| `Ctrl-R` | Reverse search history | reverse-search |
| `Ctrl-T` | Transpose characters | transpose-chars |
| `Ctrl-Y` | Yank (paste killed text) | yank |
| `Ctrl-L` | Clear terminal | clear-screen |
| `Ctrl-C` | Cancel / send interrupt | keyboard-quit |
| `Ctrl-D` | Exit (on empty line) | EOF |
| `Tab` | Trigger autocomplete | complete |
| `Ctrl-Space` | Set mark (for region select) | set-mark |
| `Ctrl-W` in region | Kill selection | kill-region |

### Vi Mode Default Shortcuts

Provided by reedline's built-in `Vi` edit mode. Vi has two sub-modes:

**Normal mode (press `Esc` to enter):**

| Shortcut | Action |
|----------|--------|
| `h` / `Left` | Move cursor left |
| `l` / `Right` | Move cursor right |
| `w` | Move forward one word |
| `b` | Move backward one word |
| `0` | Move to line start |
| `$` | Move to line end |
| `dd` | Delete current line |
| `dw` | Delete word forward |
| `db` | Delete word backward |
| `x` | Delete character at cursor |
| `u` | Undo |
| `Ctrl-R` | Redo |
| `yy` / `Y` | Yank (copy) line |
| `p` | Paste after cursor |
| `P` | Paste before cursor |
| `/` | Search forward in history |
| `?` | Search backward in history |
| `i` | Enter insert mode at cursor |
| `I` | Enter insert mode at line start |
| `a` | Enter insert mode after cursor |
| `A` | Enter insert mode at line end |
| `o` | Open line below |
| `O` | Open line above |
| `v` | Begin characterwise visual mode |
| `V` | Begin linewise visual mode |

**Insert mode (`emacs_bindings_in_vi_insert_mode` adds emacs Ctrl-key
shortcuts while in insert mode):**

| Shortcut | Action |
|----------|--------|
| `Ctrl-A` | Move to line start |
| `Ctrl-E` | Move to line end |
| `Ctrl-K` | Kill to end of line |
| `Ctrl-U` | Kill to beginning of line |
| `Ctrl-W` | Delete word backward |
| `Ctrl-P` | Previous history |
| `Ctrl-N` | Next history |
| `Esc` | Return to normal mode |

### Quick Editing Use Cases

These are the "quick editing" tasks R users perform most frequently,
with the fastest path in each editing mode:

| Task | Emacs Path | Vi Path |
|------|-----------|---------|
| Start of line | `Ctrl-A` | `Esc` `0` `i` |
| End of line | `Ctrl-E` | `Esc` `$` `a` |
| Delete word backward | `Ctrl-W` | `Esc` `db` `i` |
| Delete word forward | `Alt-D` | `Esc` `dw` `i` |
| Kill to end | `Ctrl-K` | `Esc` `D` `i` |
| Kill to start | `Ctrl-U` | Not bound `Esc` `d0` `i` |
| Previous command | `Ctrl-P` | `Esc` `k` `i` |
| Repeat last edit | Not bound | `.` |
| Transpose chars | `Ctrl-T` | Not bound |

### Custom Keybinding Maps

Users can define custom keybindings via R options for any Ctrl or Alt
key combination:

```r
# Ctrl+K inserts a pipe operator instead of killing to end of line:
options(orchard.ctrl_key_map = list(
  list(key = "k", value = " |> ")
))

# Alt+P inserts the native R pipe:
options(orchard.escape_key_map = list(
  list(key = "p", value = " |> ")
))
```

### Implementation

- Editing mode selection in `src/prompt.rs` — `edit_mode()` function
  returns `Box<dyn EditMode>` (reedline `Vi` or `Emacs`).
- Custom bindings applied via `apply_custom_bindings()` in `src/prompt.rs`
  — iterates `ctrl_key_map` and `escape_key_map`, adds reedline bindings.
- Vi mode prompt indicator in `src/r_runtime.rs` — detects `editing_mode == "vi"`,
  prepends `[I]` or `[N]` to the prompt string.
- Settings loaded from R options in `src/settings.rs` — `orchard.editing_mode`,
  `orchard.show_vi_mode_prompt`, etc.
- Reserved key guard: Ctrl-M, Ctrl-I, Ctrl-H, Ctrl-D, Ctrl-C cannot be
  remapped (they conflict with terminal control characters).

### Magic Commands (49 Registered)

| Phase | Handlers | Status |
|-------|----------|--------|
| P0 — Framework | `%lsmagic`, `%magic` | ✅ 2 handlers |
| P1 — Shell | `%pwd`, `%env`, `%bookmark`, `%cd`, `%ls`, `%sx`, `%pushd`, `%popd`, `%dhist` | ✅ 9 handlers |
| P2 — Object browser | `%objects`, `%who`, `%whos`, `%who_ls`, `%rm`, `%clear`, `%str`, `%head`, `%skim`, `%dim`, `%names`, `%plot`, `%tidy`, `%View`, `%pdoc`, `%pdef`, `%psource`, `%pfile` | ✅ 18 handlers |
| P3 — Timing | `%time`, `%timeit`, `%prun` | ✅ 3 handlers |
| P4 — History | `%hist`, `%hist_n` | ✅ 2 handlers |
| P5 — Debugger | `%debug`, `%pdb`, `%traceback`, `%where`, `%c` | ✅ 5 handlers |
| P6 — Workspace | `%pinfo`, `%pinfo2` | ✅ 2 handlers |
| P7 — Config | `%config`, `%colors`, `%alias`, `%unalias` | ✅ 4 handlers |
| P8 — File | `%run`, `%load` | ✅ 2 handlers |
| P9 — Edit | `%macro`, `%edit` | ✅ 2 handlers |
| **Total** | **49 handlers** | |

**Dispatch order:** `;` → `!` → `?` → `%` → R

---

## Feature: Intelligent In-Terminal Data Inspector

A new magic command (`%inspect` or enhanced `%str`/`%head`) that renders any
R data object as a formatted TUI table with:

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

The inspector must work with all of these R object types:

| Engine | R Object Class | Detection | Extraction Path |
|--------|---------------|-----------|-----------------|
| **DuckDB** | `duckdb_relation`, `tbl_duckdb_connection` | `class(obj)` includes `tbl_duckdb_connection` or `duckdb_relation` | `DBI::dbGetQuery()` or `as.data.frame()` with limit |
| **Arrow** | `ArrowObject`, `Table`, `RecordBatch` | `class(obj)` includes `ArrowObject` or `Table` | `as.data.frame()` with n_max |
| **tidyverse** | `tbl_df`, `grouped_df`, `spec_tbl_df` | `inherits(obj, "tbl_df")` | Standard `dplyr::glimpse()` + `head()` |
| **Vanilla R** | `data.frame`, `matrix`, `vector`, `factor`, `list` | `is.data.frame()`, `is.matrix()`, `is.vector()` | Base R `head()`, `summary()` |
| **Stan** | `stanfit` | `inherits(obj, "stanfit")` | `as.data.frame(extract())` with min n |
| **Rcpp** | `Rcpp::DataFrame`, `Rcpp::NumericMatrix` | Inherits from data.frame/matrix at R level | Standard `as.data.frame()` |
| **JS/mp** | `js` objects from V8/JS packages | `class(obj)` includes JS-specific classes | `as.list()` conversion |

### Implementation Strategy

```
Rust handler receives object name
  → Constructs R code to extract metadata
  → eval_string_raw_global() returns JSON/CSV string
  → Rust parses into Vec<ColumnMetadata>
  → comfy-table or ratatui renders as TUI table
  → Output::Text displayed in REPL (or TUI popup)
```

**Phase 1 (v0.5):** Text-based table via `comfy-table` crate. Works for
data.frames, tibbles, matrices, vectors. Returns formatted text output.

**Phase 2 (v0.7):** TUI popup via `ratatui` crate. Interactive scrolling,
sort by column, expand cell preview.

### R Metadata Extraction Code

The R-side metadata extraction follows this pattern:

```r
function(inspect_object(name)) {
  obj <- get(name, envir = .GlobalEnv)
  cls <- class(obj)
  if (inherits(obj, "data.frame") || inherits(obj, "matrix")) {
    cols <- if (is.matrix(obj)) seq_len(ncol(obj)) else names(obj)
    lapply(cols, function(col) {
      data <- obj[[col]]
      list(
        name = col,
        type = paste(class(data), collapse = "/"),
        n_missing = sum(is.na(data)),
        n_blank = if (is.character(data)) sum(data == "", na.rm = TRUE) else NA,
        mean = if (is.numeric(data)) mean(data, na.rm = TRUE) else NA,
        min = if (is.numeric(data)) min(data, na.rm = TRUE) else NA,
        max = if (is.numeric(data)) max(data, na.rm = TRUE) else NA,
        first_vals = head(data[!is.na(data)], 5)
      )
    })
  } else if (inherits(obj, "stanfit")) {
    # Extract posterior samples as data.frame
    as.data.frame(rstan::extract(obj))
  } else {
    # Fallback for vectors, factors, lists
    list(name = name, type = cls, length = length(obj),
         n_missing = sum(is.na(obj)),
         first_vals = head(obj, 5))
  }
}
```

---

## Feature: Schema-Aware Autocomplete + Variable Selector

Extends the existing completion system to provide:

### Current Completer (existing)

| Context | Source | Status |
|---------|--------|--------|
| R code | `utils:::.completeToken()` | ✅ Working |
| Packages | `.packages(all.available = TRUE)` | ✅ Working |
| LaTeX | 1983-entry static table | ✅ Working |
| File paths | `std::fs::read_dir()` | ✅ Working |

### Schema-Aware Extensions (planned)

| Context | Detection | Completion Source | Priority |
|---------|-----------|-------------------|----------|
| `dataframe$` | Regex: `\w+\$` | R `names(dataframe)` | High |
| `dataframe@` | Regex: `\w+@` (S4 slots) | R `slotNames(dataframe)` | Medium |
| `dataframe[[]]` | Regex: `\w+\[\[` | R `names(dataframe)` | Medium |
| `dplyr::` chain | Regex: `%>%\s*\w+$` | R pipe context detection | Medium |
| `data.table` | Regex: `\w+\[,` | R `names(data.table)` | Low |
| `library()` | Within `library(` context | R `.packages(all.available = TRUE)` | ✅ Done |
| `DBI::dbGetQuery()` | SQL context detection | DBI connection schema | Future |
| Formula `~` | Within `lm(`, `aov(`, etc. | R `names(data)` from formula context | Low |

### Variable Selector

A new completion mode activated by `Ctrl-Space` or `Alt-.` that:
1. Lists all variables in the global workspace
2. Shows variable type/size next to each name
3. Filters as you type
4. Inserts the selected variable name on Enter

```rust
// Pseudo-completion for variable selector
fn variable_selector(prefix: &str) -> Vec<CompletionItem> {
    // Call R to list global env variables with metadata
    let r_code = r#"
        vars <- ls(envir = .GlobalEnv)
        data.frame(
            name = vars,
            class = sapply(vars, function(v) paste(class(get(v)), collapse = "/")),
            size = sapply(vars, function(v) utils::object.size(get(v)))
        )
    "#;
    // Parse R result into Vec<CompletionItem>
    // Filter by prefix
    // Sort by recency/frequency
}
```

---

## Staged Roadmap

### v0.3 — Quick Wins + CI (Current)

**Target:** 56 handlers (49 current + 7 new)
**Status:** 🔲 To be implemented

| Handler | Description | Effort |
|---------|-------------|--------|
| `%xmode` | Traceback verbosity control | 0.5h |
| `%save` | Save history to file | 1h |
| `%automagic` | Toggle `%` prefix | 1h |
| `%help_pkg` | Package help | 0.5h |
| `%help_page` | Help page render | 0.5h |
| `%summary` | Statistical summary | 0.5h |
| `%glimpse` | Data glimpse | 0.5h |
| CI pipeline (Linux) | GitHub Actions | 1h |

### v0.4 — Debugger Completeness

**Target:** 62 handlers (56 + 6)

| Handler | Description | Effort |
|---------|-------------|--------|
| `%debugonce` | Set function to debug once | 0.5h |
| `%undebug` | Remove debugging | 0.5h |
| `%browser` | Invoke browser() | 0.5h |
| `%n` | Debugger next | 0.5h |
| `%finish` | Debugger finish | 0.5h |
| `%Q` | Debugger quit | 0.5h |

### v0.5 — Schema-Aware Autocomplete + Variable Selector

**Target:** 65 handlers (62 + 3 non-handler features)

| Feature | Description | Effort |
|---------|-------------|--------|
| `$` / `@` column completion | R `names(obj)` after `obj$` prefix | 2h |
| `%%.` pipe completion | Schema after dplyr `%>%` | 3h |
| Variable selector (`Ctrl-Space`) | Global env variable list with types/sizes | 3h |

**Architecture change:** Add a new completion backend in `src/completion.rs`
that calls R to resolve the schema of the object before the `$`/`@`/`[[`,
caches the result, and returns column names as completion items.

### v0.6 — Intelligent Data Inspector (TUI Table Renderer)

**Target:** 66 handlers (65 + 1)

| Feature | Description | Effort |
|---------|-------------|--------|
| `%inspect` | TUI table renderer for any R object | 6h |

**Phase 1 — Text table (comfy-table):**
- [ ] Add `comfy-table` dependency
- [ ] Implement R metadata extraction (column names, types, stats, sample values)
- [ ] Handle cross-engine detection: DuckDB, Arrow, tidyverse, vanilla R, Stan, Rcpp, JS
- [ ] Rust-side table layout engine
- [ ] `%inspect <name>` handler in `src/magics/inspect.rs`

**Phase 2 — TUI popup (ratatui):**
- [ ] Add `ratatui` dependency
- [ ] Interactive scroll, sort by column
- [ ] Cell value preview for long content
- [ ] Responsive column width auto-sizing

### v0.7 — History Replay + Session Persistence

**Target:** 70 handlers (66 + 4)

| Handler | Description | Effort |
|---------|-------------|--------|
| `%rerun` | Re-execute history entries | 2h |
| `%recall` | Recall history into editor | 2h |
| `%reset` | Clean workspace | 0.5h |
| `%reset_selective` | Selective cleanup | 0.5h |
| `%xdel` | Delete variables | 0.5h |

### v0.8 — Platform + Packaging

**Target:** 70 handlers (no new — infrastructure)

| Feature | Description | Effort |
|---------|-------------|--------|
| Release packaging | `cargo deb`, binary distribution | 4h |
| User documentation | README, feature guide, migration guide | 4h |
| macOS acceptance | Manual testing on physical Mac | 2h |
| CI pipeline (macOS) | macOS GitHub Actions | 2h |

### v0.9 — Logging + Extensions + Persistence

**Target:** 78 handlers (70 + 8)

| Handler | Description | Effort |
|---------|-------------|--------|
| `%store` | Session persistence (RDS) | 3h |
| `%logstart` | Start session logging | 1h |
| `%logstop` | Stop session logging | 0.5h |
| `%logstate` | Show logging state | 0.5h |
| `%load_ext` | Load extension module | 2h |
| `%reload_ext` | Reload extension | 1h |
| `%unload_ext` | Unload extension | 1h |

### v1.0 — Release Candidate

**Target:** 78+ handlers, 200+ tests, all IPython/radian parity resolved.

| Criterion | Requirement |
|-----------|-------------|
| Magic handlers | 55+ IPython parity + 18 R.nvim + framework = 73+ |
| Data inspector | Cross-engine (DuckDB, Arrow, tidyverse, vanilla, Stan, Rcpp, JS) |
| Schema autocomplete | `$`, `@`, `[[`, `%>%` pipe completion |
| Tests | 200+ passing, 0 failed |
| CI | Linux CI automated, macOS documented |
| Documentation | Feature guide, migration guide, API docs |
| Release | Binary packages for Linux |
| Platform | Linux tested, macOS beta-supported |

---

## Cross-Engine Data Inspector: Required Object Types

The `%inspect` handler must detect and correctly render these object types:

| Engine | R Class | Detection | Extraction Strategy | Priority |
|--------|---------|-----------|-------------------|----------|
| **Vanilla R** | `data.frame` | `is.data.frame()` | `head()`, `summary()` | P0 |
| **Vanilla R** | `matrix` | `is.matrix()` | `head()`, `row/colnames` | P0 |
| **Vanilla R** | `vector` | `is.vector()` | `head()`, `table()` | P0 |
| **Vanilla R** | `factor` | `is.factor()` | `levels()`, `table()` | P0 |
| **Vanilla R** | `list` | `is.list()` | `names()`, `lapply(head)` | P0 |
| **tidyverse** | `tbl_df` / `grouped_df` | `inherits("tbl_df")` | `dplyr::glimpse()`, `head()` | P0 |
| **tidyverse** | `spec_tbl_df` | `inherits("spec_tbl_df")` | `spec()`, `head()` | P1 |
| **DuckDB** | `duckdb_relation` | `class()` | `DBI::dbGetQuery(conn, "SELECT * FROM rel LIMIT 10")` | P1 |
| **DuckDB** | `tbl_duckdb_connection` | `class()` | `dplyr::collect(head())` | P1 |
| **Arrow** | `Table` / `RecordBatch` | `inherits("ArrowObject")` | `as.data.frame(arrow::as_arrow_table(obj))` | P1 |
| **Stan** | `stanfit` | `inherits("stanfit")` | `rstan::extract()`, `summary()` | P2 |
| **Rcpp** | `Rcpp::DataFrame` | Inherits data.frame | Standard R extraction | P1 |
| **JS/V8** | JS objects | `class()` contains JS | `V8::as.list()` | P3 |

---

## Feature Count Trajectory

```
v0.1: 38 handlers (pre-uplift baseline)
v0.2: 49 handlers (shell + file + timing — current)
v0.3: 56 handlers (+7: xmode, save, automagic, help_pkg, help_page, summary, glimpse)
v0.4: 62 handlers (+6: debugonce, undebug, browser, n, finish, Q)
v0.5: 65 handlers (+3: schema autocomplete, variable selector)
v0.6: 66 handlers (+1: %inspect TUI data inspector)
v0.7: 70 handlers (+4: rerun, recall, reset, reset_selective, xdel)
v0.8: 70 handlers (infrastructure: packaging, docs, macOS, CI)
v0.9: 78 handlers (+8: store, logstart, logstop, logstate, load_ext, reload_ext, unload_ext)
v1.0: 78+ handlers (all resolved, documented, tested, released)
```

---

## Verification

```bash
cargo check                                    # 0 errors
cargo test --lib --no-fail-fast                # 265 passed
cargo test --test magic_framework --no-fail-fast # 7 passed
```

For schema autocomplete unit tests:
```bash
cargo test --lib completion                   # completion module tests
cargo test --lib test_shell_sx_echo -- --ignored # R-dependent test
```

For data inspector integration tests (requires R):
```bash
ORCHARD_TEST_R=1 cargo test --test embedded_r -- --test-threads=1 --nocapture
```
