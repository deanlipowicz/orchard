# Shell Utilities + File Execution Handlers — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 8 new magic handlers (cd, ls, sx, pushd, popd, dhist, run, load) to bring the project from 38 → 46 registered handlers.

**Architecture:** Add 6 shell handlers to existing `src/magics/shell.rs` (which already has `ShellState`, `expand_tilde()`, `SHELL_STATE` global). Create `src/magics/file_magics.rs` for `%run`/`%load`. All handlers use only `std` + `r_runtime::eval_string_raw_global` — no new dependencies. The `ShellState::dir_stack` and `dir_history` scaffolding fields get activated (remove `#[allow(dead_code)]`).

**Tech Stack:** Rust, std lib only (no new crates), embedded R via `r_runtime::eval_string_raw_global`.

## Global Constraints

- Every change must compile with `cargo check` and pass `cargo test --lib` before moving to the next task.
- All handlers use `r_runtime::eval_string_raw_global()` for R evaluation — no subprocess R spawning.
- `%sx` returns output as R character vector named `sx_output` (uses `eval_string_raw_global("sx_output <- c(...)")`).
- Crate is named `orchard`. Binary uses `CARGO_BIN_EXE_orchard`. Test gate is `ORCHARD_TEST_R`.
- Add handlers to existing files where possible — only one new file (`file_magics.rs`).
- Handler structs must implement the `MagicHandler` trait (`name()`, `description()`, `run(&self, line: &MagicLine)`).
- `%cd` and `%pushd`/`%popd` update `OLDPWD` env var via `crate::shell::env_lock()` for thread safety.
- Test names follow pattern: `test_<handler>_<scenario>` in `#[cfg(test)]` blocks within the handler file.

## File Map

| File | Status | Responsibility |
|------|--------|---------------|
| `src/magics/shell.rs` | **Modify** | `Cd`, `Ls`, `Sx`, `Pushd`, `Popd`, `Dhist` handler structs + impls. Path resolution helpers. Activate `dir_stack`/`dir_history`. |
| `src/magics/file_magics.rs` | **Create** | `Run`, `Load` handler structs + impls. |
| `src/magics/mod.rs` | **Modify** | Add `pub mod file_magics;`. |
| `src/magic.rs` | **Modify** | Register all 8 new handlers in `register_all()`. |

---

### Task 1: Create `file_magics.rs` with stub handlers + module registration

**Files:**
- Create: `src/magics/file_magics.rs`
- Modify: `src/magics/mod.rs`

**Interfaces:**
- Consumes: `crate::magic::{MagicHandler, MagicLine, Output}`, `crate::r_runtime::eval_string_raw_global`
- Produces: `Run` and `Load` pub structs implementing `MagicHandler` (stubs that return `Output::Text("not implemented yet")` for now — filled in Task 7)

- [ ] **Step 1: Create `src/magics/file_magics.rs`**

Write the new file with:
```rust
use crate::magic::{self, MagicHandler, MagicLine, Output};

pub struct Run;
impl MagicHandler for Run {
    fn name(&self) -> &'static str { "run" }
    fn description(&self) -> &'static str { "Run an R script from a file" }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Text("not implemented yet".into()))
    }
}

pub struct Load;
impl MagicHandler for Load {
    fn name(&self) -> &'static str { "load" }
    fn description(&self) -> &'static str { "Load file contents into the REPL" }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Text("not implemented yet".into()))
    }
}
```

- [ ] **Step 2: Add `pub mod file_magics;` to `src/magics/mod.rs`**

- [ ] **Step 3: Run `cargo check` to verify compilation**

Expected: 0 errors.

- [ ] **Step 4: Commit**

Commit message: `feat: scaffold file_magics.rs with Run/Load stub handlers`

---

### Task 2: Implement `%cd` handler

**Files:**
- Modify: `src/magics/shell.rs`

**Interfaces:**
- Consumes: `ShellState` (for `dir_history`), `expand_tilde()` helper, `crate::shell::env_lock()`, `std::env::set_current_dir`, `std::env::var("OLDPWD")`
- Produces: `Cd` pub struct implementing `MagicHandler`. `Cd::run()` handles no-args (→~), `-` (→OLDPWD), and `<path>` (→cd+tilde expansion).

- [ ] **Step 1: Write the failing test**

Add to `src/magics/shell.rs` `#[cfg(test)]` block:
```rust
#[test]
fn test_shell_cd_nonexistent() {
    let handler = super::Cd;
    let line = MagicLine { name: "cd".into(), args: "/tmp/orchard-nonexistent-dir-××××".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_err(), "expected error for nonexistent path");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib ::test_shell_cd_nonexistent -- --exact`
Expected: FAIL — `Cd` struct not defined yet.

- [ ] **Step 3: Implement `Cd` handler**

Add to `src/magics/shell.rs`:
```rust
pub struct Cd;
impl MagicHandler for Cd {
    fn name(&self) -> &'static str { "cd" }
    fn description(&self) -> &'static str { "Change directory (supports -, ~, OLDPWD)" }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        // (full implementation per spec)
    }
}
```

Implementation logic:
1. Parse args: empty/`~` → home from `home_dir()`, `-` → read `OLDPWD` env var, else → `expand_tilde()` + resolve
2. Save current dir to `OLDPWD` via `env_lock()` + `set_var`
3. `set_current_dir(target)` — error if fails
4. Push previous dir to `ShellState::dir_history`
5. Return `Output::Text` with resolved path

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib ::test_shell_cd_nonexistent -- --exact`
Expected: PASS

- [ ] **Step 5: Add cd - test**

```rust
#[test]
fn test_shell_cd_minus() {
    let handler = super::Cd;
    // Set OLDPWD, verify cd - works
    let orig = std::env::current_dir().unwrap();
    std::env::set_var("OLDPWD", "/tmp");
    let line = MagicLine { name: "cd".into(), args: "-".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_ok(), "cd - should succeed: {:?}", result);
    // clean up
    std::env::set_current_dir(&orig).ok();
}
```

- [ ] **Step 6: Run all shell tests**

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 7: Commit**

Commit message: `feat: implement %cd handler with OLDPWD support and dir_history tracking`

---

### Task 3: Implement `%ls` handler

**Files:**
- Modify: `src/magics/shell.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_shell_ls_empty_dir() {
    let handler = super::Ls;
    let tmp = std::env::temp_dir().join(format!("orchard-ls-test-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).ok();
    let line = MagicLine { name: "ls".into(), args: tmp.to_str().unwrap().into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_ok(), "ls should succeed: {:?}", result);
    // Clean up
    std::fs::remove_dir(&tmp).ok();
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL — `Ls` not defined.

- [ ] **Step 3: Implement `Ls` handler**

```rust
pub struct Ls;
impl MagicHandler for Ls {
    fn name(&self) -> &'static str { "ls" }
    fn description(&self) -> &'static str { "List directory contents" }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let path = if line.args.is_empty() {
            std::env::current_dir().map_err(|e| /*...*/)?
        } else {
            PathBuf::from(expand_tilde(line.args.trim()))
        };
        // read_dir, sort, format per spec
    }
}
```

Implementation:
1. Resolve path (empty → cwd, else `expand_tilde()`)
2. `std::fs::read_dir(path)` — error if path missing or not a dir
3. Collect entries, extract file names, sort alphabetically
4. Format as `name\n` per entry + `(N entries)\n`
5. Return `Output::Text`

- [ ] **Step 4: Run test to verify it passes**

Expected: PASS

- [ ] **Step 5: Run all tests**

- [ ] **Step 6: Commit**

Commit message: `feat: implement %ls handler with read_dir and alphabetical sort`

---

### Task 4: Implement `%sx` handler

**Files:**
- Modify: `src/magics/shell.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_shell_sx_echo() {
    let handler = super::Sx;
    let line = MagicLine { name: "sx".into(), args: "echo hello orchard".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_ok(), "sx should succeed: {:?}", result);
    if let Ok(Output::Text(text)) = result {
        assert!(text.contains("sx_output"), "output should mention variable: {text}");
        assert!(text.contains("hello orchard"), "output should contain command output: {text}");
    }
}
```

- [ ] **Step 2: Verify it fails**

Expected: FAIL — `Sx` struct not defined.

- [ ] **Step 3: Implement `Sx` handler**

```rust
pub struct Sx;
impl MagicHandler for Sx {
    fn name(&self) -> &'static str { "sx" }
    fn description(&self) -> &'static str { "Execute shell command and capture output as R character vector" }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        // Full implementation per spec
    }
}
```

Implementation:
1. Determine shell: `std::env::var("SHELL").unwrap_or("/bin/sh".into())`
2. `Command::new(shell).args(["-c", args]).stdout(Stdio::piped()).stderr(Stdio::piped()).output()`
3. If command fails (non-zero exit), return `MagicError` with stderr
4. Split stdout by `\n`, filter trailing empty strings
5. Escape each line for R string safety (`\` → `\\`, `"` → `\"`), wrap in `"`
6. Build R expression: `sx_output <- c("line1", "line2", ...)`
7. `eval_string_raw_global(&r_expr)` — error if R eval fails
8. Return `Output::Text` with summary: `"character vector 'sx_output' assigned: [1] \"line1\" \"line2\" ..."`

- [ ] **Step 4: Run test to verify it passes**

Expected: PASS

- [ ] **Step 5: Run all tests**

- [ ] **Step 6: Commit**

Commit message: `feat: implement %sx handler with shell capture and R character vector assignment`

---

### Task 5: Implement `%pushd` and `%popd` handlers

**Files:**
- Modify: `src/magics/shell.rs`

**Note:** This task activates the `ShellState::dir_stack` field. Remove `#[allow(dead_code)]` from `ShellState`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_shell_pushd_popd() {
    let handler_push = super::Pushd;
    let handler_pop = super::Popd;
    let orig = std::env::current_dir().unwrap();
    let tmp = std::env::temp_dir();

    // Push to temp dir
    let line = MagicLine { name: "pushd".into(), args: tmp.to_str().unwrap().into(), is_cell: false };
    let result = handler_push.run(&line);
    assert!(result.is_ok(), "pushd should succeed: {:?}", result);

    // Verify cwd changed
    assert_eq!(std::env::current_dir().unwrap(), tmp);

    // Pop back
    let line = MagicLine { name: "popd".into(), args: "".into(), is_cell: false };
    let result = handler_pop.run(&line);
    assert!(result.is_ok(), "popd should succeed: {:?}", result);

    // Verify cwd restored
    assert_eq!(std::env::current_dir().unwrap(), orig);
}

#[test]
fn test_shell_popd_empty_stack() {
    // Popd on empty stack should fail
}
```

- [ ] **Step 2: Verify they fail**

Expected: FAIL — `Pushd`/`Popd` not defined.

- [ ] **Step 3: Implement `Pushd` and `Popd` handlers**

```rust
pub struct Pushd;
impl MagicHandler for Pushd {
    fn name(&self) -> &'static str { "pushd" }
    // ...
}

pub struct Popd;
impl MagicHandler for Popd {
    fn name(&self) -> &'static str { "popd" }
    // ...
}
```

Pushd behavior:
1. If args empty, read `ShellState.dir_stack` and print it (no-op)
2. Push current dir onto `dir_stack`
3. Cd to target path
4. Print stack state (`format!("{:?}", stack)`)

Popd behavior:
1. Lock `ShellState`
2. Pop from `dir_stack` — error if empty
3. `set_current_dir(popped)` — update `OLDPWD`
4. Print remaining stack

Also: Remove `#[allow(dead_code)]` from `ShellState` now that `dir_stack` is used.

- [ ] **Step 4: Run tests to verify they pass**

Expected: PASS

- [ ] **Step 5: Run all tests**

- [ ] **Step 6: Commit**

Commit message: `feat: implement %pushd and %popd handlers with dir_stack activation`

---

### Task 6: Implement `%dhist` handler

**Files:**
- Modify: `src/magics/shell.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn test_shell_dhist() {
    let handler = super::Dhist;
    let line = MagicLine { name: "dhist".into(), args: "".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_ok(), "dhist should succeed: {:?}", result);
    // May be empty if no cd was called — that's fine, should still succeed.
}
```

- [ ] **Step 2: Verify it fails**

Expected: FAIL — `Dhist` not defined.

- [ ] **Step 3: Implement `Dhist` handler**

```rust
pub struct Dhist;
impl MagicHandler for Dhist {
    fn name(&self) -> &'static str { "dhist" }
    fn description(&self) -> &'static str { "Print directory history" }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let state = shell_state().lock().unwrap();
        if state.dir_history.is_empty() {
            return Ok(Output::Text("(no directory history)\n".into()));
        }
        let mut out = String::new();
        for (i, entry) in state.dir_history.iter().enumerate() {
            out.push_str(&format!("{:>3}: {}\n", i + 1, entry.display()));
        }
        Ok(Output::Text(out))
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Expected: PASS

- [ ] **Step 5: Run all tests**

- [ ] **Step 6: Commit**

Commit message: `feat: implement %dhist handler with dir_history display`

---

### Task 7: Implement `%run` and `%load` handlers

**Files:**
- Modify: `src/magics/file_magics.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn test_file_run_nonexistent() {
    let handler = super::Run;
    let line = MagicLine { name: "run".into(), args: "/tmp/orchard-nonexistent-run-file-××××.R".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_err(), "expected error for nonexistent file");
}

#[test]
fn test_file_load_nonexistent() {
    let handler = super::Load;
    let line = MagicLine { name: "load".into(), args: "/tmp/orchard-nonexistent-load-file-××××.R".into(), is_cell: false };
    let result = handler.run(&line);
    assert!(result.is_err(), "expected error for nonexistent file");
}
```

- [ ] **Step 2: Verify they fail**

Run tests — expected FAIL (stubs return `Ok(Output::Text("not implemented yet"))`, not error).

- [ ] **Step 3: Implement `Run` handler**

Replace stub with:
```rust
fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
    let path = line.args.trim();
    if path.is_empty() {
        return Err(magic::MagicError { message: "Usage: %run <filepath>".into() });
    }
    let resolved = if path.starts_with('~') {
        crate::magics::shell::expand_tilde(path)
    } else {
        path.to_string()
    };
    if !std::path::Path::new(&resolved).exists() {
        return Err(magic::MagicError { message: format!("File not found: {path}") });
    }
    let code = format!("source({:?})", resolved);
    crate::r_runtime::eval_string_raw_global(&code)
        .map_err(|e| magic::MagicError { message: e.to_string() })?;
    Ok(Output::Text(format!("Sourced {path}\n")))
}
```

- [ ] **Step 4: Implement `Load` handler**

Replace stub with:
```rust
fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
    let path = line.args.trim();
    if path.is_empty() {
        return Err(magic::MagicError { message: "Usage: %load <filepath>".into() });
    }
    let resolved = if path.starts_with('~') {
        crate::magics::shell::expand_tilde(path)
    } else {
        path.to_string()
    };
    let contents = std::fs::read_to_string(&resolved)
        .map_err(|e| magic::MagicError { message: format!("Cannot read {path}: {e}") })?;
    Ok(Output::Text(contents))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Expected: PASS (both tests should now get errors from non-existent files)

- [ ] **Step 6: Run all tests**

- [ ] **Step 7: Commit**

Commit message: `feat: implement %run and %load handlers with file validation`

---

### Task 8: Register all 8 handlers in `register_all()`

**Files:**
- Modify: `src/magic.rs`

- [ ] **Step 1: Add registrations**

In `register_all()` in `src/magic.rs`, add:

```rust
// P1 — Shell magics (extended)
registry.register(Arc::new(crate::magics::shell::Cd));
registry.register(Arc::new(crate::magics::shell::Ls));
registry.register(Arc::new(crate::magics::shell::Sx));
registry.register(Arc::new(crate::magics::shell::Pushd));
registry.register(Arc::new(crate::magics::shell::Popd));
registry.register(Arc::new(crate::magics::shell::Dhist));

// ... in the existing P6 section (workspace) or create a new P7 section:
// P7 — File execution
registry.register(Arc::new(crate::magics::file_magics::Run));
registry.register(Arc::new(crate::magics::file_magics::Load));
```

Place the shell ones after the existing `Env` and `Bookmark` lines. Place the file ones after the edit_magic block.

- [ ] **Step 2: Run `cargo check`**

Expected: 0 errors.

- [ ] **Step 3: Run all tests**

Run: `cargo test --lib`
Expected: 154 + 9 new = 163 passed, 0 failed.

Run: `cargo test --test magic_framework`
Expected: 6 passed, 0 failed.

- [ ] **Step 4: Update `docs/developer-log.md`**

Add an entry noting the new handler count (38→46) and which handlers were added.

- [ ] **Step 5: Commit**

Commit message: `feat: register all 8 new handlers — Cd, Ls, Sx, Pushd, Popd, Dhist, Run, Load`

Handler count: 38 → 46
