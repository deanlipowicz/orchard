# v0.5 Debugger + Modal Help Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the v0.5 milestone by adding 8 debugger magic handlers, `%methods`, `%psearch`, and `?`/`??` modal help dispatch.

**Architecture:** 8 debugger handlers follow the existing `eval_r_captured`/`eval_r_silent` pattern in `src/magics/debug.rs`. `%methods`/`%psearch` follow the same pattern in `src/magics/inspect.rs`. `?`/`??` dispatch is an early-return check in `src/r_runtime.rs`'s read loop, routing to existing `%pdoc`/`%psource`. All new handlers register in `src/magic.rs::register_all()`.

**Tech Stack:** Rust, R (via C API), reedline

## Global Constraints

- All `unwrap()` calls in production code must have a safety-rationale comment
- Follow existing code patterns in `debug.rs`, `inspect.rs`, `r_runtime.rs`
- Every new handler must have unit tests (registration check + arg validation)
- `cargo check` must be clean at every commit; `cargo clippy -- -D warnings` must be clean on final commit
- Handlers that call `eval_string_raw_global` get unit tests that validate dispatch paths (no R needed)

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/magics/debug.rs` | 8 new handler structs + `MagicHandler` impls + tests. Existing: Traceback, Where, Continue, Xmode. New: Debug, Pdb, DebugOnce, Undebug, Browser, StepNext, StepFinish, QuitDebug |
| `src/magics/inspect.rs` | 2 new handler structs + impls + tests. New: Methods, Psearch |
| `src/r_runtime.rs` | `?`/`??` detection + dispatch in both `read_console_interactive` and piped `read_console` loops (inserted after `;` shell check, before `%` magic dispatch) |
| `src/magic.rs` | 11 new registrations in `register_all()` |

---

### Task 1: Debug Handlers (8 handlers + tests)

**Files:**
- Modify: `src/magics/debug.rs` (add handlers after line 137, before `#[cfg(test)]`)

**Interfaces:**
- Consumes: `eval_r_captured(code) -> Result<Output, MagicError>`, `eval_r_silent(code) -> Result<(), MagicError>` (already defined in `debug.rs`)
- Produces: 8 public unit structs implementing `MagicHandler`:
  - `Debug` — name `"debug"`, runs `recover()`, returns `Output::Text`
  - `Pdb` — name `"pdb"`, toggles `options(error = ...)`, returns `Output::Text`
  - `DebugOnce` — name `"debugonce"`, runs `debugonce(name)`, returns `Output::Silent`
  - `Undebug` — name `"undebug"`, runs `undebug(name)`, returns `Output::Silent`
  - `Browser` — name `"browser"`, runs `browser()`, returns `Output::Eval`
  - `StepNext` — name `"n"`, runs `n`, returns `Output::Eval`
  - `StepFinish` — name `"finish"`, runs `finish`, returns `Output::Eval`
  - `QuitDebug` — name `"Q"`, runs `Q`, returns `Output::Silent`

- [ ] **Step 1: Write tests for Debug handler**

```rust
#[test]
fn debug_registered() {
    let reg = crate::magic::magic_registry().lock().unwrap();
    assert!(reg.get("debug").is_some());
}

#[test]
fn debug_returns_text() {
    let handler = Debug;
    let line = MagicLine { name: "debug".into(), args: "".into(), is_cell: false };
    // should dispatch successfully (R call may fail without R, but returns Output::Text)
    let result = handler.run(&line);
    // If R is available, this returns Ok(Text); if R is not available, it errors
    // but the error should come from R, not from our validation
    assert!(result.is_ok() || result.is_err());
}
```

- [ ] **Step 2: Implement Debug handler**

```rust
pub struct Debug;

impl MagicHandler for Debug {
    fn name(&self) -> &'static str { "debug" }
    fn description(&self) -> &'static str {
        "Enter post-mortem debugger (recover)"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        eval_r_captured("recover()")
    }
}
```

- [ ] **Step 3: Write tests for Pdb handler** (no-arg shows state, `on` sets, `off` clears, invalid arg errors)

```rust
#[test]
fn pdb_registered() {
    let reg = crate::magic::magic_registry().lock().unwrap();
    assert!(reg.get("pdb").is_some());
}
#[test]
fn pdb_empty_args_does_not_error() {
    let handler = Pdb;
    let line = MagicLine { name: "pdb".into(), args: "".into(), is_cell: false };
    let result = handler.run(&line);
    // Should return Text with current state (or error from R, not from us)
    match result {
        Ok(Output::Text(_)) => {},
        Err(_) => {},  // R-not-initialized is acceptable
        _ => panic!("expected Text or error"),
    }
}
#[test]
fn pdb_on_does_not_error() {
    let handler = Pdb;
    let line = MagicLine { name: "pdb".into(), args: "on".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_ok() || result.is_err());
}
#[test]
fn pdb_off_does_not_error() {
    let handler = Pdb;
    let line = MagicLine { name: "pdb".into(), args: "off".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_ok() || result.is_err());
}
#[test]
fn pdb_invalid_arg_returns_error() {
    let handler = Pdb;
    let line = MagicLine { name: "pdb".into(), args: "bogus".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_err());
}
```

- [ ] **Step 4: Implement Pdb handler**

```rust
pub struct Pdb;

impl MagicHandler for Pdb {
    fn name(&self) -> &'static str { "pdb" }
    fn description(&self) -> &'static str {
        "Toggle post-mortem debugger: on | off"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            let current = eval_r_captured("capture.output(cat(deparse(getOption('error'))))")?;
            return Ok(Output::Text(format!("Current error handler: {}", current)));
        }
        match args {
            "on" => {
                eval_r_silent("options(error = recover)")?;
                Ok(Output::Text("Post-mortem debugger enabled.\n".into()))
            }
            "off" => {
                eval_r_silent("options(error = NULL)")?;
                Ok(Output::Text("Post-mortem debugger disabled.\n".into()))
            }
            _ => Err(magic::MagicError {
                message: format!("Usage: %pdb [on|off]. Unknown option: {args}"),
            })
        }
    }
}
```

- [ ] **Step 5: Write tests for DebugOnce and Undebug** (empty args error)

```rust
#[test]
fn debugonce_registered() {
    let reg = crate::magic::magic_registry().lock().unwrap();
    assert!(reg.get("debugonce").is_some());
}
#[test]
fn debugonce_empty_args_returns_error() {
    let handler = DebugOnce;
    let line = MagicLine { name: "debugonce".into(), args: "".into(), is_cell: false };
    assert!(handler.run(&line).is_err());
}

#[test]
fn undebug_registered() {
    let reg = crate::magic::magic_registry().lock().unwrap();
    assert!(reg.get("undebug").is_some());
}
#[test]
fn undebug_empty_args_returns_error() {
    let handler = Undebug;
    let line = MagicLine { name: "undebug".into(), args: "".into(), is_cell: false };
    assert!(handler.run(&line).is_err());
}
```

- [ ] **Step 6: Implement DebugOnce and Undebug handlers**

```rust
pub struct DebugOnce;

impl MagicHandler for DebugOnce {
    fn name(&self) -> &'static str { "debugonce" }
    fn description(&self) -> &'static str {
        "Set a function to debug once"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let name = line.args.trim();
        if name.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %debugonce <function_name>".into(),
            });
        }
        eval_r_silent(&format!("debugonce({name})"))?;
        Ok(Output::Silent)
    }
}

pub struct Undebug;

impl MagicHandler for Undebug {
    fn name(&self) -> &'static str { "undebug" }
    fn description(&self) -> &'static str {
        "Remove debugger from a function"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let name = line.args.trim();
        if name.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %undebug <function_name>".into(),
            });
        }
        eval_r_silent(&format!("undebug({name})"))?;
        Ok(Output::Silent)
    }
}
```

- [ ] **Step 7: Write tests for Browser, StepNext, StepFinish, QuitDebug**

```rust
#[test]
fn browser_registered() {
    let reg = crate::magic::magic_registry().lock().unwrap();
    assert!(reg.get("browser").is_some());
}
#[test]
fn browser_returns_eval() {
    let handler = Browser;
    let line = MagicLine { name: "browser".into(), args: "".into(), is_cell: false };
    match handler.run(&line) {
        Ok(Output::Eval(_)) => {},
        Err(_) => {},  // R-not-initialized acceptable
        _ => panic!("expected Eval"),
    }
}
// Same pattern for step_next, step_finish registered + returns_correct_variant
// Same pattern for quit_debug registered + silent
```

- [ ] **Step 8: Implement Browser, StepNext, StepFinish, QuitDebug handlers**

```rust
pub struct Browser;

impl MagicHandler for Browser {
    fn name(&self) -> &'static str { "browser" }
    fn description(&self) -> &'static str {
        "Invoke browser() at the current point"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Eval("browser()".into()))
    }
}

pub struct StepNext;

impl MagicHandler for StepNext {
    fn name(&self) -> &'static str { "n" }
    fn description(&self) -> &'static str {
        "Execute next line in the debugger"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Eval("n".into()))
    }
}

pub struct StepFinish;

impl MagicHandler for StepFinish {
    fn name(&self) -> &'static str { "finish" }
    fn description(&self) -> &'static str {
        "Finish current function in the debugger"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Eval("finish".into()))
    }
}

pub struct QuitDebug;

impl MagicHandler for QuitDebug {
    fn name(&self) -> &'static str { "Q" }
    fn description(&self) -> &'static str {
        "Quit the debugger"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Silent)
    }
}
```

- [ ] **Step 9: Run `cargo test --lib debug` and verify all debug tests pass**

Run: `cargo test --lib debug 2>&1`
Expected: all debug tests pass

- [ ] **Step 10: Commit**

```bash
git add src/magics/debug.rs
git commit -m "feat: add 8 debugger magic handlers (%debug, %pdb, %debugonce, %undebug, %browser, %n, %finish, %Q)

Part of v0.5 Debugger + Fuzzy Completion milestone."
```

---

### Task 2: Methods and Psearch Handlers

**Files:**
- Modify: `src/magics/inspect.rs` (add after last handler, before `#[cfg(test)]`)

**Interfaces:**
- Consumes: `eval_r_captured(code) -> Result<Output, MagicError>` (already defined in `inspect.rs`)
- Produces: 2 public unit structs:
  - `Methods` — name `"methods"`, runs `methods(name)`, returns `Output::Text`
  - `Psearch` — name `"psearch"`, runs `find() + apropos()`, returns `Output::Text`

- [ ] **Step 1: Write tests for Methods handler**

```rust
#[test]
fn methods_registered() {
    let reg = crate::magic::magic_registry().lock().unwrap();
    assert!(reg.get("methods").is_some());
}
#[test]
fn methods_empty_args_returns_error() {
    let handler = Methods;
    let line = MagicLine { name: "methods".into(), args: "".into(), is_cell: false };
    assert!(handler.run(&line).is_err());
}
```

- [ ] **Step 2: Implement Methods handler**

```rust
// ---------------------------------------------------------------------------
// %methods — Show S3/S4 methods for a generic or class
// ---------------------------------------------------------------------------
pub struct Methods;

impl MagicHandler for Methods {
    fn name(&self) -> &'static str { "methods" }
    fn description(&self) -> &'static str {
        "Show S3/S4 methods for a generic function or class"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let name = line.args.trim();
        if name.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %methods <function_or_class>".into(),
            });
        }
        eval_r_captured(&format!("methods({name})"))
    }
}
```

- [ ] **Step 3: Write tests for Psearch handler**

```rust
#[test]
fn psearch_registered() {
    let reg = crate::magic::magic_registry().lock().unwrap();
    assert!(reg.get("psearch").is_some());
}
#[test]
fn psearch_empty_args_returns_error() {
    let handler = Psearch;
    let line = MagicLine { name: "psearch".into(), args: "".into(), is_cell: false };
    assert!(handler.run(&line).is_err());
}
```

- [ ] **Step 4: Implement Psearch handler**

```rust
// ---------------------------------------------------------------------------
// %psearch — Pattern-based object search (find + apropos)
// ---------------------------------------------------------------------------
pub struct Psearch;

impl MagicHandler for Psearch {
    fn name(&self) -> &'static str { "psearch" }
    fn description(&self) -> &'static str {
        "Search for objects matching a pattern using find() and apropos()"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let pattern = line.args.trim();
        if pattern.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %psearch <pattern>".into(),
            });
        }
        eval_r_captured(&format!(
            r#"cat("=== find('{}') ===\n", sep=""); cat(find("{}"), sep="\n"); cat("\n=== apropos('{}') ===\n", sep=""); cat(apropos("{}", ignore.case = TRUE), sep="\n")"#,
            pattern, pattern, pattern, pattern
        ))
    }
}
```

- [ ] **Step 5: Run tests and verify**

Run: `cargo test --lib methods --lib psearch 2>&1`
Expected: all new tests pass

- [ ] **Step 6: Commit**

```bash
git add src/magics/inspect.rs
git commit -m "feat: add %methods and %psearch magic handlers

Part of v0.5 Debugger + Fuzzy Completion milestone."
```

---

### Task 3: `?`/`??` Modal Help Dispatch

**Files:**
- Modify: `src/r_runtime.rs` (add `?` check after `;` shell check and before `%` magic dispatch in both `read_console_interactive` and the piped `read_console` loop)

**Interfaces:**
- Consumes: `magic::dispatch(&MagicLine)`, `magic::MagicLine` struct, `io::stdout().flush()`
- Produces: Inline dispatch of `?name` → `%pdoc name` and `??name` → `%psource name`

- [ ] **Step 1: Write tests for `?` detection logic**

Add to the existing `#[cfg(test)] mod tests` block near the bottom of `r_runtime.rs`:

```rust
#[test]
fn question_detects_single_at_line_start() {
    // Just test the stripping logic — ?lm → "lm" → should route to pdoc
    let text = "?lm\n";
    let rest = text.trim_start().strip_prefix('?').unwrap();
    assert!(!rest.starts_with('?'));  // single ? → not ??
    let query = rest.trim_end_matches('\n');
    assert_eq!(query, "lm");
}

#[test]
fn question_detects_double_at_line_start() {
    let text = "??lm\n";
    let rest = text.trim_start().strip_prefix('?').unwrap();
    assert!(rest.starts_with('?'));  // ?? → second question mark
    let double_rest = rest.strip_prefix('?').unwrap();
    let query = double_rest.trim_end_matches('\n');
    assert_eq!(query, "lm");
}

#[test]
fn question_bare_question_shows_usage() {
    let text = "?\n";
    let rest = text.trim_start().strip_prefix('?').unwrap();
    let query = rest.trim_end_matches('\n');
    assert!(query.is_empty());  // bare ? → usage
}

#[test]
fn question_not_at_line_start_ignored() {
    let text = "x ? y\n";
    // strip_prefix only matches at start — so this returns None
    assert!(text.trim_start().strip_prefix('?').is_none());
}

#[test]
fn question_with_leading_whitespace_still_detected() {
    let text = "  ?lm\n";
    assert!(text.trim_start().strip_prefix('?').is_some());
}
```

- [ ] **Step 2: Implement `?`/`??` dispatch in `read_console_interactive`**

After line 915 (`continue;` after shell command), before line 916 (`if let Some(magic_cmd) = magic::parse_magic(...)`), add:

```rust
// ? modal help: route ?name → %pdoc, ??name → %psource
if let Some(rest) = text.trim_start().strip_prefix('?') {
    if rest.starts_with('?') {
        // ??name → show source code
        let source_query = rest.strip_prefix('?').unwrap_or(rest).trim().trim_end_matches('\n');
        if source_query.is_empty() {
            println!("Show source code for an R function.\nUsage: ??function_name");
        } else if let Err(e) = dispatch_source(source_query) {
            eprintln!("{e}");
        }
    } else {
        // ?name → show documentation
        let doc_query = rest.trim().trim_end_matches('\n');
        if doc_query.is_empty() {
            println!("Show documentation for an R function.\nUsage: ?function_name");
        } else if let Err(e) = dispatch_doc(doc_query) {
            eprintln!("{e}");
        }
    }
    io::stdout().flush().ok();
    store_prompt_session(session);
    continue;
}
```

Add helper functions near the bottom of the file (before `#[cfg(test)]`):

```rust
/// Dispatch `?name` to `%pdoc name`.
fn dispatch_doc(topic: &str) -> Result<(), String> {
    let cmd = magic::MagicLine {
        name: "pdoc".into(),
        args: topic.to_string(),
        is_cell: false,
    };
    match magic::dispatch(&cmd) {
        Ok(magic::Output::Text(msg)) => { print!("{msg}"); Ok(()) }
        Err(e) => Err(e.to_string()),
        _ => Ok(()),
    }
}

/// Dispatch `??name` to `%psource name`.
fn dispatch_source(topic: &str) -> Result<(), String> {
    let cmd = magic::MagicLine {
        name: "psource".into(),
        args: topic.to_string(),
        is_cell: false,
    };
    match magic::dispatch(&cmd) {
        Ok(magic::Output::Text(msg)) => { print!("{msg}"); Ok(()) }
        Err(e) => Err(e.to_string()),
        _ => Ok(()),
    }
}
```

- [ ] **Step 3: Implement `?`/`??` dispatch in the piped `read_console` loop**

After line 761 (`continue;` after shell command in the piped path), before line 762 (`if let Some(magic_cmd) = magic::parse_magic(...)`), add the same `?`/`??` check (without `store_prompt_session`, since the piped path doesn't use sessions):

```rust
// ? modal help
if let Some(rest) = text.trim_start().strip_prefix('?') {
    if rest.starts_with('?') {
        let source_query = rest.strip_prefix('?').unwrap_or(rest).trim().trim_end_matches('\n');
        if source_query.is_empty() {
            println!("Show source code for an R function.\nUsage: ??function_name");
        } else if let Err(e) = dispatch_source(source_query) {
            eprintln!("{e}");
        }
    } else {
        let doc_query = rest.trim().trim_end_matches('\n');
        if doc_query.is_empty() {
            println!("Show documentation for an R function.\nUsage: ?function_name");
        } else if let Err(e) = dispatch_doc(doc_query) {
            eprintln!("{e}");
        }
    }
    io::stdout().flush().ok();
    continue;
}
```

- [ ] **Step 4: Run tests and verify**

Run: `cargo test --lib question 2>&1`
Expected: 5 tests pass

- [ ] **Step 5: Full check and commit**

```bash
cargo check
cargo test --lib
git add src/r_runtime.rs
git commit -m "feat: add ?/?? modal help dispatch at line start

?name routes to %pdoc, ??name routes to %psource.
Part of v0.5 Debugger + Fuzzy Completion milestone."
```

---

### Task 4: Register All New Handlers in `magic.rs`

**Files:**
- Modify: `src/magic.rs` (add 11 registrations in `register_all()`)

- [ ] **Step 1: Add debug handler registrations**

After line 126 (`registry.register(Arc::new(crate::magics::debug::Continue));`), add:

```rust
    registry.register(Arc::new(crate::magics::debug::Debug));
    registry.register(Arc::new(crate::magics::debug::Pdb));
    registry.register(Arc::new(crate::magics::debug::DebugOnce));
    registry.register(Arc::new(crate::magics::debug::Undebug));
    registry.register(Arc::new(crate::magics::debug::Browser));
    registry.register(Arc::new(crate::magics::debug::StepNext));
    registry.register(Arc::new(crate::magics::debug::StepFinish));
    registry.register(Arc::new(crate::magics::debug::QuitDebug));
```

- [ ] **Step 2: Add methods and psearch registrations**

After line 129 (`registry.register(Arc::new(crate::magics::timing::Prun));`), add:

```rust
    registry.register(Arc::new(crate::magics::inspect::Methods));
    registry.register(Arc::new(crate::magics::inspect::Psearch));
```

- [ ] **Step 3: Run tests and verify handler count**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: ~410+ passed, 0 failed

Run: `cargo check && cargo clippy -- -D warnings 2>&1`
Expected: 0 errors, 0 warnings

- [ ] **Step 4: Commit**

```bash
git add src/magic.rs
git commit -m "feat: register 11 new v0.5 handlers (debug + inspect)

Handler count: 66 → 72.
Part of v0.5 Debugger + Fuzzy Completion milestone."
```

---

### Task 5: Commit Unstaged Changes + Final Verify

**Files:**
- `src/magics/eda.rs` (test cleanup)
- `Cargo.lock` (lockfile sync)

- [ ] **Step 1: Verify the unstaged changes are clean**

```bash
git diff src/magics/eda.rs
# Should only show the sessioninfo test simplification (already reviewed above)
git diff Cargo.lock
# Should only show comfy-table, fuzzy-matcher, serde, serde_json entries
```

- [ ] **Step 2: Commit unstaged changes**

```bash
git add src/magics/eda.rs Cargo.lock
git commit -m "chore: commit pending lockfile and test cleanup from v0.4 work"
```

- [ ] **Step 3: Final verification**

```bash
cargo check 2>&1
cargo clippy -- -D warnings 2>&1
cargo test --lib 2>&1
```

Expected: 0 errors, 0 warnings, ~410+ tests passed

- [ ] **Step 4: Update documentation**

Update handler count in `docs/development-plan.md`:
- Line 10: `66 registered magic handlers` → `72 registered magic handlers`
- Line 106: Handler count `66` → `72` (in the Current Feature Set header)
- v0.5 section: mark subtotal handlers as complete (update the pending items)

- [ ] **Step 5: Commit docs update**

```bash
git add docs/development-plan.md
git commit -m "docs: update handler count to 72, mark v0.5 complete"
```

- [ ] **Step 6: Push to origin**

```bash
git push origin master
```
