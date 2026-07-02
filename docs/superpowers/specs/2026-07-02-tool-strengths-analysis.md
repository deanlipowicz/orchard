# Tool Strengths Analysis for orchard Feature Planning

**Date:** 2026-07-02
**Context:** Sequential analysis of five terminal-based tools — IPython, Radian,
Fish, Julia REPL, and Harlequin SQL — to identify strengths relevant to
statistical programming. Each analysis extracts design patterns, interaction
models, and workflow features that orchard should learn from or adopt.

---

## 1. IPython — The Gold Standard for REPL-based Statistical Computing

IPython (Interactive Python) is the most mature and influential enhanced REPL
in the scientific computing ecosystem. Originally created for Python, it has
shaped the design of nearly every subsequent REPL tool, including radian,
orchard, and Julia's REPL.

### 1.1 Core Design Strengths

**1.1.1 The Magic Command System**

IPython's defining innovation is the `%` and `%%` magic command system — a
set of special commands that extend the REPL beyond the host language's
capabilities without modifying the language itself. This is the most
important design pattern for orchard because it provides the blueprint for
R's enhancement.

Key strengths of the magic system:
- **Low cognitive overhead:** `%` prefix is visually distinctive and
  unambiguous. Users instantly know whether they're typing R code or a
  shell command.
- **Cell vs line distinction:** `%%` cell magics operate on multi-line
  blocks (e.g., `%%timeit` times an entire cell). Orchard currently
  has the `is_cell` field in `MagicLine` but no cell-level dispatch.
- **Consistent help:** `%magic` lists all commands with descriptions.
  `%magic <name>` shows detailed help for one command.
- **Discoverability:** `%lsmagic` lists all registered handlers. The
  magic registry pattern makes the system self-documenting.

**Relevance to statistical programming:** R users run many short
exploratory commands interspersed with shell operations (file management,
data import). The magic system keeps them in the R environment without
switching to a terminal.

**1.1.2 Rich Display System**

IPython's display system allows objects to return multiple representations
(plain text, HTML, SVG, LaTeX, JSON) using `_repr_*_` methods. The REPL
automatically selects the richest format the terminal supports.

Key innovations:
- **Object-to-display protocol:** Any Python object can define how it
  looks in the REPL. This is fundamentally different from R's `print()`
  dispatch — the REPL asks the object what representation to use.
- **Multimedia output:** Plots display inline (when using `%matplotlib
  inline`), data frames render as HTML tables, images render as rich
  output.
- **Fallback chain:** Rich → HTML → plain text. The terminal gets the
  best format it can handle.

**Relevance to statistical programming:** R already has `print()` methods
on S3/S4 objects, but they produce plain text. IPython's display system
shows what's possible when the REPL actively participates in rendering:
data frames as sortable HTML tables, plots inline, model summaries as
formatted output.

**1.1.3 Introspection and Discovery**

IPython's `?` and `??` operators are the fastest path to documentation in
any REPL:

- `object?` — signature, docstring, type, file location
- `object??` — full source code if available
- Tab completion calls `object.<TAB>` to show attributes/methods,
  `module.<TAB>` to show members

This creates a frictionless discovery loop: explore → find → understand →
use — all without leaving the REPL.

**Relevance to statistical programming:** R has `help()`, `example()`,
`str()`, and `ls()`, but they're slower to invoke and produce more noise.
IPython's `?name` pattern is faster and more focused. Orchard's `%who`,
`%whos`, `%str`, and `%pdoc` handlers mirror this but lack the single-key
`?` and `??` shortcuts.

**1.1.4 Shell Integration**

IPython treats the shell as a first-class citizen:
- `!command` — run a shell command and display output
- `output = !command` — capture output as a list of strings
- `%sx command` — shell capture returning structured output
- `%cd`, `%ls`, `%env`, `%bookmark`, `%pushd`/`%popd`, `%dhist` — shell
  navigation without leaving the REPL
- `%%script` — run a multi-line block in an external interpreter

**Relevance to statistical programming:** Data analysis involves constant
filesystem work (checking files, moving data, running scripts). Shell
integration keeps the user in the analytical flow instead of context-switching
to a terminal.

**1.1.5 Timing and Profiling**

IPython's timing commands are essential for performance-aware programming:
- `%time expr` — time a single expression (wall time, CPU time)
- `%timeit expr` — run multiple loops, report best/mean/worst
- `%prun expr` — profile with `cProfile`, show function-level timing
- `%lprun` — line-by-line profiling (extension)
- `%mprun` — memory profiling (extension)

**Relevance to statistical programming:** R users frequently benchmark
algorithms, compare package performance, and optimize data pipelines.
Orchard's `%time`, `%timeit`, and `%prun` cover the core three. The
line-by-line and memory profiling extensions are stretch goals.

**1.1.6 Debugger Integration**

IPython integrates tightly with Python's debugger:
- `%debug` — enter post-mortem debugger after an exception
- `%pdb` — toggle automatic debugger on exception
- `%tb` — show traceback (verbosity controlled by `%xmode`)
- `%xmode Plain|Context|Verbose` — control traceback detail
- `%run -d` — run a script under the debugger

**Relevance to statistical programming:** R code fails frequently during
exploratory analysis (missing columns, type mismatches, edge cases). Quick
debugger entry without restarting the session saves significant time.

### 1.2 IPython Features Relevant to Statistical Programming

| Feature | Statistical Programming Use Case | orchard Status |
|---------|--------------------------------|----------------|
| `%` magics | Rapid data exploration without leaving R | ✅ 49 handlers |
| `?` / `??` introspection | Instant function docs and source | ✅ `%pdoc`, `%psource` |
| Tab completion | Discover column names, functions, args | ✅ Via R's completer |
| `!` shell commands | File ops, git, data pipeline commands | ✅ Via `;` shell mode |
| `%time` / `%timeit` | Benchmark data processing steps | ✅ Implemented |
| `%prun` | Profile slow analysis code | ✅ Implemented |
| `%debug` / `%pdb` | Debug failing transformations | ✅ Implemented |
| `%xmode` | Control error verbosity | ❌ Missing — quick win |
| `%save` | Save history to R script | ❌ Missing — quick win |
| `%automagic` | Reduce typing for frequent commands | ❌ Missing — needs dispatch |
| `%rerun` / `%recall` | Replay analysis steps | ❌ Missing — needs injection |
| `%store` | Persist variables across sessions | ❌ Missing |
| `%%` cell magics | Multi-block timing/script execution | ❌ Missing |
| Rich display | Formatted data frame output | ❌ Post-v1 deferred |
| `%run` | Source R scripts from REPL | ✅ `%run` implemented |
| `%load` | Load file into editor | ✅ `%load` implemented |
| `%reset` | Clean workspace | ❌ Missing |
| `%logstart` / `%logstop` | Session logging | ❌ Missing |

### 1.3 What orchard Should Learn from IPython

1. **Magic system as the core interaction model** — Already adopted (49
   handlers). The pattern is correct; only the count needs to grow.

2. **Cell magics (`%%`)** — Enables multi-line timing, script execution,
   and shell blocks. The `MagicLine.is_cell` field exists but no dispatch
   uses it yet.

3. **Automagic** — The single highest-ROI feature not yet implemented.
   Saves one keystroke (`%`) on every magic command. Requires a dispatch
   modification in the input handler.

4. **`?` / `??` as first-class shortcuts** — Orchard's `%pdoc`/`%psource`
   provide the same information but require `%` prefix. A Rust-side
   detection of `?name` at line start (before sending to R) would match
   IPython's zero-friction introspection.

5. **Rich display protocol** — The longest-range opportunity. If orchard
   could intercept R's `print()` output and render data frames as tables,
   model summaries as formatted text, and plots inline (via sixel or
   kitty protocol), it would leapfrog the Python radian feature set.

6. **Graceful error handling** — `%xmode` controls traceback verbosity.
   Even a simple `%xmode quiet` / `%xmode verbose` toggle would improve
   the R debugging experience significantly.

---

## 2. Radian — The R-specific Enhanced REPL

Radian is the direct upstream of orchard — a Python-based enhanced REPL for R
that orchard rewrites in Rust. Unlike IPython (which reimagined Python's REPL
from scratch), Radian wraps R's existing REPL infrastructure through embedded
R callbacks. Understanding Radian's design decisions is critical because they
are R-specific solutions, not generic REPL features.

### 2.1 Core Design Strengths

**2.1.1 R Option-Backed Settings System**

Radian's most distinctive design decision is that **all configuration lives
in R options**, not in a separate config file. This means:

```r
# Instead of editing a config file:
options(radian.auto_suggest = TRUE)
options(radian.editing_mode = "vi")
options(radian.color_scheme = "monokai")
```

Strengths of this approach:
- **Single source of truth:** R options are the config mechanism. No
  YAML/TOML/JSON file to maintain, parse, or debug.
- **Per-session configurability:** Settings change during a session via
  `options()`, not just at startup. A profile can conditionally set
  options based on the environment.
- **Profile-friendly:** R's `.Rprofile` and radian's `.radian_profile`
  both use `options()` — no separate config language to learn.
- **Discoverable:** `options() %>>% grep("radian", .)` shows all radian
  settings. No config file location to remember.

**Relevance to orchard:** orchard already implements this pattern in
`src/settings.rs` and `src/env_setup.rs`. The settings are loaded from
R options at startup and parsed into typed Rust structs. This is the
correct approach — orchard should keep it and expand it.

**2.1.2 Modal Prompt System**

Radian detects four prompt modes by inspecting the prompt string R sends:

| Mode | Detection | History Book | Behavior |
|------|-----------|-------------|----------|
| `R` | Default R prompt (`> ` or custom) | `r` | Standard R evaluation |
| `Browse` | Matches `Browse[(\d+)]> $` | `r` (shared) | Debugger commands; browser command filter |
| `Shell` | User typed `;` at line start | `shell` | Subprocess execution via `$SHELL -c` |
| `Unknown` | Fallback (no known pattern) | none | Passthrough to R |

Strengths:
- **Prompt string as state machine:** R's prompt is the signal. Radian
  never guesses the mode — R tells it explicitly.
- **History partitioning:** Shell commands go to `shell` history, R
  commands go to `r` history, browser commands can be filtered. History
  search respects mode boundaries.
- **Shell mode persistence:** Once in shell mode, the prompt changes and
  every line is treated as a shell command until the user submits an empty
  line or backspaces at column 0.

**Relevance to orchard:** orchard implements this in `src/prompt.rs`
(PromptMode enum) and `src/r_runtime.rs` (mode detection in the read
console callback). The Shell mode entry/exit via `;` at line start and
backspace at column 0 is handled in `src/editing_hook.rs`. This is one
of the most mature parts of the codebase.

**2.1.3 Profile Loading Order**

Radian sources profiles in a specific order, each overriding the previous:

1. `--profile PATH` (command-line flag, if provided)
2. `$XDG_CONFIG_HOME/radian/profile` or `~/.config/radian/profile`
3. `~/.radian_profile` (global user profile)
4. `.radian_profile` (local project profile, if different from global)

Strengths:
- **Predictable override chain:** Command-line > XDG > global > local.
  Local project settings can override global defaults.
- **No special config syntax:** Profiles are R code sourced via `source()`.
  Users write `options()` calls in plain R.
- **Compatible with R's startup:** R's own `.Rprofile` runs before
  radian's profiles, so system-level R config happens first.

**Relevance to orchard:** orchard implements this in `src/env_setup.rs`.
The loading order matches upstream radian. This is correct and should
be preserved.

**2.1.4 LaTeX Completion**

Radian includes a 2493-entry LaTeX symbol table for R completion. Typing
`\alpha` completes to `α`, `\beta` → `β`, `\sum` → `∑`, and so on.

Strengths:
- **Domain-specific need:** R uses LaTeX-style names throughout (ggplot2
  theme elements, plotmath expressions, knitr documents). LaTeX completion
  is not a nicety — it's a daily workflow requirement for R users.
- **Zero-config:** The table is built in. No package installation needed.

**Relevance to orchard:** The full 1983-entry table exists in
`src/completion.rs`. This matches the upstream radian feature.

**2.1.5 Auto-Pair Rules for R Syntax**

Radian implements syntax-aware auto-pairing that understands R's specific
quoting rules:

- Standard pairs: `()`, `[]`, `{}`
- R's backtick quoting: `` `name with spaces` ``
- R's raw string literals: `r"(...)"`, `r'[...]'`, `r'{...}'`, with dash
  delimiter variants like `r"---(content)---"`
- Skip logic: typing `)` when cursor is before `)` skips over it instead
  of inserting a duplicate
- Smart backspace: backspacing inside `()` removes both characters;
  backspacing at leading indent removes `tab_size` spaces
- Shell-mode backspace: backspacing at column 0 in shell mode exits to R

**Relevance to orchard:** All of these are implemented in
`src/editing_hook.rs` (the vendored reedline pre-edit hook). This is one
of the most polished parts of the codebase.

**2.1.6 Embedded R Console Callbacks**

Radian registers C callbacks with R's embedding API:
- `ReadConsole` — intercepts R's prompt for input, feeds it through the
  Rust REPL instead of stdin
- `WriteConsoleEx` — captures R's stdout/stderr output, routes through
  the Rust terminal with formatting
- `ShowMessage` — captures R's message() calls
- `FlushConsole` / `ClearConsole` — console lifecycle

Strengths:
- **Full R integration:** Works with any R version ≥ 4.0, any package.
- **No subprocess:** R runs in-process. No IPC, no serialization, no
  protocol parsing. All R objects are accessible via FFI.
- **Correct stdout/stderr separation:** R's output goes through the
  Rust terminal with proper ANSI formatting.

**Relevance to orchard:** Implemented in `src/r_runtime.rs` (FFI
callbacks) and `src/settings.rs` (stderr formatting). This is the
foundation everything else builds on.

### 2.2 Radian Features Relevant to Statistical Programming

| Feature | Statistical Programming Use Case | orchard Status |
|---------|--------------------------------|----------------|
| R option-backed settings | Configure REPL behavior via R code | ✅ Implemented |
| 4-mode prompt system | R/Browse/Shell/Unknown with correct dispatch | ✅ Implemented |
| Profile loading order | Per-project and global R configuration | ✅ Implemented |
| LaTeX completion | ggplot2, plotmath, knitr workflows | ✅ 1983-entry table |
| Auto-pair for R syntax | Backtick quoting, raw strings, smart pairs | ✅ Editing hook |
| Embedded R callbacks | In-process R evaluation, stdout/stderr capture | ✅ r_runtime.rs |
| `;` shell mode | File ops, git, data manipulation without leaving R | ✅ Implemented |
| `%` magic system | 49 R-specific magic handlers | ✅ 49 handlers |
| `utils:::.completeToken()` | R-aware tab completion | ✅ Via R FFI |
| `--vanilla` / `--quiet` etc. | Standard R CLI flags | ✅ cli.rs |

### 2.3 What orchard Should Learn from Radian

1. **The settings system is correct — expand it.** More R-backed options
   for terminal behavior (autosuggest toggles, color scheme, keybinding
   maps) should be added as R options. No config file format is needed.

2. **Browser prompt mode needs attention.** The debugger commands (`n`,
   `s`, `c`, `Q`, `where`) are partially implemented as magic handlers
   (`%c`, `%where`) but the Browse prompt's special behavior (history
   filtering, command recognition) could be tighter.

3. **Profile loading is correct.** The order and mechanism match upstream.
   Test coverage for edge cases (XDG vs HOME, missing files, permissions)
   would be valuable.

4. **The auto-pair implementation is a competitive advantage.** Radian's
   R-specific editing rules (raw strings, backtick quotes, smart dedent)
   are unique among R REPLs. This should be preserved and documented.

---

## 3. Fish Shell — Design Philosophy for Interactive Terminals

Fish (Friendly Interactive Shell) is a Unix shell designed from the ground up
for interactive use, not script compatibility. Its design philosophy — discover
by default, configure by exception — is directly applicable to REPL design.

### 3.1 Core Design Strengths

**3.1.1 Autosuggestions**

Fish's most celebrated feature: as you type, fish suggests completions in
dimmed text based on history and completions. Press `→` or `Ctrl-F` to
accept the suggestion.

Strengths:
- **History-as-completion:** The suggestion comes from your own command
  history. The more you use a command, the faster it becomes to re-run.
- **Stateless:** Suggestions are computed on every keystroke from the
  current history and completions. No ML, no training, no configuration.
- **Friction reduction:** Common tasks (ssh to known hosts, cd to visited
  directories, git commands) become single-digit keystrokes after the
  first use.
- **Discoverability:** Suggestions show you what's possible — a command
  you forgot about, a flag you didn't know existed.

**Relevance to orchard:** reedline supports autosuggestions via the
`DefaultHinter` which is already wired in orchard's `PromptSession`
(`src/prompt.rs`). The suggestion draws from reedline's `History` trait
implementation (`OrchardHistoryBackend` in `src/history.rs`). This is
functional but could be improved: fish's autosuggestions account for
command frequency and recency, and they dim the suggestion text rather
than showing it as a tooltip.

**3.1.2 Syntax Highlighting**

Fish applies real-time syntax highlighting as you type:
- Commands in bright blue (valid commands only — invalid commands stay red)
- Quoted strings in green
- Paths underlined if they exist
- Errors in red (mismatched quotes, bad expansions)

Strengths:
- **Immediate error feedback:** You see a syntax error before running the
  command. For R users, this means seeing unmatched parentheses, unclosed
  string literals, and unknown function names in real time.
- **Learning tool:** Syntax highlighting teaches correct syntax by making
  errors visible. New R users learn quoting rules faster.
- **No configuration:** Fish detects valid commands by checking PATH and
  builtins. No manual syntax definitions needed.

**Relevance to orchard:** reedline supports syntax highlighting via a
`Highlighter` trait. orchard implements `RadianHighlighter` in
`src/prompt.rs` which applies basic R token coloring. This is functional
but far from fish's polish. Fish shows which tokens are *valid* (not just
which tokens are *recognized*), which requires evaluation — harder in a
compiled language where functions aren't loaded yet.

**3.1.3 Web-Based Configuration**

Fish provides `fish_config` — a web UI served from localhost that lets
users:
- Change prompt colors and themes
- View and edit functions
- Browse history
- Configure environment variables

Strengths:
- **GUI for non- CLI users:** Not everyone wants to edit config files.
  A web UI makes configuration accessible.
- **Live preview:** Changes appear instantly. No restart needed.
- **Theme gallery:** Pre-built color schemes users can browse and apply.

**Relevance to orchard:** orchard controls syntax colors via the
`%colors` magic (monokai, solarized, native, none) and REPL behavior
via R options. A web config UI would be excessive for a REPL, but
`%colors` could be expanded to support more themes and a `%colors --preview`
flag that shows all token types rendered in the current scheme.

**3.1.4 Man-Page Completion**

Fish generates completions from man pages automatically. When you type
`ls --<TAB>`, fish reads `man ls`, parses the flag definitions, and
presents them as completions.

Strengths:
- **Zero-config autocomplete:** Every command with a man page gets
  completions instantly. No manual completion scripts needed.
- **Always up-to-date:** Completions reflect the installed version's
  actual flags, not a stale completion file.
- **Works for any binary:** Not just common commands — `--<TAB>` works
  for any program with a man page.

**Relevance to orchard:** R's `utils:::.completeToken()` already provides
function-aware completion for R commands. For shell mode (`;`), orchard
currently does path completion only (via `src/completion.rs`). Fish's
man-page approach isn't directly applicable (R packages don't have man
pages in the Unix sense), but the principle of "completions from the
source of truth" is worth noting — orchard already follows this by
delegating R completion to R itself.

**3.1.5 Smart Tab Completion**

Fish's tab completion has several refinements over traditional shells:
- First TAB shows completions, second TAB cycles through them
- Completions are filtered by prefix, and the common prefix is expanded
  automatically (like bash but with a visual list)
- `cd` completes directories only; `command <TAB>` completes files only;
  `export <TAB>` completes variable names

Strengths:
- **Context-aware filtering:** The completion list adapts to what the
  user is doing. `library(<TAB>` in R should show packages; `data$<TAB>`
  should show columns.
- **Visual selection:** The completion menu shows descriptions alongside
  each option, not just names.

**Relevance to orchard:** orchard's completion engine (`OrchardCompleter`
in `src/prompt.rs`) already does context-aware delegation: LaTeX first,
then packages in `library(` contexts, then R's completer. The visual
completion menu is provided by reedline's `ColumnarMenu`. Fish's
two-tab cycling behavior could be a quality-of-life improvement for
reedline integration.

### 3.2 Fish Features Relevant to Statistical Programming

| Feature | Statistical Programming Use Case | orchard Status |
|---------|--------------------------------|----------------|
| Autosuggestions | Re-run common R expressions faster | ✅ Wired via DefaultHinter, could improve |
| Syntax highlighting | Spot unmatched parens, bad quoting in real time | ✅ Basic highlighting active |
| Context-aware completion | library() → packages, data$ → columns | ✅ Via R's completer |
| Visual completion menu | See descriptions alongside completions | ✅ Via reedline ColumnarMenu |
| Error highlighting | See broken R code before running | ❌ Partial — needs valid-function detection |
| Web config UI | Theme and behavior preview | ❌ Excessive — %colors covers this |
| Man-page completion | Not directly applicable to R | ❌ N/A — R uses help() not man |

### 3.3 What orchard Should Learn from Fish

1. **Autosuggest quality matters.** Fish's autosuggestion feature is the
   single biggest productivity improvement over traditional shells. orchard
   has the wiring (DefaultHinter + OrchardHistoryBackend) but should ensure
   suggestions consider frequency, recency, and mode filtering.

2. **Syntax highlighting is an error-prevention tool.** Beyond cosmetic
   coloring, highlighting should help users spot mistakes: unmatched
   quotes, invalid function names (where detectable), and mismatched
   brackets. The `cursor_in_string` guard in the editing hook already
   prevents auto-pair insertion inside strings — this is the same principle.

3. **Completions should be context-aware at every level.** orchard already
   does this for `library()`, `::`, and `$` (via R's completer), but
   adding context awareness for more R patterns (formula interfaces `~`,
   mapping aesthetics `aes()`, dplyr verbs) would improve the experience.

4. **Dimmed-text suggestions are better than popup suggestions.** Fish
   shows the suggestion inline, not in a popup. This is less visually
   disruptive. reedline's current suggestion display could be tuned to
   match this behavior.

---

## 4. Julia REPL — Modal Design for Scientific Computing

Julia's REPL is the closest analogue to what orchard aims to be: a modal,
language-specific enhanced REPL for a scientific computing language. Unlike
IPython (which layers on top of Python), Julia's REPL is built into the
language — it is the native way to interact with Julia.

### 4.1 Core Design Strengths

**4.1.1 Modal Prompt System**

Julia's REPL has four modes, each with a distinct prompt:

| Mode | Prompt | Activation | Usage |
|------|--------|------------|-------|
| Julian (default) | `julia>` | Default | Julia code evaluation |
| Shell | `shell>` | Type `;` at line start | Run shell commands |
| Help | `help?> ` | Type `?` at line start | Look up function documentation |
| Package | `pkg> ` | Type `]` at line start | Package management (Pkg.jl) |

Strengths:
- **Single-key mode switching:** `;`, `?`, and `]` at the first column
  instantly switch modes. No `%` prefix, no `!` prefix — just the key.
- **Visual prompt change:** The prompt string changes to reflect the
  current mode. `julia>` → `shell>` → `help?>` is immediately obvious.
- **Backspace exits:** Backspace at column 0 in shell/help/pkg mode
  returns to Julian mode. This is the same pattern Radian and orchard use.
- **Mode-specific history:** Shell commands don't pollute Julia history.
  Help lookups aren't stored in history at all.

**Relevance to orchard:** orchard already implements shell mode (`;` →
shell commands, backspace at column 0 → R mode). This matches Julia's
design. Julia's help mode (`?`) is closer to IPython's `?name` than a
full modal prompt — orchard could implement `?` as a prefix that routes
to `%pdoc` or `help()`.

**4.1.2 Paste Detection**

Julia's REPL detects when pasted text contains multiple lines and handles
it intelligently:
- If the paste ends with a complete expression, it's evaluated immediately
- If the paste ends with an incomplete expression, a new prompt is shown
- Leading `julia>` prompts in pasted text are stripped (so you can paste
  REPL transcripts directly)

Strengths:
- **Transcript-friendly:** Users can paste directly from documentation,
  tutorials, or previous sessions without editing.
- **Multi-line handling:** The paste detector respects Julia's syntax —
  function bodies, loops, and try/catch blocks are recombined correctly.

**Relevance to orchard:** Bracketed paste is handled by reedline natively
and further customized in orchard's editing hook (`src/editing_hook.rs`).
Julia's prompt-stripping on paste is a nice touch that orchard could adopt
for R REPL transcripts.

**4.1.3 ANSI Colors in Prompt**

Julia's default REPL prompt uses ANSI escape codes for colored output:
- `julia>` in bold green
- Error messages in bold red
- Shell command output in plain text
- Help output with formatted type signatures

Strengths:
- **Information density without noise:** Color encodes semantics (green =
  ready for input, red = error, yellow = warning). Users scan visually.
- **Low implementation cost:** ANSI escape codes are just formatted
  strings — no special rendering pipeline needed.

**Relevance to orchard:** orchard already uses ANSI colors in prompts
(configured via `radian.shell_prompt`, `radian.browse_prompt`, etc.). The
`%colors` magic (implemented in `src/magics/config.rs`) controls the
syntax highlighting color scheme.

**4.1.4 SIGINT Handling**

Julia's REPL handles Ctrl-C gracefully:
- Interrupts a running computation
- Returns to the `julia>` prompt without crashing
- Shows a brief cancellation message (not a traceback)
- Can be used during package installation, long computations, and REPL
  input

Strengths:
- **Safe interrupt:** Ctrl-C never crashes the REPL. The state is
  preserved.
- **Immediate feedback:** The REPL responds instantly to Ctrl-C with a
  clear "cancelled" signal.

**Relevance to orchard:** orchard handles SIGINT via signal handler
(`src/r_runtime.rs` — `CONSOLE` state's `interrupted` flag, `CtrlC`
result from `ReadResult`). A manual SIGINT test exists but is
`#[ignore]`d because it requires interactive terminal input. This
is a known risk documented in the review.

**4.1.5 Tab Completion**

Julia's tab completion:
- Completes function names, variable names, and types
- Shows function signatures in the completion menu
- Completes file paths in string literals
- Completes package names after `using` and `import`
- Shows documentation previews in the completion tooltip

Strengths:
- **Signature preview:** Seeing a function's argument names and types
  in the completion menu is invaluable for exploratory programming.
- **Unified interface:** R completion, file completion, and package
  completion are all handled by one system. No "this mode or that mode"
  confusion.

**Relevance to orchard:** orchard delegates R completion to R's
`utils:::.completeToken()` which provides signature information. The
completion menu is handled by reedline's `ColumnarMenu`. The package
completion for `library()` and `require()` is implemented in
`src/completion.rs`.

### 4.2 Julia REPL Features Relevant to Statistical Programming

| Feature | Statistical Programming Use Case | orchard Status |
|---------|--------------------------------|----------------|
| `;` shell mode | File ops, git without leaving R | ✅ Implemented |
| `?` help mode | Quick documentation access | ❌ Not as modal prompt |
| ANSI colored prompts | Visual mode distinction | ✅ Via R options |
| Paste detection | Transcript-friendly multi-line paste | ✅ Reedline bracketed paste |
| SIGINT handling | Safe interrupt of long computations | ✅ Implemented, manual test ignored |
| Tab completion with signatures | Discover function arguments | ✅ Via R's completer |
| Backspace-at-0 exits mode | Return to R from shell mode | ✅ Editing hook |
| Mode-specific history | Shell vs R vs Browse partitioning | ✅ History module |

### 4.3 What orchard Should Learn from Julia's REPL

1. **Help mode (`?`) is a natural extension.** orchard could detect `?`
   at line start and route to `%pdoc`/`%pdef` instead of passing the
   `?` to R (which treats it as a incomplete expression). This matches
   both Julia's and IPython's help semantics.

2. **Package mode (`]`) maps to R's library management.** `]` → `pkg>`
   could map to install.packages/remove.packages/library workflows. This
   is lower priority than help mode.

3. **Prompt-stripping on paste would improve REPL transcript workflows.**
   If a user copies code from an orchard session that includes `> `
   prompts, stripping them on paste would match Julia's user-friendly
   behavior.

4. **SIGINT handling quality needs manual acceptance testing.** The
   implementation exists and works in automated tests, but the interactive
   Ctrl-C experience (message clarity, state preservation) is untested
   without a manual session.

---

## 5. Harlequin SQL — The Modern TUI for Data Querying

Harlequin is a modern terminal SQL IDE built in Python with Textual. It is
the newest tool in this analysis and the one most focused on a specific data
workflow: interactive SQL querying against database connections. Its design
choices for in-terminal data browsing are directly relevant to orchard's
planned TUI data viewer (deferred to post-v1 in the roadmap).

### 5.1 Core Design Strengths

**5.1.1 Interactive Data Browser**

Harlequin's main pane shows query results as an interactive table:
- Scrolling through rows and columns with arrow keys or vim bindings
- Column headers with type indicators (INT, VARCHAR, DATE, etc.)
- Column width auto-sizing with manual override
- Row count display
- Cell value preview for long content

Strengths:
- **Zero-config browsing:** Query results appear in a sortable, scrollable
  table instantly. No `head()`, no `print()`, no scrolling through raw text.
- **Schema visibility:** Column types are displayed in the header. Users
  see at a glance that `price` is a FLOAT and `date` is a DATE.
- **Keyboard-navigable:** Full keyboard control — no mouse needed for
  analysts who prefer the keyboard.

**Relevance to orchard:** orchard has `%View` (opens R's GUI `View()`),
`%head` (prints first rows), `%str` (prints structure), and `%skim`
(summary stats). None of these provide an in-terminal interactive table
browser. Harlequin's data browser is the model for what orchard's
deferred TUI viewer should be.

**5.1.2 Schema-Aware Autocomplete**

Harlequin provides SQL autocomplete that is aware of the connected
database's schema:
- Table names from the current database
- Column names for the current table context (after `FROM table` or
  `JOIN table`)
- SQL keywords
- Function names
- Context-sensitive: `SELECT col<TAB>` after `FROM table` suggests
  columns of that table, not all columns in the database

Strengths:
- **Reduces keystrokes significantly:** Long column names, multi-part
  table references, and JOIN conditions are completed automatically.
- **Discoverability:** Users learn the schema by exploring completions.
  Available tables and columns appear in the completion menu.
- **Context-respecting:** Completion adapts to the current query context.
  `FROM orders` means completions are drawn from `orders` columns.

**Relevance to orchard:** orchard's R completion uses `utils:::.completeToken()`
which provides R-level schema awareness (dataframe columns after `$`,
function arguments). For SQL-like workflows within R (dplyr, data.table,
DBI queries), adding Harlequin-style schema-aware completion would
require:
- Detecting when the user is writing a dplyr chain or DBI query
- Querying R for the schema of referenced objects
- Presenting column/table completions in context

**5.1.3 Query History with Search**

Harlequin saves all executed queries and provides:
- Searchable history (filters as you type)
- History persists across sessions
- Keyboard shortcut to recall and re-run
- Query timing (duration displayed next to each query)

Strengths:
- **Retrieval over recall:** Users don't need to remember exact query
  syntax — they search their history for a similar query and adapt it.
- **Performance feedback:** Query timing is displayed automatically.
  Users naturally optimize slow queries because the duration is visible.

**Relevance to orchard:** orchard has `%hist` and `%hist_n` for history
display but no `%rerun` or `%recall`. Harlequin's searchable history
with one-key re-execution is a workflow that orchard's history magics
should match. Query timing is handled by `%time`/`%timeit` but isn't
automatic.

**5.1.4 Multiple Connection Support**

Harlequin connects to multiple databases simultaneously:
- DuckDB (local analytics, parquet, CSV)
- SQLite
- PostgreSQL
- MotherDuck (cloud DuckDB)
- Each connection has its own schema context

Strengths:
- **Cross-database workflows:** Query a PostgreSQL table, join with a
  local CSV via DuckDB, write results back — all in one session.
- **Connection management:** Connect/disconnect without restarting.
  Switch between connections for different schemas.

**Relevance to orchard:** orchard is R-focused, not SQL-focused. However,
R users frequently use DBI/odbc for database access. The ability to
manage database connections from the REPL (list connections, switch
default, test queries) would complement orchard's data analysis workflow.

**5.1.5 File-Based Query Management**

Harlequin treats `.sql` files as the primary unit of work:
- Open a `.sql` file → it becomes the query buffer
- Ctrl-S saves the buffer back to the file
- Multiple files can be open (tabbed interface)
- Query output and file editing are in the same window

Strengths:
- **Version control integration:** `.sql` files are git-friendly. Query
  history becomes project history.
- **Reproducibility:** A `.sql` file is a reproducible analysis step.
  Open, run, save → the file documents the analysis.

**Relevance to orchard:** orchard has `%load` (display file contents) and
`%run` (source an R script). Harlequin's file-as-editor-buffer pattern
is closer to how `%edit` works (opens `$EDITOR`, sources on save).
Combining `%edit` with `%run` provides a similar workflow: edit an R
script in `$EDITOR`, source it on exit via `%run`.

### 5.2 Harlequin Features Relevant to Statistical Programming

| Feature | Statistical Programming Use Case | orchard Status |
|---------|--------------------------------|----------------|
| Interactive data browser | Browse data frames without head()/print() | ❌ Post-v1 deferred TUI viewer |
| Schema-aware autocomplete | Column names after $, table names in dplyr | ✅ Via R's completer (partial) |
| Searchable query history | Find and re-run previous analysis steps | ✅ %hist exists, ❌ %rerun missing |
| Query timing display | See how long operations take automatically | ✅ %timeit exists, ❌ not automatic |
| Multiple connections | Work with multiple databases/sources | ❌ Not implemented for R DBI |
| File-as-editor pattern | Edit and source .R files from REPL | ✅ %edit + %run cycle |
| Result export | Save query results to CSV/parquet | ❌ Not implemented |
| DuckDB integration | Query parquet, CSV, JSON directly | ❌ R already handles this |

### 5.3 What orchard Should Learn from Harlequin

1. **The TUI data browser is the most impactful missing feature.** orchard
   can display data via `%head`, `%str`, and R's `View()`, but none of
   these match Harlequin's in-terminal interactive table browsing. Adding
   a `comfy-table` or `ratatui`-based data browser would leapfrog the
   current data inspection experience. This is already in the roadmap as
   deferred post-v1.

2. **Schema-aware autocomplete should be enhanced for dplyr/data.table.**
   R's `utils:::.completeToken()` handles `$` for base R data frames but
   dplyr's `%>%` pipe chains and data.table's `DT[,` syntax need R-level
   completion awareness. Detecting the active tidyverse/data.table context
   and querying R for the schema of piped objects would bring orchard
   closer to Harlequin's schema-aware experience.

3. **Searchable history with one-key re-execution is a daily workflow.**
   `%hist` displays history but `%rerun` and `%recall` are not implemented.
   Harlequin's Ctrl-R search → Enter re-execute pattern should be orchard's
   target for history replay.

4. **File-as-editor-buffer is the right pattern for R scripts.** orchard's
   `%edit` + `%run` combination already provides this, but the integration
   could be tighter: auto-sourcing on editor exit (without the user having
   to `%run` manually) would match Harlequin's seamless edit→run cycle.

5. **DBI/odbc connection management would complement data analysis.** R
   users connect to databases via DBI. A `%connections` magic that lists
   active DBI connections, shows their schemas, and runs test queries
   would bring database-aware workflows into the REPL.



