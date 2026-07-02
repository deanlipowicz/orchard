# Shell Utilities + File Execution Magic Handlers

**Date:** 2026-07-02
**Context:** Phase 1 of uplift from 38 → 72+ handlers. Prioritizes the most visible daily-use gap: shell utilities and file execution. See `docs/developer-log.md` § 2026-07-02 — Documentation vs Code Audit for the full gap analysis.

---

## Scope

Add 8 new magic handlers across two natural groupings:

| Group | Handlers | Count |
|-------|----------|-------|
| Shell utilities | `%cd`, `%ls`, `%sx`, `%pushd`, `%popd`, `%dhist` | 6 |
| File execution | `%run`, `%load` | 2 |

**Running total after this phase:** 38 → 46 registered handlers.

---

## Architecture

### Existing infrastructure used

- `src/magics/shell.rs` — `ShellState` with `dir_stack: Vec<PathBuf>` and `dir_history: Vec<PathBuf>` (currently `#[allow(dead_code)]` scaffolding)
- `src/magics/shell.rs` — `expand_tilde()` helper for `~` expansion
- `src/magics/shell.rs` — `SHELL_STATE` global via `OnceLock<Mutex<ShellState>>`
- `src/magic.rs` — `register_all()` registration point
- `src/shell.rs` — `env_lock()` for safe env var mutation (used by `%cd` for `OLDPWD`)

### New file

- `src/magics/file_magics.rs` — Contains `Run` and `Load` handler structs

### No new dependencies

All 8 handlers use only:
- `std::process::Command` (shell execution, `%sx`)
- `std::fs::read_dir` / `std::fs::read_to_string` (filesystem listing, file loading)
- `std::env::set_current_dir` / `std::env::var` (directory changes, `OLDPWD`)
- `r_runtime::eval_string_raw_global` (R evaluation for `%sx`, `%run`)
- Existing `expand_tilde()` helper

---

## Handler Specifications

### `%cd` — Change Directory

**File:** `src/magics/shell.rs`

**Arguments:**
- (empty) or `~` → cd to home directory
- `-` → swap to `OLDPWD` (like shell semantics)
- `<path>` → cd to path (tilde expansion, relative, or absolute)

**Behavior:**
1. Resolve the target path using `expand_tilde()` and standard path resolution
2. Save current directory to `OLDPWD` env var (via `env_lock()` + `set_var`)
3. Call `std::env::set_current_dir(target)`
4. Push previous directory to `ShellState::dir_history`
5. Print the new current directory (like `cd` in interactive shells)

**Error cases:**
- Path does not exist → `MagicError`
- `OLDPWD` unset when using `cd -` → message "(no previous directory)"

**Returns:** `Output::Text` with the resolved path.

---

### `%ls` — List Directory

**File:** `src/magics/shell.rs`

**Arguments:**
- (empty) → list current directory
- `<path>` → list specified directory

**Behavior:**
1. Resolve path with `expand_tilde()` if it starts with `~`
2. Call `std::fs::read_dir(path)`
3. Sort entries alphabetically
4. Print each entry name, one per line
5. Show entry count at end

**Error cases:**
- Path does not exist → `MagicError`
- Path is not a directory → `MagicError`

**Returns:** `Output::Text` with sorted listing.

---

### `%sx` — Shell Capture (Return as R Character Vector)

**File:** `src/magics/shell.rs`

**Arguments:** Shell command string (everything after `%sx`)

**Behavior:**
1. Determine shell: `$SHELL` env var, fallback to `/bin/sh`
2. Spawn `$SHELL -c <args>` via `std::process::Command` with `stdout(Stdio::piped())`
3. Capture stdout as a String
4. Split by newlines, filter empty trailing lines
5. Escape each line for R string safety:
   - Replace `\` with `\\`
   - Replace `"` with `\"`
   - Wrap in double quotes
6. Construct R assignment: `<var_name> <- c("line1", "line2", ...)`
7. Evaluate via `eval_string_raw_global()` to create the R variable
8. Return output showing the assigned values

**Variable name:** `sx_output` (overwritable by future `%sx -n varname` enhancement)

**Output format:**
```
character vector 'sx_output' assigned: [1] "line1" "line2" ...
```

**Error cases:**
- Shell command fails (non-zero exit) → `MagicError` with stderr content
- R eval fails → `MagicError` with R error message

---

### `%pushd` — Push Directory Onto Stack

**File:** `src/magics/shell.rs`

**Arguments:**
- (empty) → print the current directory stack without adding or changing directory
- `<path>` → save current directory onto `dir_stack`, then cd to path

**Behavior:**
1. Lock `ShellState`
2. Push current directory (`std::env::current_dir()`) onto `dir_stack`
3. Resolve target path
4. Call `std::env::set_current_dir(target)`
5. Print stack state

**Output format:**
```
~ /home/user/projects /tmp
```

**Error cases:**
- Path does not exist → `MagicError`

---

### `%popd` — Pop Directory From Stack

**File:** `src/magics/shell.rs`

**Arguments:** None

**Behavior:**
1. Lock `ShellState`
2. Pop the last entry from `dir_stack`
3. Call `std::env::set_current_dir(popped_path)`
4. Print remaining stack state

**Error cases:**
- Stack empty → `MagicError`

---

### `%dhist` — Directory History

**File:** `src/magics/shell.rs`

**Arguments:** None

**Behavior:**
1. Lock `ShellState`
2. Read `dir_history` (auto-populated by `%cd`)
3. Print each entry with a 1-based index

**Output format:**
```
 1: /home/user
 2: /tmp
 3: /home/user/projects
```

---

### `%run` — Source an R Script

**File:** `src/magics/file_magics.rs` (new)

**Arguments:** `<path> [args...]`

**Behavior:**
1. Resolve path using tilde expansion if needed
2. Verify file exists and is readable
3. Call `eval_string_raw_global("source('<resolved_path>')")`
4. Return result

**Future enhancement:** pass additional arguments to the script via `source(..., local=...)` or `commandArgs(trailingOnly=TRUE)`.

**Error cases:**
- File not found → `MagicError`
- R eval fails → `MagicError` with R error

---

### `%load` — Load File Contents Into REPL

**File:** `src/magics/file_magics.rs` (new)

**Arguments:** `<path>`

**Behavior:**
1. Resolve path using tilde expansion
2. Read file via `std::fs::read_to_string(path)`
3. Return file contents as `Output::Text`

**Note:** Unlike `%run`, this does not evaluate the file — it displays the contents in the REPL for review or editing.

**Error cases:**
- File not found → `MagicError`

---

## Files Changed

| File | Change |
|------|--------|
| `src/magics/shell.rs` | Add 6 handler structs + impls. Remove `#[allow(dead_code)]` from `ShellState`. Add helper for path resolution / OLDPWD management. |
| `src/magics/file_magics.rs` | **New file** — `Run` and `Load` handler structs + impls |
| `src/magics/mod.rs` | Add `pub mod file_magics;` |
| `src/magic.rs` | Register all 8 new handlers in `register_all()` |

---

## Testing

### New unit tests (in existing `#[cfg(test)]` blocks)

| Test | Location | What it verifies |
|------|----------|-----------------|
| `test_shell_cd_roundtrip` | `shell.rs` tests | Creates temp dir, cds into it, verifies cwd, cd's back |
| `test_shell_cd_minus` | `shell.rs` tests | Verifies OLDPWD swap |
| `test_shell_cd_nonexistent` | `shell.rs` tests | Verifies error on bad path |
| `test_shell_ls_empty_dir` | `shell.rs` tests | Lists a temp empty directory |
| `test_shell_sx_echo` | `shell.rs` tests | `%sx echo hello` — captures output |
| `test_shell_pushd_popd` | `shell.rs` tests | Push dir, verify stack, pop back, verify cwd |
| `test_shell_dhist` | `shell.rs` tests | Verify dir_history tracking after cd |
| `test_file_run_nonexistent` | `file_magics.rs` tests | Error on missing file |
| `test_file_load_nonexistent` | `file_magics.rs` tests | Error on missing file |

### Manual acceptance

```bash
cargo run -- -q
> %cd /tmp           # should change directory
> %pwd               # should show /tmp
> %ls                # should list /tmp contents
> %sx echo hello     # should create sx_output in R
> %pushd /home       # should push /tmp, cd to /home
> %popd              # should pop back to /tmp
> %dhist             # should show directory history
> %run test.R        # should source test.R
> %load test.R       # should display test.R contents
```

---

## Verification

After implementation:
```bash
cargo check           # 0 errors
cargo test --lib      # 154 + new tests pass
cargo test --test magic_framework  # 6 pass
```
