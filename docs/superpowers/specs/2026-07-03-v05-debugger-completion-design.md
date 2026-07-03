# v0.5 Debugger + Modal Help + Inspection Completion Design

**Date:** 2026-07-03
**Status:** Approved design — awaiting implementation plan
**Milestone:** v0.5 Debugger + Fuzzy Completion (target: 72 handlers)

---

## Objective

Complete the v0.5 milestone by adding:

1. **8 debugger magic handlers** — post-mortem debugging, debugger control, browser invocation
2. **`?` / `??` modal help** — line-start detection routes to `%pdoc` / `%psource`
3. **`%methods`** — S3/S4 dispatch introspection
4. **`%psearch`** — pattern-based object search

Current handler count: 66. Target after this work: 72.

---

## Files Affected

| File | Change |
|------|--------|
| `src/magics/debug.rs` | Add 8 handler structs + `MagicHandler` impls + unit tests |
| `src/magics/inspect.rs` | Add `Methods` and `Psearch` handler structs + impls + unit tests |
| `src/r_runtime.rs` | Add `?`/`??` early check in both `read_console_interactive` and piped `read_console` dispatch loops |
| `src/magic.rs` | Register all 11 new handlers in `register_all()` under P3 (Debugging) priority |

No new modules needed. No new dependencies.

---

## Section 1: 8 Debugger Handlers

All land in `src/magics/debug.rs`. Each is a unit struct implementing `MagicHandler`.
They follow the exact pattern established by the existing `Where`, `Continue`, and `Traceback` handlers.

### Handler Table

| Handler | R Code | Output Type | Validation |
|---------|--------|-------------|------------|
| `Debug` | `recover()` | `Output::Text` | None — no args, calls recover for post-mortem |
| `Pdb` | `options(error = recover)` / `options(error = NULL)` | `Output::Text` | Accepts `on`, `off`, or empty (show state) |
| `DebugOnce` | `debugonce(name)` | `Output::Silent` | Requires exactly one arg (function name) |
| `Undebug` | `undebug(name)` | `Output::Silent` | Requires exactly one arg (function name) |
| `Browser` | `browser()` | `Output::Eval` | None — injects browser() into R eval |
| `StepNext` | `n` | `Output::Eval` | None — debugger "step next" command |
| `StepFinish` | `finish` | `Output::Eval` | None — debugger "step out" command |
| `QuitDebug` | `Q` | `Output::Silent` | None — debugger "quit" command, silent output |

### Shared Helpers

`debug.rs` already defines:
- `eval_r_captured(code) -> Result<Output, MagicError>` — runs R, captures stdout
- `eval_r_silent(code) -> Result<(), MagicError>` — runs R, no output

All 8 handlers use these. No new FFI or R integration.

### R Code Details

- `%debug`: calls `recover()` after showing traceback — post-mortem entry
- `%pdb on`: `options(error = recover)` — sets error handler to enter debugger
- `%pdb off`: `options(error = NULL)` — restores default error handling
- `%pdb` (no args): reads `getOption("error")` and reports current state
- `%debugonce <fn>`: `debugonce(<fn>)` — sets debug flag for next invocation
- `%undebug <fn>`: `undebug(<fn>)` — removes debug flag
- `%browser`: `browser()` — enters browser at current point
- `%n`: evaluates `n` in the debugger environment (step to next line)
- `%finish`: evaluates `finish` in the debugger environment (step out of current function)
- `%Q`: evaluates `Q` in the debugger environment (exit debugger, return to top level)

---

## Section 2: `?` / `??` Modal Help

### Detection

In both `read_console_interactive` (line ~899) and the piped `read_console` loop (line ~755), add a check **after the `;` shell command check and before `%` magic dispatch**:

```rust
// ? modal help: route ?name → %pdoc, ??name → %psource
if let Some(query) = text.trim_start().strip_prefix('?') {
    let query = query.trim_end_matches('\n');
    if query.starts_with('?') {
        // ??name → show source code
        let source_query = query.strip_prefix('?').unwrap_or(query).trim();
        if source_query.is_empty() {
            print!("Show source code for an R function.\nUsage: ??function_name\n");
            continue;
        }
        match dispatch(&MagicLine {
            name: "psource".into(),
            args: source_query.to_string(),
            is_cell: false,
        }) {
            Ok(Output::Text(msg)) => print!("{msg}"),
            Err(e) => eprintln!("{e}"),
            _ => {}
        }
    } else {
        // ?name → show documentation
        let doc_query = query.trim();
        if doc_query.is_empty() {
            print!("Show documentation for an R function.\nUsage: ?function_name\n");
            continue;
        }
        match dispatch(&MagicLine {
            name: "pdoc".into(),
            args: doc_query.to_string(),
            is_cell: false,
        }) {
            Ok(Output::Text(msg)) => print!("{msg}"),
            Err(e) => eprintln!("{e}"),
            _ => {}
        }
    }
    io::stdout().flush().ok();
    continue;
}
```

### Rules

- Only matches when `?` or `??` is at the **start of the line** (after optional whitespace)
- `?name` → dispatches to `%pdoc name`
- `??name` → dispatches to `%psource name`
- Bare `?` or `??` → prints short usage hint
- After dispatch, `continue` to next read (does not submit to R)
- Handles errors gracefully — prints error message and continues

### Why not a magic handler

Approach A (early check) was chosen over Approach B (pseudo-magic handler) because:
- `?` is an R operator (`help()` abbreviation) — registering it as a magic would conflict with R evaluation paths
- The check is simpler and more transparent as a 15-line early-return in the dispatch loop
- No changes needed to `parse_magic()` or the magic registry

---

## Section 3: `%methods` and `%psearch`

Both land in `src/magics/inspect.rs` adjacent to the existing inspection handlers (Objects, Who, Whos, etc.).

### `%methods`

| Aspect | Detail |
|--------|--------|
| R code | `methods(name)` |
| Output | `Output::Text` |
| Validation | Requires exactly one argument |

No args → returns error message "Usage: %methods <function_or_class_name>"
Valid arg → returns captured output of `methods(name)` which shows:
- S3 methods: `[1] print.data.frame* print.Date ...
- S4 methods: `[1] show,AmpliconGraph-method ...
- Marked with `*` for non-visible methods

### `%psearch`

| Aspect | Detail |
|--------|--------|
| R code | `cat(find(what, numeric=TRUE), sep="\\n")` then `apropos(what, ignore.case=TRUE)` |
| Output | `Output::Text` |
| Validation | Requires exactly one argument |

No args → returns error message "Usage: %psearch <pattern>"
Valid arg → returns:
1. `find(what)` results — shows which packages contain objects matching `what`
2. `apropos(what, ignore.case = TRUE)` — shows all matching names

---

## Section 4: Registration in `magic.rs`

All 11 handlers register in `register_all()` under P3 (Debugging and timing):

```
// P3 — Debugging and timing
registry.register(Arc::new(crate::magics::debug::Traceback));
registry.register(Arc::new(crate::magics::debug::Where));
registry.register(Arc::new(crate::magics::debug::Continue));
registry.register(Arc::new(crate::magics::debug::Debug));          // new
registry.register(Arc::new(crate::magics::debug::Pdb));            // new
registry.register(Arc::new(crate::magics::debug::DebugOnce));      // new
registry.register(Arc::new(crate::magics::debug::Undebug));        // new
registry.register(Arc::new(crate::magics::debug::Browser));        // new
registry.register(Arc::new(crate::magics::debug::StepNext));       // new
registry.register(Arc::new(crate::magics::debug::StepFinish));     // new
registry.register(Arc::new(crate::magics::debug::QuitDebug));      // new
registry.register(Arc::new(crate::magics::timing::Time));
registry.register(Arc::new(crate::magics::timing::TimeIt));
registry.register(Arc::new(crate::magics::timing::Prun));
registry.register(Arc::new(crate::magics::inspect::Methods));       // new
registry.register(Arc::new(crate::magics::inspect::Psearch));       // new
```

---

## Section 5: Testing

### Unit Tests (no R required)

**In `debug.rs`:**
- Each handler exists in the registry (registration test)
- `%pdb` toggle: no args shows state, `on` sets error=recover, `off` sets error=NULL
- `%pdb` invalid arg: returns error
- `%debugonce` with no args: returns error
- `%undebug` with no args: returns error
- `%browser` returns `Output::Eval`
- `%n` returns `Output::Eval`
- `%finish` returns `Output::Eval`
- `%Q` returns `Output::Silent`

**In `inspect.rs`:**
- `%methods` with no args: returns error
- `%methods` with args: returns `Output::Text`
- `%psearch` with no args: returns error
- `%psearch` with args: returns `Output::Text`

**In `r_runtime.rs`:**
- `?name` at line start detected and routes correctly
- `??name` at line start detected and routes correctly
- Bare `?` shows usage text
- `?` not at line start (e.g., `x ? y`) is not caught
- `?` detection works with leading whitespace

### Integration Tests

Deferred — handler-level R FFI tests are a known project gap. The handlers call existing R functions through the established `eval_r_captured`/`eval_r_silent` helpers. No new R-gated tests are added.

---

## Section 6: Error Handling

- `%debugonce` and `%undebug` return `MagicError` if no argument is provided
- `%pdb` returns `MagicError` if an unrecognized argument is provided
- `%methods` and `%psearch` return `MagicError` if no argument is provided
- `?`/`??` dispatch wraps the `dispatch()` call — errors print to stderr and the loop continues
- R-level errors (unknown function, etc.) propagate through the existing `eval_r_captured` error path

---

## Verification

```bash
cargo check                    # 0 errors, 0 warnings
cargo clippy -- -D warnings    # 0 warnings
cargo test --lib               # ~410+ passed, 0 failed
```

Handler count after: 72 (66 + 8 debug + 1 methods + 1 psearch - ? is not a handler)
