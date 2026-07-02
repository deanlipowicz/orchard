# EDA Magic Handlers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 8 EDA magic commands (%summary, %glimpse, %describe, %missing, %corr, %freq, %compare, %sessioninfo)

**Architecture:** Single new file `src/magics/eda.rs` with 8 `MagicHandler` impls, each wrapping R functions via existing `eval_r_captured()` / `eval_with_pkg_check()` helpers. Registration in `src/magic.rs`, module export in `src/magics/mod.rs`.

**Tech Stack:** Rust, R FFI via existing `r_runtime::eval_string_raw_global()`

## Global Constraints

- Follow existing handler pattern (see `inspect.rs`, `timing.rs`)
- Use `eval_r_captured()` for base-R handlers, `eval_with_pkg_check()` for optional-package handlers
- All handlers return `Output::Text`
- `#![deny(unsafe_op_in_unsafe_fn)]` enforced
- Handler names are lowercase, single-word, matching the R function name
- No `unwrap()` in production code without safety-rationale comment

---

### Task 1: Create the module file with 8 handlers

**Files:**
- Create: `src/magics/eda.rs`
- Modify: `src/magics/mod.rs` (add `pub mod eda;`)
- Modify: `src/magic.rs` (add registrations)

- [ ] **Step 1: Create `src/magics/eda.rs` with all 8 handlers**

Write the file with the following structure:

```rust
use crate::magic::{self, MagicHandler, MagicLine, Output};

fn eval_r_captured(code: &str) -> Result<Output, magic::MagicError> {
    let wrapped = format!("capture.output({code})");
    let text = crate::r_runtime::eval_string_raw_global(&wrapped).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    Ok(Output::Text(text))
}

fn eval_with_pkg_check(code: &str, pkg: &str) -> Result<Output, magic::MagicError> {
    let check = format!(
        "if (!requireNamespace('{pkg}', quietly=TRUE)) stop('package {pkg} is not installed')"
    );
    crate::r_runtime::eval_string_raw_global(&check).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    eval_r_captured(code)
}
```

Then 8 handler structs, each with `name() -> &'static str`, `description() -> &'static str`, and `run(&self, line: &MagicLine) -> Result<Output, MagicError>`:

1. **Summary** — `summary(<args>)`, no pkg check
2. **Glimpse** — `dplyr::glimpse(<args>)`, pkg: `dplyr`
3. **Describe** — `skimr::skim(<args>)`, pkg: `skimr`
4. **Missing** — `naniar::miss_summary(<args>)`, pkg: `naniar`, wrap in `capture.output(print(...))`
5. **Corr** — `cor(<args>, use = 'pairwise.complete.obs')`, no pkg check
6. **Freq** — `janitor::tabyl(<args>)`, pkg: `janitor`
7. **Compare** — `waldo::compare(<args>, max_diffs = 20)`, pkg: `waldo`, wrap in `capture.output(print(...))`
8. **SessionInfo** — `sessioninfo::session_info()`, pkg: `sessioninfo`

Run: `cargo check` — verify 0 errors, 0 warnings

- [ ] **Step 2: Add `pub mod eda;` to `src/magics/mod.rs`**

Add after `pub mod debug;`:
```rust
pub mod eda;
```

Run: `cargo check` — verify 0 errors

- [ ] **Step 3: Register handlers in `src/magic.rs`**

In `register_all()`, after the P8 section (file_magics), add:
```rust
// P9 — EDA handlers
registry.register(Arc::new(crate::magics::eda::Summary));
registry.register(Arc::new(crate::magics::eda::Glimpse));
registry.register(Arc::new(crate::magics::eda::Describe));
registry.register(Arc::new(crate::magics::eda::Missing));
registry.register(Arc::new(crate::magics::eda::Corr));
registry.register(Arc::new(crate::magics::eda::Freq));
registry.register(Arc::new(crate::magics::eda::Compare));
registry.register(Arc::new(crate::magics::eda::SessionInfo));
```

Run: `cargo check` — verify 0 errors, 0 warnings

- [ ] **Step 4: Run full lib tests**

```bash
cargo test --lib
```
Expected: all tests pass

- [ ] **Step 5: Run clippy**

```bash
cargo clippy -- -D warnings
```
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add src/magics/eda.rs src/magics/mod.rs src/magic.rs
git commit -m "feat: add 8 EDA magic handlers (%summary, %glimpse, %describe, %missing, %corr, %freq, %compare, %sessioninfo)

Part of v0.3 EDA Core milestone."
```

### Task 2: Add parse+dispatch tests for each handler

**Files:**
- Modify: `src/magics/eda.rs` (add `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write tests for each handler**

Add tests that verify:
- Each handler is registered in the magic registry
- Parsing `%<name> <args>` returns the correct `MagicLine`
- Dispatch returns `Ok(Output::Text(_))`

- [ ] **Step 2: Run tests**

```bash
cargo test --lib eda
```
Expected: 8+ tests pass

- [ ] **Step 3: Run full test suite**

```bash
cargo test --lib
cargo clippy -- -D warnings
```
Expected: all pass, 0 warnings

- [ ] **Step 4: Commit**

```bash
git add src/magics/eda.rs
git commit -m "test: add parse+dispatch tests for 8 EDA magic handlers"
```

### Task 3: Update development plan counts

**Files:**
- Modify: `docs/development-plan.md`

- [ ] **Step 1: Update handler count from 47 to 55**

Update:
- Line 7: `47 registered magic handlers` → `55 registered magic handlers`
- Line 7: `363 tests` → `370+ tests`
- Registration table: Add EDA row matching table format
- v0.3 target: Mark EDA handlers as complete

- [ ] **Step 2: Commit**

```bash
git add docs/development-plan.md
git commit -m "docs: update handler count and v0.3 EDA milestone status"
```
