# P0 — Magic Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the core `%`-prefix magic command framework: registry, dispatch, REPL integration, and two built-in magics (`%lsmagic`, `%magic`).

**Architecture:** A `MagicRegistry` singleton (behind `OnceLock<Mutex<...>>`, same pattern as `ConsoleState`) maps command names to handler functions. The `read_console_interactive` and piped `read_console` paths check for `%` prefix after the existing `;` shell-command check. Magics return structured output (`Display`, `Eval`, `DisplayAndEval`, `Silent`) that the REPL loop routes to stdout or R evaluation. Automagic is a boolean setting (default off) parsed from the R option `radian.automagic`.

**Tech Stack:** Rust, existing radian-rs codebase (reedline, embedded R via ffi).

## Global Constraints

- All existing tests must continue to pass: `cargo test -- --test-threads=1` and `RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture --test-threads=1`.
- Follow existing codebase conventions: inline `#[cfg(test)] mod tests { }` in each module, `anyhow::Result`, `OnceLock<Mutex<...>>` for global state.
- New module declared in `src/lib.rs`, its own file under `src/`.
- R option name for automagic: `radian.automagic` (logical, default `FALSE`).
- The `%` prefix is detected only at **start of line after optional whitespace**. Mid-expression `%` (e.g. `a %>% b`, `a %% b`) is never treated as magic.

---

### Task 1: Add automagic setting to Settings and ConsoleSettings

**Files:**
- Modify: `src/settings.rs:17-47` — add `automagic: bool` field
- Modify: `src/r_runtime.rs:215-236` — add `automagic: bool` to `ConsoleSettings`
- Modify: `src/r_runtime.rs:238-264` — wire automagic in `ConsoleSettings::default()`
- Modify: `src/r_runtime.rs:301-327` — wire automagic in `install_console_settings()`

**Interfaces:**
- Consumes: None (first task).
- Produces: `Settings::automagic` (bool, default `false`), `ConsoleSettings::automagic` (bool, default `false`).

- [ ] **Step 1: Add `automagic` field to `Settings` struct**

```rust
// src/settings.rs, after line 46 (after ctrl_key_map)
pub automagic: bool,
```

- [ ] **Step 2: Set default to `false` in `Settings::default()`**

```rust
// src/settings.rs, after ctrl_key_map line
automagic: false,
```

- [ ] **Step 3: Load from R option in `Settings::load_from_r_options()`**

Add near the other radian option reads:
```rust
automagic: runtime.get_option_bool("radian.automagic", d.automagic)?,
```

- [ ] **Step 4: Add `automagic` to `ConsoleSettings` struct**

```rust
// src/r_runtime.rs, after ctrl_key_map field
pub automagic: bool,
```

- [ ] **Step 5: Wire in `ConsoleSettings::default()`**

```rust
// src/r_runtime.rs, after ctrl_key_map line
automagic: settings.automagic,
```

- [ ] **Step 6: Wire in `install_console_settings()`**

```rust
// src/r_runtime.rs, after ctrl_key_map line
automagic: settings.automagic,
```

- [ ] **Step 7: Run tests to verify no regressions**

```bash
cargo test -- --test-threads=1
```
Expected: all tests pass (no behavioral changes yet — just new fields with defaults).

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "p0: add automagic setting to Settings and ConsoleSettings"
```

---

### Task 2: Create magic module with registry and dispatch

**Files:**
- Create: `src/magic.rs`
- Modify: `src/lib.rs` — add `pub mod magic;`

**Interfaces:**
- Consumes: None (standalone module, depends on no other task).
- Produces:
  - `magic::MagicCommand { name: String, args: String, is_cell: bool }`
  - `magic::MagicOutput { Display(String), Eval(String), DisplayAndEval { display, code }, Silent }`
  - `magic::parse_magic(text: &str, automagic: bool) -> Option<MagicCommand>`
  - `magic::dispatch(cmd: &MagicCommand) -> anyhow::Result<MagicOutput>`
  - `magic::register_magic(name: &str, handler: fn(&str) -> anyhow::Result<MagicOutput>)`
  - `magic::is_magic_line(text: &str) -> bool`
  - `magic::lsmagic() -> String`
  - `magic::is_magic_name(name: &str) -> bool`

- [ ] **Step 1: Write failing tests for `parse_magic`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_percent_prefix_magic() {
        let cmd = parse_magic("%lsmagic", false).unwrap();
        assert_eq!(cmd.name, "lsmagic");
        assert!(cmd.args.is_empty());
        assert!(!cmd.is_cell);
    }

    #[test]
    fn parse_percent_magic_with_args() {
        let cmd = parse_magic("%who data.frame", false).unwrap();
        assert_eq!(cmd.name, "who");
        assert_eq!(cmd.args, "data.frame");
    }

    #[test]
    fn parse_non_magic_returns_none() {
        assert!(parse_magic("1 + 1", false).is_none());
        assert!(parse_magic("ls()", false).is_none());
        assert!(parse_magic("", false).is_none());
    }

    #[test]
    fn parse_magic_with_leading_whitespace() {
        let cmd = parse_magic("  %lsmagic", false).unwrap();
        assert_eq!(cmd.name, "lsmagic");
    }

    #[test]
    fn automagic_enables_prefixless_magic() {
        // When automagic is on and name is registered
        register_magic("who", |_| Ok(MagicOutput::Silent));
        let cmd = parse_magic("who data.frame", true).unwrap();
        assert_eq!(cmd.name, "who");
        assert_eq!(cmd.args, "data.frame");
    }

    #[test]
    fn automagic_does_not_consume_r_function_calls() {
        // ls() is an R call, not a magic — automagic should not match
        register_magic("ls", |_| Ok(MagicOutput::Silent));
        assert!(parse_magic("ls()", true).is_none());
        assert!(parse_magic("ls(myvar)", true).is_none());
    }

    #[test]
    fn parse_cell_magic() {
        let cmd = parse_magic("%%timeit", false).unwrap();
        assert_eq!(cmd.name, "timeit");
        assert!(cmd.is_cell);
    }
```

- [ ] **Step 2: Run test to verify failures**

```bash
cargo test magic -- --test-threads=1 2>&1 | head -30
```
Expected: all magic tests fail with "module not found" or "function not defined".

- [ ] **Step 3: Write the magic module implementation**

```rust
// src/magic.rs — full file content

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// A parsed magic command.
#[derive(Debug, Clone)]
pub struct MagicCommand {
    /// Magic name (without `%` prefix).
    pub name: String,
    /// Argument string (everything after the name, trimmed).
    pub args: String,
    /// Whether this is a cell magic (`%%` prefix).
    pub is_cell: bool,
}

/// The result of dispatching a magic command.
#[derive(Debug)]
pub enum MagicOutput {
    /// Display this text to the user (no R evaluation).
    Display(String),
    /// Queue this R code for evaluation.
    Eval(String),
    /// Display text AND queue R code.
    DisplayAndEval { display: String, code: String },
    /// No output, no evaluation.
    Silent,
}

type MagicHandler = fn(&str) -> anyhow::Result<MagicOutput>;

struct MagicRegistry {
    magics: HashMap<String, MagicHandler>,
}

static REGISTRY: OnceLock<Mutex<MagicRegistry>> = OnceLock::new();

fn registry() -> &'static Mutex<MagicRegistry> {
    REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        m.insert("lsmagic".to_string(), lsmagic_handler as MagicHandler);
        m.insert("magic".to_string(), magic_handler as MagicHandler);
        Mutex::new(MagicRegistry { magics: m })
    })
}

/// Register a new magic handler. Panics if the name is already registered.
pub fn register_magic(name: &str, handler: MagicHandler) {
    let mut reg = registry().lock().unwrap();
    reg.magics.insert(name.to_string(), handler);
}

/// Check if a magic name is registered.
pub fn is_magic_name(name: &str) -> bool {
    let reg = registry().lock().unwrap();
    reg.magics.contains_key(name)
}

/// Return a sorted list of registered magic names with descriptions.
pub fn lsmagic() -> String {
    let reg = registry().lock().unwrap();
    let mut names: Vec<&String> = reg.magics.keys().collect();
    names.sort();
    let mut out = String::from("Available magics:\n");
    for name in names {
        out.push_str(&format!("  %{name}\n"));
    }
    out
}

/// Try to parse a magic command from the input line.
///
/// Returns `None` if the line is not a magic command. When `automagic` is true,
/// lines starting with a registered magic name (not followed by `(`) are also
/// treated as magic commands.
pub fn parse_magic(text: &str, automagic: bool) -> Option<MagicCommand> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    // Check for `%` or `%%` prefix
    if let Some(rest) = trimmed.strip_prefix("%%") {
        let (name, args) = split_name_args(rest);
        return Some(MagicCommand {
            name: name.to_string(),
            args: args.to_string(),
            is_cell: true,
        });
    }
    if let Some(rest) = trimmed.strip_prefix('%') {
        let (name, args) = split_name_args(rest);
        return Some(MagicCommand {
            name: name.to_string(),
            args: args.to_string(),
            is_cell: false,
        });
    }

    // Automagic: no `%` prefix, but line starts with a registered magic name
    // and is not an R function call (i.e. not followed by `(`).
    if automagic {
        let (candidate, _rest) = split_name_args(trimmed);
        if !candidate.is_empty() {
            let reg = registry().lock().unwrap();
            if reg.magics.contains_key(candidate) {
                // Ensure the first non-whitespace token is directly followed
                // by space/newline/eof, not by `(` (which would be an R call).
                let after_name = &trimmed[candidate.len()..].trim_start();
                if !after_name.starts_with('(') {
                    let (name, args) = split_name_args(trimmed);
                    return Some(MagicCommand {
                        name: name.to_string(),
                        args: args.to_string(),
                        is_cell: false,
                    });
                }
            }
        }
    }

    None
}

/// Split "name arg1 arg2" into ("name", "arg1 arg2").
fn split_name_args(input: &str) -> (&str, &str) {
    let trimmed = input.trim_start();
    let end = trimmed.find(|c: char| c.is_whitespace()).unwrap_or(trimmed.len());
    let name = &trimmed[..end];
    let args = trimmed[end..].trim_start();
    (name, args)
}

/// Dispatch a magic command to its registered handler.
pub fn dispatch(cmd: &MagicCommand) -> anyhow::Result<MagicOutput> {
    let reg = registry().lock().unwrap();
    let handler = reg
        .magics
        .get(&cmd.name)
        .ok_or_else(|| anyhow::anyhow!("Unknown magic: {}", cmd.name))?;
    (handler)(&cmd.args)
}

// --- Built-in handlers ---

fn lsmagic_handler(_args: &str) -> anyhow::Result<MagicOutput> {
    Ok(MagicOutput::Display(lsmagic()))
}

fn magic_handler(_args: &str) -> anyhow::Result<MagicOutput> {
    Ok(MagicOutput::Display(
        "Magic commands use the % prefix.\n\
         Use %lsmagic to list available magics.\n\
         Set options(radian.automagic = TRUE) to use magics without the % prefix.\n"
            .to_string(),
    ))
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_percent_prefix_magic() {
        // lsmagic is registered by default
        let cmd = parse_magic("%lsmagic", false).unwrap();
        assert_eq!(cmd.name, "lsmagic");
        assert!(cmd.args.is_empty());
        assert!(!cmd.is_cell);
    }

    #[test]
    fn parse_percent_magic_with_args() {
        // who is not registered yet — parsing still works, dispatch will fail
        let cmd = parse_magic("%who data.frame", false).unwrap();
        assert_eq!(cmd.name, "who");
        assert_eq!(cmd.args, "data.frame");
    }

    #[test]
    fn parse_non_magic_returns_none() {
        assert!(parse_magic("1 + 1", false).is_none());
        assert!(parse_magic("ls()", false).is_none());
        assert!(parse_magic("", false).is_none());
    }

    #[test]
    fn parse_magic_with_leading_whitespace() {
        let cmd = parse_magic("  %lsmagic", false).unwrap();
        assert_eq!(cmd.name, "lsmagic");
    }

    #[test]
    fn automagic_enables_prefixless_magic() {
        // The name must be registered for automagic to work
        register_magic("who", |_| Ok(MagicOutput::Silent));
        let cmd = parse_magic("who data.frame", true).unwrap();
        assert_eq!(cmd.name, "who");
        assert_eq!(cmd.args, "data.frame");
    }

    #[test]
    fn automagic_does_not_consume_r_function_calls() {
        // If a name is registered but the input looks like an R call (has `(`), skip
        register_magic("ls", |_| Ok(MagicOutput::Silent));
        assert!(parse_magic("ls()", true).is_none());
        assert!(parse_magic("ls(myvar)", true).is_none());
    }

    #[test]
    fn parse_cell_magic() {
        let cmd = parse_magic("%%timeit", false).unwrap();
        assert_eq!(cmd.name, "timeit");
        assert!(cmd.is_cell);
    }

    #[test]
    fn lsmagic_lists_registered_magics() {
        let output = lsmagic();
        assert!(output.contains("lsmagic"));
        assert!(output.contains("magic"));
    }

    #[test]
    fn dispatch_known_magic_succeeds() {
        register_magic("test_dispatch", |_| {
            Ok(MagicOutput::Display("ok".to_string()))
        });
        let cmd = MagicCommand {
            name: "test_dispatch".to_string(),
            args: String::new(),
            is_cell: false,
        };
        let result = dispatch(&cmd).unwrap();
        match result {
            MagicOutput::Display(s) => assert_eq!(s, "ok"),
            _ => panic!("expected Display"),
        }
    }

    #[test]
    fn dispatch_unknown_magic_fails() {
        let cmd = MagicCommand {
            name: "nonexistent".to_string(),
            args: String::new(),
            is_cell: false,
        };
        assert!(dispatch(&cmd).is_err());
    }
}
```

- [ ] **Step 4: Add `pub mod magic;` to `src/lib.rs`**

```rust
pub mod magic;  // insert after `pub mod lexer;` (keep alphabetical order)
```

- [ ] **Step 5: Run tests to verify the magic module works**

```bash
cargo test magic -- --test-threads=1
```
Expected: all magic tests pass.

- [ ] **Step 6: Run full test suite to verify no regressions**

```bash
cargo test -- --test-threads=1
```
Expected: previous test count + new magic tests, all passing.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "p0: create magic module with registry, parse, dispatch, and built-in lsmagic/magic"
```

---

### Task 3: Hook magic dispatch into the interactive REPL loop

**Files:**
- Modify: `src/r_runtime.rs:877-952` — add magic dispatch in `read_console_interactive()`
- Modify: `src/r_runtime.rs:717-820` — add magic dispatch in piped `read_console()`

**Interfaces:**
- Consumes:
  - `magic::parse_magic(text, automagic) -> Option<MagicCommand>`
  - `magic::dispatch(&MagicCommand) -> Result<MagicOutput>`
  - `ConsoleSettings::automagic: bool`
- Produces: None (injects behavior into existing REPL paths).

- [ ] **Step 1: Write integration tests for magic dispatch in REPL**

These test the magic dispatch logic at the Rust level (not end-to-end REPL), verifying that magic commands are detected and routed correctly.

```rust
// In src/r_runtime.rs, inside the existing #[cfg(test)] mod tests

#[test]
fn magic_dispatch_replaces_input_with_r_code() {
    // Register a test magic that returns Eval
    crate::magic::register_magic("test_r", |_| {
        Ok(crate::magic::MagicOutput::Eval("1 + 1".to_string()))
    });
    let cmd = crate::magic::parse_magic("%test_r", false).unwrap();
    let output = crate::magic::dispatch(&cmd).unwrap();
    match output {
        crate::magic::MagicOutput::Eval(code) => assert_eq!(code, "1 + 1"),
        _ => panic!("expected Eval"),
    }
}

#[test]
fn magic_dispatch_displays_output() {
    crate::magic::register_magic("test_display", |_| {
        Ok(crate::magic::MagicOutput::Display("hello magic".to_string()))
    });
    let cmd = crate::magic::parse_magic("%test_display", false).unwrap();
    let output = crate::magic::dispatch(&cmd).unwrap();
    match output {
        crate::magic::MagicOutput::Display(s) => assert_eq!(s, "hello magic"),
        _ => panic!("expected Display"),
    }
}
```

- [ ] **Step 2: Add magic import to `r_runtime.rs`**

Add `magic,` to the `use crate::{...}` block at the top of `src/r_runtime.rs`.

- [ ] **Step 3: Add magic dispatch in `read_console_interactive()`**

After the shell command block (after line ~946, after the `if mode.accept_inline()` block closing brace) and before `store_prompt_session`, insert:

```rust
// Check for magic commands (% prefix)
if let Some(magic_cmd) = magic::parse_magic(&text, settings.automagic) {
    match magic::dispatch(&magic_cmd) {
        Ok(magic::MagicOutput::Display(msg)) => {
            print!("{msg}");
            io::stdout().flush().ok();
            store_prompt_session(session);
            continue;
        }
        Ok(magic::MagicOutput::Eval(code)) => {
            store_prompt_session(session);
            append_history(mode, &text);
            return queue_input(&code, mode, buf, len);
        }
        Ok(magic::MagicOutput::DisplayAndEval { display, code }) => {
            print!("{display}");
            io::stdout().flush().ok();
            store_prompt_session(session);
            append_history(mode, &text);
            return queue_input(&code, mode, buf, len);
        }
        Ok(magic::MagicOutput::Silent) => {
            store_prompt_session(session);
            append_history(mode, &text);
            continue;
        }
        Err(err) => {
            eprintln!("Magic error: {err}");
            store_prompt_session(session);
            continue;
        }
    }
}
```

- [ ] **Step 4: Add magic dispatch in piped `read_console()`**

In the piped input loop (around line 810, after the shell command check), insert similar dispatch:

```rust
if let Some(magic_cmd) = magic::parse_magic(&text, settings.automagic) {
    match magic::dispatch(&magic_cmd) {
        Ok(magic::MagicOutput::Eval(code)) => {
            append_history(&mode, &text);
            return queue_input(&code, &mode, buf, len);
        }
        Ok(magic::MagicOutput::Display(msg)) => {
            print!("{msg}");
            continue;
        }
        Ok(magic::MagicOutput::DisplayAndEval { display, code }) => {
            print!("{display}");
            append_history(&mode, &text);
            return queue_input(&code, &mode, buf, len);
        }
        Ok(magic::MagicOutput::Silent) => {
            continue;
        }
        Err(err) => {
            eprintln!("Magic error: {err}");
            continue;
        }
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -- --test-threads=1
```
Expected: all tests pass (new integration tests for magic dispatch + all existing tests).

- [ ] **Step 6: Manual smoke test**

Build and run the REPL with a quick manual check:

```bash
cargo build && echo '%lsmagic' | ./target/debug/radian-rs -q
```
Expected: the REPL starts, evaluates `%lsmagic`, which displays the list of available magics, then exits.

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "p0: hook magic dispatch into interactive and piped REPL loops"
```

---

### Task 4: Wire automagic through ConsoleSettings to REPL dispatch

**Files:**
- Modify: `src/r_runtime.rs:301-327` — already done in Task 1 step 6
- Modify: `src/r_runtime.rs:877-952` — already uses `settings.automagic` in Task 3 step 3

**Interfaces:**
- Consumes: `ConsoleSettings::automagic` field (from Task 1)
- Produces: Interactive REPL respects `radian.automagic` R option

- [ ] **Step 1: Verify the wiring is complete**

Automagic is already wired end-to-end:
1. Task 1: `Settings::automagic` → `load_from_r_options` reads `radian.automagic` → `ConsoleSettings::automagic` via `Default` and `install_console_settings`.
2. Task 3: `read_console_interactive()` and `read_console()` pass `settings.automagic` to `magic::parse_magic()`.

No additional code changes needed.

- [ ] **Step 2: Run full test suite**

```bash
cargo test -- --test-threads=1
RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture --test-threads=1
```
Expected: all tests pass.

- [ ] **Step 3: Update the developer log**

Append an entry to `/home/workstation/radian-rust-rewrite/docs/developer-log.md`:

```markdown
## 2026-07-01 — P0 Magic Framework Implemented

**Delivered:**
- `src/magic.rs` — MagicRegistry, `%`/`%%` parser, dispatch, automagic support
- `%lsmagic` — lists all registered magic commands
- `%magic` — prints magic system help
- `radian.automagic` R option (default `FALSE`) — when `TRUE`, registered magic names
  can be used without the `%` prefix (unless followed by `(`, which is treated as R call)
- Magic dispatch hooked into `read_console_interactive` and piped `read_console`
- 12 unit tests covering parse, dispatch, automagic, cell magics, and error handling

**Usage:**
- `%lsmagic` at the REPL prompt lists available magics
- `%magic` shows help text
- Set `options(radian.automagic = TRUE)` to enable prefixless magic detection

**Next phase:** P1 — shell integration (`!`, `%cd`, `%pwd`, `%env`, `%ls`) or
P2 — object browser + inspection (`%whos`, `?`/`??`, data inspection magics).
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "p0: complete — magic framework with automagic and REPL integration"
```

---

## Verification Checklist

Run after all tasks complete:

```bash
# Full unit test suite
cargo test -- --test-threads=1
# Expected: all unit tests pass (previous count + ~12 new magic tests)

# Embedded R tests
RADIAN_RS_TEST_R=1 cargo test --test embedded_r -- --nocapture --test-threads=1
# Expected: all 6 embedded R tests pass

# Manual smoke: piped magic
echo '%lsmagic' | ./target/debug/radian-rs -q
# Expected: magics list displayed

# Manual smoke: piped magic with R code output
echo '%magic' | ./target/debug/radian-rs -q
# Expected: magic help text displayed

# Manual smoke: R expression still works
echo '1 + 1' | ./target/debug/radian-rs -q
# Expected: [1] 2
```
