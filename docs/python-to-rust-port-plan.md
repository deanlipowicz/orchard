# radian Python-to-Rust Port Plan

## 1. Objective

Build a Rust implementation of the core `radian` terminal application behavior.

The goal is behavior compatibility with the Python application, not Python API
compatibility. The Rust binary should eventually provide an interactive R
console with radian-style prompts, history, completion, multiline editing,
syntax highlighting, shell escape mode, profile loading, and embedded R event
loop integration.

This repository currently contains:

- A Rust binary crate named `radian-rs`.
- A sparse upstream reference checkout at `third_party/radian-upstream/radian`.
- This implementation plan and developer review log.
- A working Linux-first Rust implementation of the core embedded R REPL path.

Do not start by rewriting every file mechanically. Start by building the
minimal embedded R REPL, then add radian behavior in layers.

## 2. Upstream Snapshot

Source reviewed:

- URL: `https://github.com/randy3k/radian/tree/master/radian`
- Downloaded reference path: `third_party/radian-upstream/radian`
- Upstream commit: `a7cb91a99b2361404f3baab031cc18b935353660`

The upstream package is small, but the behavior is spread across several
callback and prompt-toolkit extension points. The important source files are:

| Upstream file | Role in Python radian | Rust port treatment |
| --- | --- | --- |
| `app.py` | CLI parsing, R discovery, environment setup, embedded R startup, callback registration, welcome output | Port directly into `cli`, `r_discovery`, `env_setup`, and `app` modules |
| `dyld.py` | Unix/macOS dynamic library path repair and re-exec | Port early; embedded R will fail without correct loader paths |
| `console.py` | R `read_console` and `write_console_ex` callback behavior | Port as the core bridge between R and the Rust prompt session |
| `prompt_session.py` | Modal prompt setup for R, browse, shell, unknown, settings application, input hook | Port as explicit `PromptSession` state machine |
| `lineedit/*.py` | Modal history, buffer search, prompt-toolkit overrides | Reimplement behavior over Rust line editor primitives |
| `settings.py` | R-option-backed radian settings and defaults | Port as `Settings::load_from_r_options` |
| `completion.py` | R completion, package completion, LaTeX completion, shell path completion | Port after core prompt works |
| `key_bindings.py` | Most visible editing behavior: enter handling, auto-pairs, indentation, paste, shell escape, editor integration | Port incrementally after prompt and history |
| `lexer.py` and `document.py` | R highlighting and `cursor_in_string` guard | Port a lightweight scanner; avoid full R parser in v1 |
| `rutils.py` | Profile loading, installed packages, cleanup, Windows UTF-8, hooks | Port helper-by-helper as needed |
| `shell.py` | `;` shell mode and `cd` handling | Port directly |
| `latex/*` | LaTeX symbol completion data | Convert data into Rust static table or generated file |

## 2.1 Current Review, 2026-06-29

This review compares the current Rust tree against the upstream Python checkout
at `third_party/radian-upstream/radian`. It is a status overlay for the phase
plan below, not a replacement for the original target behavior.

Review basis:

- Rust source in `src/`, integration tests in `tests/embedded_r.rs`, and docs in
  `docs/`.
- Upstream Python files `app.py`, `console.py`, `prompt_session.py`,
  `completion.py`, `key_bindings.py`, `settings.py`, `rutils.py`, `shell.py`,
  `lineedit/history.py`, `lexer.py`, and `document.py`.
- Verification commands from this review:
  - `cargo test -- --test-threads=1`
  - `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture --test-threads=1`

Current phase status:

| Phase | Current status | Completed functionality | Missing or weak functionality |
| --- | --- | --- | --- |
| 0. Project and Build Skeleton | Sufficient | `Cargo.toml`, `build.rs`, R discovery for build, link to `libR`, generated R bindings, explicit missing-R failure. | The implemented layout is flatter than the proposed future layout; do not split modules unless a change needs it. |
| 1. CLI and Environment Setup | Sufficient for v1 | Public flags and compatibility flags parse, `--vanilla` expands, R env vars and R dir vars are set, local history file can be created, `--version` reports Rust/R paths. | `--ask-save` is currently treated like `--save`; Python radian leaves ask-save semantics to R. Windows `CMDER_ROOT` compatibility from upstream is not implemented. |
| 2. Dynamic Library Path Repair | Sufficient for Linux/macOS v1 | Unix loader path repair, one-time re-exec marker, `R_LD_LIBRARY_PATH`, Linux loader var, guarded macOS DYLD cleanup and best-effort BLAS injection. | macOS behavior is not acceptance-tested here and does not parse Mach-O load commands like upstream's optional `lief` path. Windows bypasses this phase. |
| 3. Embedded R Runtime | Sufficient for v1 | Embedded R initialization, console callback registration, REPL driving, eval/source/options helpers, string call helper, protect guard, R error context, embedded R subprocess tests. | `parse_complete` is a Rust heuristic, not R's parser; complex R syntax may differ. Cleanup/finalizer hooks equivalent to upstream `register_cleanup` are not implemented. |
| 4. Settings and Profile Loading | Sufficient for Linux v1 | R-option-backed settings, upstream defaults, prompt override behavior, explicit/XDG/global/local profile order. | Windows R-expanded `~/.radian_profile` fallback is missing. Settings for autosuggest, matching-bracket highlight, `complete_while_typing`, and some editor behavior are parsed but not fully honored by the live editor. |
| 5. Console Callback Bridge | Sufficient for v1 with one manual risk | stdout/stderr callbacks, stderr formatting, suppression flags, cursor tracking, EOF, Ctrl-C callback interrupt raise, piped multiline input, UTF-8-safe chunking, nested prompt fallback, terminal width sync, Unix R polled-event timer. | Manual SIGINT acceptance for long-running R expressions remains ignored/environment-sensitive. Non-Unix event processing is a no-op. Upstream askpass setup is not implemented. |
| 6. Prompt Modes | Sufficient for v1 | R, browse, shell, and unknown prompt detection/display; persistent shell prompt for `;`; one-shot `;command`; vi mode prompt display. | Browser prompt behavior is not covered by an integration/manual acceptance result in the repo. Reticulate prompt mode from upstream is not implemented. |
| 7. History | Sufficient for file format and append/search | Compatible parser/writer, mode labels, mode-book compatibility, duplicate filtering, browser-command filtering, trimming, local/global path selection, unit tests. | Loaded radian history is not wired into reedline navigation/search/autosuggest, so interactive arrow/history behavior is weaker than upstream. |
| 8. Completion | Partial | R completion is wired and seeds R token state, package completion uses installed packages, shell path completion exists, small LaTeX seed table exists, embedded R coverage verifies base function/package availability. | LaTeX table is only five symbols versus upstream's large table. No explicit-vs-automatic completion timeout distinction. Package-context parsing is shallow. Shell completion is not explicitly gated to manual completion events. `utils::rc.settings(ipck=TRUE)` is not set. |
| 9. Key Bindings and Editing Behavior | Sufficient for v1 | All 13 Phase 9 keybindings are live via the pre-edit hook: context-aware auto-pairs, closing-delimiter skip, smart backspace (pair/indent/shell-exit), Enter indent, Tab indent, raw R string pairs, bracketed-paste strip, Ctrl-X Ctrl-E editor, Ctrl-C menu dismiss. | Autosuggest (`radian.auto_suggest` parsed but not forwarded), matching-bracket highlight (`radian.highlight_matching_bracket` parsed but not wired), and custom key maps (`radian.escape_key_map`/`ctrl_key_map` not parsed). |
| 10. Lexer and String Detection | Sufficient lightweight v1 | Tokenizer covers comments, whitespace, names/backticks, numbers, operators, punctuation, normal strings, raw strings, and `cursor_in_string`; highlighter is wired. | This is not a full R lexer/parser and does not aim for pygments parity. Matching-bracket highlighting setting is parsed but not implemented. |
| 11. Shell Mode | Sufficient for Unix v1 | Shell command execution, `cd`, `cd -`, `~` and env expansion, Unix shell splitting, tests. | Windows shell behavior is a compile-time fallback and is not validated. |
| 12. Packaging and Platform Support | Partial | Linux is the exercised target. macOS-specific loader helpers compile behind cfgs. Windows UTF-8 env setup partially sets `LANG` for R >= 4.2. | No packaging/install story, no CI matrix, macOS/Windows acceptance missing, Windows R option `encoding = "UTF-8"` missing, non-Unix input hook missing. |

Current milestone status:

- **Milestone A: Sufficient.** CLI, discovery, loader setup, embedded R, basic
  evaluation, and callbacks are implemented and tested.
- **Milestone B: Sufficient for v1.** Settings, profiles, prompt basics,
  multiline input, EOF, resize behavior, nested prompt fallback, and Unix
  event-loop processing are present. Manual long-running SIGINT remains a risk.
- **Milestone C: Sufficient.** History file compatibility, shell mode, AND
  loaded history navigation (Ctrl-R, up/down-arrow with mode filtering)
  are all live.
- **Milestone D: Partial.** Completion and first-pass editing are live, but
  upstream-equivalent completion semantics and context-aware keybindings remain.
- **Milestone E: Partial.** Linux is tested; macOS and Windows are documented
  but not hardened.

Current implementation plan from this review:

1. Keep the flat module layout until a specific change makes splitting cheaper
   than keeping it.
2. Finish Phase 8 before claiming completion parity: import/generate the full
   LaTeX table, separate automatic and explicit completion behavior if reedline
   exposes the event, and strengthen package context detection.
3. Finish Phase 9 Phase 2 only if reedline can support contextual editing hooks
   or a small custom `EditMode` proves cheaper than scattered workarounds.
4. ~~Wire loaded history into reedline before expanding autosuggest/history search
   features; file-format support already exists.~~ **Done** — `RadianHistoryBackend`
   implements reedline's `History` trait, seeded from radian's loaded entries,
   providing Ctrl-R and up/down-arrow navigation with mode-aware filtering.
5. Treat reticulate prompt integration, custom key maps, on-load hooks, askpass,
   and full Windows/macOS parity as post-core compatibility work unless a user
   explicitly needs them.

## 2.2 Strategic Steering Release Framework

Use release gates to steer the project after the current Linux-first core is
complete. Phase status tracks implementation detail; release gates decide what
the project is allowed to claim.

Release lanes:

- **Core Parity Lane:** finish Phase 8 completion parity, Phase 9
  context-aware editing, loaded history navigation/search, browser prompt
  acceptance, and long-running Ctrl-C acceptance before claiming core parity.
- **Platform Lane:** validate Linux first, then macOS, then Windows. Do not
  claim a platform as supported without an acceptance result in the developer
  log.
- **Compatibility Lane:** track upstream-only behavior separately from core
  parity: reticulate prompt integration, custom key maps, on-load hooks,
  askpass setup, cleanup/finalizer hooks, and full Windows UTF-8 behavior.
- **Maintenance Lane:** after core parity, prioritize regression tests,
  release packaging, user docs, dependency updates, and compatibility with
  supported R versions.

Release gates:

| Gate | Claim allowed | Required evidence |
| --- | --- | --- |
| `v0.1 Experimental` | Linux-first Rust REPL is usable for basic sessions. | Default tests pass, real-R gated tests pass, current gaps are documented. |
| `v0.2 Core Parity` | Core Python radian workflows are matched on Linux. | Phase 8 is sufficient (LaTeX table expanded), loaded history navigation works, autosuggest is wired, custom key maps work, matching-bracket highlight works, browser prompt and long-running Ctrl-C acceptance are recorded. |
| `v0.3 Platform Beta` | macOS is beta-supported; Windows status is explicit. | macOS acceptance passes with a named R install path. Windows either passes startup/shell/console/UTF-8 checks or is explicitly excluded from this gate. |
| `v1.0 Replacement Candidate` | `radian-rs` can be recommended as a replacement for supported workflows. | Core parity holds, supported platforms have acceptance logs, install/update docs exist, and deferred compatibility features are either implemented or listed as non-goals. |

Priority rules:

1. Finish incomplete user-facing core workflows before adding secondary upstream
   compatibility features.
2. Do not split modules, add dependencies, or build framework code unless a
   current gate requires it.
3. Promote a phase or platform only with test or manual acceptance evidence in
   `docs/developer-log.md`.
4. Keep deferred upstream features visible, but do not let them block core
   parity unless a real user workflow depends on them.

## 3. Non-Negotiable Compatibility Decisions

1. The Rust implementation embeds R directly through R's C API.
   Do not keep Python or `rchitect` in the runtime.

2. The first usable milestone is a correct embedded R console.
   Do not start with syntax highlighting or keybinding polish.

3. Preserve the existing history file format so users can reuse
   `~/.radian_history`.

4. Preserve radian's profile loading order and R option names.
   Existing `.radian_profile` files should continue to control settings.

5. Treat terminal/editor library limitations as implementation constraints.
   If `reedline` cannot support a required behavior cleanly, wrap or fork the
   needed pieces instead of dropping the behavior silently.

## 4. Proposed Rust Architecture

The original target layout was:

```text
src/
  main.rs
  app.rs
  cli.rs
  env_setup.rs
  r_discovery.rs
  r_runtime/
    mod.rs
    ffi.rs
    callbacks.rs
    protect.rs
  settings.rs
  prompt/
    mod.rs
    session.rs
    mode.rs
    history.rs
    keymap.rs
    completion.rs
    lexer.rs
    shell.rs
  latex.rs
  platform/
    mod.rs
    unix.rs
    macos.rs
    windows.rs
```

The current implementation intentionally keeps a flatter `src/*.rs` layout.
Keep it unless a real ownership boundary or repeated edit pain justifies a
split.

Recommended crates:

- `clap` with `derive` for CLI parsing.
- `anyhow` for application-level errors.
- `thiserror` for typed module errors.
- `tracing` and `tracing-subscriber` for debug logging.
- `libc` for FFI types.
- `bindgen` as a build dependency for R headers.
- `cc` as a build dependency if C shims become necessary.
- `regex` for prompt and completion guards.
- `dirs` for user config and history paths.
- `shell-words` for Unix shell command parsing.
- `unicode-width` for prompt display correctness.
- `crossterm` plus `reedline` as the first attempt for line editing.
- `nu-ansi-term` for ANSI formatting.

If `reedline` blocks modal prompt behavior, keep the same public module layout
and replace only the implementation behind `prompt/session.rs`.

## 5. Implementation Phases

### Phase 0: Project and Build Skeleton

Steps:

1. Add the recommended dependencies to `Cargo.toml`.
2. Add `build.rs`.
3. In `build.rs`, discover `R_HOME` in this order:
   - `R_HOME` environment variable.
   - `R_BINARY` environment variable followed by `R RHOME`.
   - `R` found on `PATH` followed by `R RHOME`.
4. Emit link search path for `$R_HOME/lib`.
5. Link against `R`.
6. Generate bindings for the minimal R headers:
   - `Rembedded.h`
   - `Rinterface.h`
   - `Rinternals.h`
7. If R is not installed, make the build failure explicit:
   `R was not found. Install R or set R_HOME/R_BINARY.`

Acceptance:

- `cargo check` succeeds on a machine with R installed.
- The error is readable when R is missing.

### Phase 1: CLI and Environment Setup

Port the options from `app.py` exactly.

Public CLI flags:

```text
-v, --version
--r-binary PATH
--profile PATH
-q, --quiet, --silent
--no-environ
--no-site-file
--no-init-file
--local-history
--global-history
--no-history
--vanilla
--save
--ask-save
--restore-data
--debug
--coverage
--cprofile
```

Also accept but ignore these compatibility flags:

```text
--no-save
--no-restore-data
--no-restore-history
--no-restore
--no-readline
--interactive
```

Behavior:

1. `--r-binary` sets `R_BINARY`.
2. `--vanilla` expands to:
   - `--no-history`
   - `--no-environ`
   - `--no-site-file`
   - `--no-init-file`
3. `--version` prints:
   - radian-rs version
   - R executable
   - R version
   - Rust executable path if available
4. Set:
   - `RADIAN_VERSION`
   - `RADIAN_COMMAND_ARGS`
   - `R_DOC_DIR`
   - `R_INCLUDE_DIR`
   - `R_SHARE_DIR`
5. If `--no-environ`, set:
   - `R_ENVIRON=`
   - `R_ENVIRON_USER=`
6. If `--no-site-file`, set `R_PROFILE=`.
7. If `--no-init-file`, set `R_PROFILE_USER=`.
8. If `--local-history`, create `.radian_history` if it does not exist.

R path fallback:

- First check `$R_HOME/doc`, `$R_HOME/include`, `$R_HOME/share`.
- If missing, run:
  ```text
  R --no-echo --vanilla -e "cat(paste(R.home('doc'), R.home('include'), R.home('share'), sep=':'))"
  ```

Acceptance:

- CLI parsing unit tests cover every flag.
- `--vanilla` expansion is tested.
- Environment variable setup is tested with temporary process env isolation.

### Phase 2: Dynamic Library Path Repair

Port `dyld.py`.

Unix behavior:

1. Check whether `$R_HOME/lib` is present in `R_LD_LIBRARY_PATH`.
2. If missing, compute `R_LD_LIBRARY_PATH` from `$R_HOME/etc/ldpaths` when
   present.
3. Set:
   - `R_LD_LIBRARY_PATH`
   - `LD_LIBRARY_PATH` on Linux and other Unix
   - `DYLD_FALLBACK_LIBRARY_PATH` on macOS
4. Re-exec the current executable once after changing loader paths.

macOS behavior:

1. Implement the Python behavior of clearing a previous
   `R_DYLD_INSERT_LIBRARIES` entry from `DYLD_INSERT_LIBRARIES`.
2. Add `libRblas.dylib` injection only if needed and discoverable.
3. If Mach-O parsing is too much for v1, implement the best-effort fallback:
   `$R_HOME/lib/libRBlas.dylib`.

Acceptance:

- Unit test path string composition.
- Manual test on Linux verifies the process re-execs once and embeds R.
- macOS behavior is guarded behind platform cfgs.

### Phase 3: Embedded R Runtime

Create a safe wrapper around R's unsafe C API.

Core responsibilities:

1. Initialize embedded R with args equivalent to:
   ```text
   radian --quiet --no-restore-history --no-readline
   ```
   plus the user's init/save/restore flags.
2. Register console callbacks:
   - `ReadConsole`
   - `WriteConsoleEx`
   - busy callback if required by the selected R API path
3. Drive R's main loop until exit.
4. Provide safe helpers:
   - `eval_string(code) -> RResult<Sexp>`
   - `call(package, function, args) -> RResult<Sexp>`
   - `get_option(name) -> RValue`
   - `set_option(name, value)`
   - `source_file(path)`
   - `parse_complete(code) -> bool`
   - `installed_packages() -> Vec<String>`
5. Implement a small `Protect` guard for `PROTECT`/`UNPROTECT`.

Important:

- All R API calls should stay on the main thread unless proven safe.
- Keep unsafe blocks inside `r_runtime`.
- Convert R errors into Rust errors with enough context to debug the failing
  expression.

Acceptance:

- Integration test starts embedded R and evaluates `1 + 1`.
- Integration test sets and reads an R option.
- Integration test sources a temporary R file.

### Phase 4: Settings and Profile Loading

Port `settings.py` and profile helpers from `rutils.py`.

Settings defaults:

```text
auto_suggest = false
emacs_bindings_in_vi_insert_mode = false
editing_mode = "emacs"
color_scheme = "native"
auto_match = true
highlight_matching_bracket = false
auto_indentation = true
tab_size = 4
complete_while_typing = true
completion_timeout = 0.15
completion_prefix_length = 2
completion_adding_spaces_around_equals = true
history_size = 20000
global_history_file = "~/.radian_history"
local_history_file = ".radian_history"
history_search_no_duplicates = false
history_search_ignore_case = false
history_ignore_browser_commands = true
insert_new_line = true
indent_lines = true
shell_prompt = "\x1b[31m#!>\x1b[0m "
browse_prompt = "\x1b[33mBrowse[{}]>\x1b[0m "
show_vi_mode_prompt = true
vi_mode_prompt = "\x1b[34m[{}]\x1b[0m "
stderr_format = "\x1b[31m{}\x1b[0m"
auto_width = getOption("setWidthOnResize", TRUE)
```

Prompt behavior:

- If `getOption("radian.prompt")` is set, use it.
- Else if `getOption("prompt")` equals `"> "`, use
  `"\x1b[34mr$>\x1b[0m "`.
- Else use R's `prompt` option.

Profile loading order:

1. If `--profile PATH` is provided and exists, source only that file.
2. Otherwise source XDG profile if it exists:
   - `$XDG_CONFIG_HOME/radian/profile`
   - or `~/.config/radian/profile` on Unix
   - or `~/radian/profile` on Windows
3. Source global `~/.radian_profile` if it exists.
4. On Windows only, also support R-expanded `~/.radian_profile` fallback.
5. Source local `.radian_profile` if it exists and is not the same path as the
   global profile.

Acceptance:

- Unit tests verify settings defaults.
- Integration tests verify R options override defaults.
- Profile loading order is tested with temporary files.

### Phase 5: Console Callback Bridge

Port `console.py`.

State to maintain:

- `terminal_cursor_at_beginning: bool`
- `suppress_stdout: bool`
- `suppress_stderr: bool`
- `interrupted: bool`
- stored text for long non-ASCII multiline input

Read console behavior:

1. If R asks for input while the prompt application is already running, fall
   back to native terminal input.
2. Otherwise set the prompt message and activate the correct mode.
3. Insert a leading newline when radian would do so:
   - terminal cursor is not at beginning, or
   - `insert_new_line` is enabled and current mode requests it.
4. On `Ctrl-C`, mark interrupted and raise the interrupt back to R.
5. On EOF, return `None` to R.
6. For long non-ASCII multiline input in R or browse mode:
   - wrap as `{\n...\n}`
   - feed line-by-line to avoid R splitting inside long strings.

Write console behavior:

1. stdout writes raw text unless suppressed.
2. stderr writes text formatted through `settings.stderr_format`.
3. Normalize CRLF and ANSI escapes only for cursor-position tracking.
4. Update `terminal_cursor_at_beginning` based on whether normalized output
   ends with `\n`.

Acceptance:

- Unit tests for ANSI normalization and cursor tracking.
- Integration test prints stdout and stderr from R.
- Manual test confirms `Ctrl-C` interrupts a long-running R expression.

### Phase 6: Prompt Modes

Port the modal behavior from `prompt_session.py`.

Modes:

| Mode | Activation | History book | Multiline | Completion |
| --- | --- | --- | --- | --- |
| `r` | R prompt equals configured prompt | `r` | yes if `indent_lines` | R completer |
| `browse` | prompt matches `Browse\[([0-9]+)\]> $` | `r` | yes if `indent_lines` | R completer |
| `shell` | user types `;` at beginning in R mode | `shell` | yes if `indent_lines` | path completer |
| `unknown` | fallback | none | no | none |

Required prompt rules:

- `r` prompt displays R's message.
- `browse` prompt displays configured `browse_prompt` with current browse
  level.
- `shell` prompt displays configured `shell_prompt`.
- `unknown` prompt displays whatever R sent.
- If vi mode is active and `show_vi_mode_prompt` is true, prepend configured
  vi mode prompt.

Input hook:

During prompt waiting:

1. If terminal width changed, set R option `width` to at least 20.
2. If R has pending events, temporarily detach line editor input, enter rare
   mode, and process events.
3. Otherwise poll events.
4. Sleep for roughly 1/30 second.

Acceptance:

- Unit tests for mode activation.
- Manual test in `browser()` confirms browse prompt detection.
- Manual test checks terminal resize updates `getOption("width")`.

### Phase 7: History

Port `lineedit/history.py` and the history-related parts of
`lineedit/buffer.py`.

History file format must remain:

```text

# time: 2026-06-28 12:34:56 UTC
# mode: r
+x <- 1
+x + 1
```

Behavior:

1. `--no-history` uses memory-only history.
2. `--local-history` or existing local history uses `.radian_history`.
3. Otherwise use expanded `global_history_file`.
4. Create parent directory for global history with mode `0700` on Unix.
5. Store mode with every entry.
6. Do not store empty input.
7. Do not store consecutive duplicate input with same mode.
8. Filter navigation/search by compatible mode:
   - same mode, or
   - both modes share the same `history_book`.
9. Support:
   - duplicate-free search if configured
   - case-insensitive search if configured
   - history trimming when file grows beyond configured size
10. In browse mode, do not store browser commands when
    `history_ignore_browser_commands` is true:
    `n`, `s`, `f`, `c`, `cont`, `Q`, `where`, `help`.

Acceptance:

- Unit tests load old radian history files.
- Unit tests store multiline history entries.
- Unit tests verify mode filtering.
- Unit tests verify browser command ignore list.

### Phase 8: Completion

Port `completion.py`.

R completion:

1. If current word length is below `completion_prefix_length` and completion
   was not explicitly requested, return no completions.
2. Try LaTeX completions first.
3. If any LaTeX completions match, return only those.
4. For R completions:
   - skip during `library(...)`, `require(...)`, and
     `requireNamespace(...)` package-name contexts.
   - call into R's completion machinery.
   - suppress stderr during completion.
   - timeout only for automatic completion, not explicit completion or
     `pkg::name` completion.
   - append spaces around `=` completions when configured.
   - skip completions ending with `::`; package completion handles these.

Package completion:

1. Token regex equivalent: `[a-zA-Z0-9._]+$`.
2. Installed packages come from `.packages(all.available = TRUE)`.
3. If cursor is in a string or in package-name context, complete package name.
4. Otherwise complete `package::`.

Shell path completion:

1. Only run on explicit completion request.
2. If command starts with `cd `, return directories only.
3. Expand `~` and environment variables.
4. Use current working directory for relative paths.
5. On Unix, escape spaces unless the path was quoted.
6. On Windows, normalize backslashes to slashes.

Acceptance:

- Unit tests for package context regexes.
- Unit tests for shell path completion.
- Integration test verifies R completion returns known base functions.
- Integration test verifies installed package completion includes `base`.

### Phase 9: Key Bindings and Editing Behavior

Port `key_bindings.py` after the prompt loop, history, and completion work.

Required v1 bindings:

1. `Enter` / `Ctrl-J`
   - If R input is parse-complete, submit.
   - Otherwise insert newline with indentation.
2. Auto-pairs when not inside a string and following text is compatible:
   - `()`
   - `[]`
   - `{}`
   - `""`
   - `''`
3. Raw R string pairs:
   - `r"(...)"`, `r"[...]"`, `r"{...}"`
   - support dash delimiters like `r"---(... )---"`
4. Closing delimiters skip over existing closing delimiter.
5. Backspace inside an empty pair deletes both sides.
6. Backspace in leading indentation deletes up to `tab_size` spaces.
7. Typing a closing bracket on a blank indented line dedents one level.
8. `Tab` in leading whitespace inserts spaces, not a literal tab.
9. Bracketed paste:
   - normalize CRLF/CR to LF.
   - if pasted text ends with newline, cursor is at end, and code is complete,
     strip trailing newline and submit.
   - otherwise insert literal paste content.
10. `;` at cursor beginning in R mode activates shell mode.
11. Backspace at cursor beginning in shell mode returns to R/browse mode.
12. `Ctrl-C` cancels completion before interrupting input.
13. `Ctrl-X Ctrl-E` opens the configured editor.

Editor selection:

1. Use R option `editor` if it is a string.
2. Else use `VISUAL`.
3. Else use `EDITOR`.
4. Else use `vi`.

Acceptance:

- Unit tests for auto-pair insertion and deletion.
- Unit tests for bracketed paste decision.
- Manual test for editor integration.
- Manual test for shell mode entry and exit.

### Phase 10: Lexer and String Detection

Port enough of `lexer.py` and `document.py`.

Do not implement a complete R parser. Implement a scanner that can identify:

- comments
- whitespace
- valid R names including backtick names
- numbers
- common operators
- punctuation
- single-quoted strings
- double-quoted strings
- R raw strings with `()`, `[]`, `{}` delimiters and dash variants

Use the scanner for:

- syntax highlighting
- `cursor_in_string`
- auto-pair guards
- package completion decision

Rule for `cursor_in_string`:

1. Tokenize text before cursor after trimming trailing whitespace.
2. Walk backward over trailing newline text tokens.
3. Return true if the last meaningful token is a string or lexer error.
4. Return false otherwise.

Acceptance:

- Unit tests for normal strings, escaped quotes, comments, and raw strings.
- Unit tests match Python behavior for representative samples.

### Phase 11: Shell Mode

Port `shell.py`.

Behavior:

1. Empty shell command prints a newline.
2. Parse command:
   - Windows: split once on first space.
   - Unix: shell-like split.
3. `cd`:
   - requires exactly one argument.
   - `cd -` swaps to `OLDPWD` or current directory if unset.
   - expand `~` and env vars.
   - update `OLDPWD`.
   - print new current directory.
4. Other commands:
   - Windows: run through shell.
   - Unix: run `$SHELL -c command`, defaulting to `/bin/sh`.
   - inherit stdin/stdout.
   - wait for process completion.

Acceptance:

- Unit tests for `cd`, `cd -`, bad `cd`, env expansion.
- Manual test runs `;pwd` or `;echo ok` from the R prompt.

### Phase 12: Packaging and Platform Support

Linux:

- Primary development target.
- Must pass embedded R integration tests.

macOS:

- Implement loader path and BLAS injection behavior behind `cfg(target_os =
  "macos")`.
- Test manually with Homebrew R and CRAN R if available.

Windows:

- Keep API boundaries ready for Windows.
- Implement UTF-8 behavior equivalent to `rutils.set_utf8`:
  - if R version is at least 4.2.0 and `LANG` is unset, set
    `LANG=en_US.UTF-8`.
  - set R option `encoding` to `UTF-8`.
- Console implementation may require a separate backend.

## 6. Suggested Milestones

### Milestone A: Minimal Embedded R

Deliver:

- CLI parses flags.
- R discovery works.
- Dynamic loader setup works.
- Embedded R starts.
- `1 + 1` can be evaluated interactively.
- stdout/stderr callbacks print to terminal.

Do not include completion, history, highlighting, or advanced keybindings yet.

### Milestone B: radian Prompt Basics

Deliver:

- Settings load from R options.
- Profiles source in correct order.
- R and browse prompt modes work.
- Multiline R input waits for parse completeness.
- Ctrl-C and EOF behave correctly.

### Milestone C: History and Shell

Deliver:

- Existing `.radian_history` files load.
- New history entries write in compatible format.
- Mode-aware navigation/search works.
- Shell mode works with `;`, `cd`, `cd -`, and subprocess execution.

### Milestone D: Completion and Editing Polish

Deliver:

- R completion.
- Package completion.
- LaTeX completion.
- Shell path completion.
- Auto-pairs, indentation, bracketed paste, editor integration.

### Milestone E: Cross-Platform Hardening

Deliver:

- Manual testing on Linux/macOS/Windows.

## 7. Testing Strategy

Use three test layers.

### Unit Tests

Run with:

```bash
cargo test
```

Cover:

- CLI flag parsing.
- `--vanilla` expansion.
- R discovery path handling.
- Environment variable composition.
- Settings defaults.
- Prompt mode activation.
- History parser/writer.
- History mode filtering.
- Lexer string detection.
- Completion regexes.
- Shell command parsing.
- Keybinding pure editing transforms.

### Integration Tests

Mark tests that require R with an environment gate:

```bash
RADIAN_RS_TEST_R=1 cargo test --test embedded_r
```

Cover:

- Embedded R initializes.
- R expression evaluation works.
- R options can be read and written.
- R profile files can be sourced.
- stdout and stderr callbacks fire.
- Parse-completeness checks match R behavior.
- Installed package completion sees base packages.

Current note: the embedded tests are present, but they only exercise real R
subprocess behavior when `RADIAN_RS_TEST_R=1` is set. Without that environment
variable, the tests return early and only prove the harness compiles.

### Manual Acceptance Tests

Run:

```bash
cargo run -- --version
cargo run --
```

Verify:

- Startup greeting appears unless `--quiet`.
- `1 + 1` prints `2`.
- Multiline input works:
  ```r
  if (TRUE) {
      1 + 1
  }
  ```
- Ctrl-C interrupts:
  ```r
  Sys.sleep(100)
  ```
- `browser()` enters browse prompt.
- History persists after restart.
- `;pwd` or `;echo ok` works.
- `cd` changes Rust process working directory.
- Completion works for `lib`, `base::`, and package names.
- Existing `~/.radian_history` is readable.

Additional current manual checks still needed:

- `browser()` prompt mode in an interactive terminal.
- Ctrl-C interrupting `Sys.sleep(100)` or another long-running R expression.
- macOS loader behavior with Homebrew R and CRAN R.
- Windows startup, shell behavior, console output, and UTF-8 `encoding`.

## 8. Known Risks and How to Handle Them

### R Embedding API Risk

Risk: R's embedding API is global and sensitive to initialization order.

Plan:

- Keep R calls on the main thread.
- Hide all unsafe APIs inside `r_runtime`.
- Add integration tests early.
- Do not combine terminal event-loop work and R calls across threads unless
  absolutely necessary.

### Terminal Editor Capability Risk

Risk: `reedline` may not expose every prompt-toolkit behavior needed by radian.

Plan:

- Implement pure editing operations separately from the terminal library.
- Keep the prompt session behind a local trait or facade.
- If needed, replace the backend without rewriting completion/history/settings.

### Dynamic Loader Risk

Risk: embedded R can fail before Rust code gets useful errors if loader paths
are wrong.

Plan:

- Port loader setup before R initialization.
- Re-exec after setting loader paths.
- Add verbose debug logging under `--debug`.

## 9. Definition of Done for the Port

The Rust port is complete enough to replace Python radian when:

1. `radian-rs --version` reports itself and the active R installation.
2. Interactive R evaluation works with correct stdout/stderr behavior.
3. R profiles and radian settings work through existing R option names.
4. R, browse, shell, and unknown prompt modes work.
5. Multiline input and parse-complete submit behavior match radian.
6. Existing radian history files are readable and new entries are compatible.
7. R/package/LaTeX/shell path completion work.
8. Core keybindings, auto-pairs, indentation, bracketed paste, and editor
   integration work.
9. Ctrl-C, EOF, nested input, and terminal resize behavior are handled.
10. Linux is tested automatically, and macOS/Windows behavior is either tested
    or clearly documented with platform-specific gaps.

Current blockers against this definition of done:

- Phase 8 completion parity is still partial (LaTeX table is tiny).
- Autosuggest, custom keybindings (`escape_key_map`/`ctrl_key_map`), and
  matching-bracket highlight are not wired.
- Platform hardening is Linux-first; macOS and Windows remain unaccepted.
- Reticulate prompt integration, on-load hooks, and askpass are not implemented.

## 10. First Task for the Implementer

Start with Milestone A only.

Concrete first steps:

1. Add dependencies and `build.rs`.
2. Implement `cli.rs`, `r_discovery.rs`, and `env_setup.rs`.
3. Implement the smallest possible `r_runtime` that initializes R and evaluates
   one expression.
4. Add one integration test gated by `RADIAN_RS_TEST_R=1`.
5. Only after that works, add console callbacks and the interactive loop.

Do not implement completion, history, or keybindings until the embedded R
runtime is proven.
