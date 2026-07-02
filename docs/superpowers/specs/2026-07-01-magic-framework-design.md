# Magic Command Framework — P0 Design

Date: 2026-07-01
Status: Design (pre-implementation)
Project: radian-rs (Rust REPL)
Author: Agent (design decisions by user)

## 1. Objective

Build the foundation for IPython-style magic commands (`%` / `%%` prefix) and
R.nvim compensation features in the Rust REPL. P0 delivers the magic dispatch
system, not the individual feature magics (those are P1–P7).

## 2. Architecture

### 2.1 New Modules

```
src/
  magic.rs           — MagicRegistry, MagicHandler trait, Output/Error types
  magic_parser.rs    — Line prefix detection, argument extraction, cell accumulation
  magics/
    mod.rs           — register_all() entry point
    lsmagic.rs       — %lsmagic handler
    magic_help.rs    — %magic [name] handler
    automagic.rs     — %automagic [on|off] handler + conflict detection
```

### 2.2 Dispatch Flow

```
run_repl loop:
  reedline.read_line() → line

  ┌─ automagic check (if enabled):
  │   first_word = extract first whitespace-delimited token
  │   if is_registered(first_word) AND not an R function (Rf_findFun):
  │     → treat as %first_word (synthesize % prefix)
  │
  ├─ parse_magic_line(line) detects %/%% prefix after optional whitespace
  │
  ├── no magic prefix → proceed to R_ReplDLLdo1 (existing path, unchanged)
  │
  ├── %% prefix → cell magic:
  │      name, args = parse first line
  │      body = accumulate_cell_body(reedline)  // sub-loop until blank line
  │      dispatch(name, MagicLine { args, is_cell: true, body })
  │
  └── % prefix → line magic:
        name, args = parse line
        dispatch(name, MagicLine { args, is_cell: false, body: None })

  dispatch calls MagicRegistry::lookup(name) and handler.run(&MagicLine)
    Ok(Output::Text(s)) → write_console_ex(s, STDERR_CONSOLE)
    Ok(Output::Silent)  → no output
    Err(MagicError)     → write_console_ex(msg, STDERR_CONSOLE)

  After dispatch → continue (skip R_ReplDLLdo1)
```

Dispatch is **read-only** after startup. The `MagicRegistry` is populated once
in `main()` or `app::run()` and stored in a `LazyLock<MagicRegistry>`. No
mutex needed on the hot path.

### 2.3 Relationship to Existing Code

- Magic dispatch lives in `run_repl` in `src/r_runtime.rs` (the REPL loop
  remains in that module for P0; extraction to `src/repl.rs` can happen later
  if `run_repl` grows large enough to warrant it). It does NOT modify the
  `read_console` callback or any R FFI code.
- Output goes through the existing `write_console_ex` static, which is
  registered as R's `ptr_R_WriteConsoleEx`. This means magic output respects
  stderr formatting and suppression flags.
- Magics that need to evaluate R code call `r_runtime::eval_code()` through
  the crate's public API.

## 3. Parser Design

### 3.1 `parse_magic_line(line: &str) -> Option<ParsedMagic>`

```rust
pub struct ParsedMagic {
    pub name: String,
    pub args: String,       // everything after name, trimmed
    pub is_cell: bool,
}
```

Rules:
1. Strip leading whitespace
2. If first char is `%` → parse
3. If first two chars are `%%` → `is_cell = true`, name starts at position 2
4. If single `%` → `is_cell = false`, name starts at position 1
5. Name extends to first whitespace or end of line
6. Args is remainder after name, trimmed
7. If name is empty → return None (bare `%` or `%%` is not a magic)
8. Return None if no `%` prefix

### 3.2 `accumulate_cell_body(reedline, prompt) -> Result<Option<String>>`

Accumulates body lines for `%%` magics:
1. Set prompt to continuation prompt (e.g., `"... "` in R mode)
2. Loop:
   a. `reedline.read_line()` → line
   b. If line is empty (blank line) → break, return accumulated body
   c. If EOF → break, return accumulated body (partial cell)
   d. If Ctrl-C → break, return None (abort the magic entirely)
   e. Append line + `\n` to body accumulator
3. Do NOT push intermediate lines to reedline history
4. After loop, restore original prompt

## 4. Handler API

### 4.1 Core Types

```rust
/// Immutable input to a magic handler
pub struct MagicLine {
    pub args: String,
    pub is_cell: bool,
    pub body: Option<String>,
}

/// What the magic handler produces
pub enum Output {
    Text(String),
    Silent,
}

pub struct MagicError {
    pub message: String,
}

pub trait MagicHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn run(&self, line: &MagicLine) -> Result<Output, MagicError>;
}
```

### 4.2 Registry

```rust
pub struct MagicRegistry {
    handlers: HashMap<&'static str, Box<dyn MagicHandler>>,
}

impl MagicRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, handler: Box<dyn MagicHandler>);
    pub fn dispatch(&self, name: &str, line: &MagicLine) -> Result<Output, MagicError>;
    pub fn list_all(&self) -> Vec<(&'static str, &'static str)>;
    pub fn is_registered(&self, name: &str) -> bool;
}
```

### 4.3 Thread Safety

- `MagicHandler: Send + Sync` — handlers must be safe to call from any thread.
  R's `read_console` callback runs on R's thread, but `run_repl` runs on the
  main thread. Using `Send + Sync` keeps the option open without committing
  to a threading model.
- `MagicRegistry` is populated once at startup, then read-only. Shared via
  `LazyLock<MagicRegistry>`. No synchronization needed on dispatch.
- Automagic state (`automagic_enabled`) is an `AtomicBool` stored in
  `PromptContext` or a new `MagicState` alongside it.

## 5. Built-in Magics (P0)

### 5.1 `%lsmagic`

Lists all registered magics with descriptions:

```
Available magics:
  %lsmagic    — List available magics
  %magic      — Print help about the magic system
  %automagic  — Toggle automagic on/off
```

Handler: `lsmagic::Lsmagic` struct implementing `MagicHandler`.

### 5.2 `%magic [name]`

Without argument: print a brief introduction to the magic system:

```
Radian magic commands use the % prefix.
Type %lsmagic to see available magics.
Type %magic <name> for help on a specific magic.
```

With argument: print the matching handler's name + description, or "Unknown
magic: X" if not found.

Handler: `magic_help::MagicHelp` struct implementing `MagicHandler`.

### 5.3 `%automagic [on|off]`

Without argument: print current automagic state (`Automagic: on` / `Automagic: off`).

With argument `on` or `1` / `true`: enable automagic.
With argument `off` or `0` / `false`: disable automagic.
Invalid argument: print usage (`Usage: %automagic [on|off]`).

Handler: `automagic::Automagic` struct implementing `MagicHandler`.

### 5.4 `register_all()`

```rust
pub fn register_all(registry: &mut MagicRegistry) {
    registry.register(Box::new(lsmagic::Lsmagic));
    registry.register(Box::new(magic_help::MagicHelp));
    registry.register(Box::new(automagic::Automagic));
}
```

Called from `main()` or `app::init()` during startup.

## 6. Automagic

### 6.1 Semantics

When automagic is enabled, a command name that appears at the start of a line
(no `%` prefix) is treated as a magic command IF it does not conflict with an
R function name in the current environment.

### 6.2 Conflict Detection

At dispatch time (not at startup):

1. Extract the first whitespace-delimited token from the line
2. If `is_registered(token)` is false → not a magic, pass through to R
3. Call `Rf_findFun(token, R_GlobalEnv)` via FFI to check if R has a
   function with that name
4. If `Rf_findFun` returns `R_NilValue` or raises a symbol-not-found error
   → treat as magic (dispatch without `%` prefix)
5. If `Rf_findFun` returns a function → not a magic, pass through to R

Performance: `Rf_findFun` is called once per automagic candidate line.
Caching is not needed for P0 — the R call is fast (symbol table lookup).

### 6.3 State

Automagic state is stored as an `AtomicBool`. The setting could be persisted
via R options (`options(radian.automagic = TRUE)`) in a later phase, but for
P0 it is session-only.

## 7. Cell Magic Accumulation

### 7.1 User Experience

```
%%plot                  ← user types this
reedline prompt → ...   ← continuation prompt
  x <- 1:10             ← body line 1 (accumulated, not in history)
  y <- x^2              ← body line 2
  plot(x, y)            ← body line 3
                          ← blank line terminates
[magic output]
```

### 7.2 Prompt During Accumulation

The continuation prompt is `"... "` followed by the mode's short name:
- R mode: `"... "`
- Browse mode: `"... Browse> "`
- Shell mode: `"... $ "`

The mode is already tracked in `PromptContext`. The continuation prompt
string is `"... "` if the current mode is R, or `"... {mode_name}> "`
otherwise (e.g., `"... Browse> "`). This matches the existing convention
where the prompt reflects the current mode.

### 7.3 History

- Intermediate cell lines are NOT pushed to reedline history
- Only the full accumulated cell is recorded:
  - One entry in reedline's in-memory history (the 1st line with `%%`)
  - Not written to radian's history file (magic commands are not persisted
    to the radian history file format, matching existing behavior where
    shell commands are also excluded)

## 8. Error Handling

| Scenario | Behavior |
|----------|----------|
| Unknown magic name | Print `Unknown magic: %<name>. Type %lsmagic to see available magics.` to stderr. The line is NOT passed to R. |
| Handler returns `Err(MagicError)` | Print the error message to stderr via write_console. |
| Cell accumulation interrupted by Ctrl-C | Abort the cell magic entirely. No output, no partial dispatch. |
| Cell accumulation hits EOF | Dispatch with whatever body was accumulated (partial cell). |
| `Rf_findFun` itself errors during automagic check | Treat as "no conflict" — assume automagic applies. Rare edge case (R heap corruption). |

## 9. Testing Strategy

### 9.1 Unit Tests

- `magic_parser.rs`:
  - `%lsmagic` → name="lsmagic", args="", is_cell=false
  - `%who data.frame` → name="who", args="data.frame", is_cell=false
  - `%%plot` → name="plot", args="", is_cell=true
  - `  %who` → leading whitespace stripped
  - `x <- 1` → None (no magic prefix)
  - `%` → None (bare percent, no name)
  - `%%` → None (bare double percent, no name)

- `magic.rs`:
  - `MagicRegistry::dispatch` with unknown name → MagicError
  - `MagicRegistry::dispatch` with known name → handler called
  - `MagicRegistry::is_registered` works for registered/unregistered names

- `magics/automagic.rs`:
  - `%automagic` without args prints current state
  - `%automagic on` enables automagic
  - `%automagic off` disables automagic
  - `%automagic invalid` prints usage

- `magics/lsmagic.rs`:
  - `%lsmagic` output contains all registered handler names/descriptions

### 9.2 Integration Tests

- A new `tests/magic_framework.rs` integration test that:
  1. Registers a test-only magic (`%ping` → returns `"pong"`)
  2. Calls `dispatch` with `%ping` and verifies `Output::Text("pong")`
  3. Calls `dispatch` with `%nonexistent` and verifies `MagicError`
  4. Calls `dispatch` with a non-magic line (no `%` prefix) and verifies
     the line is returned as-is (no dispatch)
  5. If embedded R is available (`RADIAN_RS_TEST_R=1`), tests that automagic
     dispatches a name that is not an R function and passes through a name
     that is an R function

### 9.3 What P0 Does NOT Test

- Real R evaluation inside magic handlers (that's P2+)
- Cell accumulation via actual reedline calls (tested in unit tests with
  a mock or in integration with a real reedline instance)
- Automagic conflict detection against real R functions (requires embedded R)

## 10. Future-Proofing

The handler API is designed so that P2+ magics can:

- Accept optional flags (e.g., `%who data.frame` — argument parsing is
  the handler's responsibility, not the framework's)
- Evaluate R code via `r_runtime::eval_code` (available as a public crate API)
- Return rich output (String formatting is up to the handler)
- Register additional magics at runtime (the registry needs a `&mut` ref,
  but for v1 all registration happens at startup)

Cell magic support at the handler level is minimal — the handler receives
`body: Option<String>` and decides how to interpret it. The framework does
not enforce any particular cell semantics.

## 11. Non-Goals (P0)

- Argument parsing helpers (flags, quoted strings) — each handler parses
  its own args
- Persistence of automagic setting across sessions
- Extension system (`%load_ext`) — that's P7
- Per-magic help text beyond name + description — `%magic <name>` just
  prints the handler's stored description
- Output paging or formatting helpers
- R evaluation inside the framework itself — handlers call R if they need to
- Tab completion of magic names inside the REPL — deferred to a later phase
