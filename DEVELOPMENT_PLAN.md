# orchard 2014 Development Plan

> **⚠️ STALE DOCUMENT — 2026-07-02 Audit**
> Handler and test counts in this file (72+ handlers, 165 tests) are pre-recovery
> estimates. Current codebase has 38 registered handlers and ~160 tests passing.
> The regressed-components table refers to components that have been rebuilt
> during P0–P5. See `docs/developer-log.md` § 2026-07-02 — Documentation vs Code
> Audit for the full discrepancy catalog.

Audited with: `rustc 1.96.0`, `cargo clippy`, `cargo check`, manual source review.
Date: 2026-07-01
Tests: 165 pass (159 unit + 6 integration), 1 ignored. — **stale**; actual: ~160 tests (154 lib + 6 magic_framework).

## Phase 0 — Tooling Cleanup (safe to apply immediately)

These are auto-fixable or trivial changes with no behavioral impact.

### 0.1 Run `cargo clippy --fix --lib -p orchard`

Auto-applies 12 suggestions from clippy. Includes:

- `src/history.rs:234` — `sort_by(|a, b| b.id.cmp(&a.id))` → `sort_by_key(|b| std::cmp::Reverse(b.id))`
- `src/history.rs:566` — remove unnecessary `mut`
- `src/editing_hook.rs:41` — collapse nested `if` into let-chain
- `src/editing_hook.rs:55` — collapse `if` into match arm guard
- `src/r_runtime.rs:815, 936` — collapse nested `if`s
- `src/history.rs:456, 489, 518` — `field_reassign_with_default` in tests
- `src/editing_hook.rs:357` — `field_reassign_with_default` in tests
- `src/prompt.rs:446` — `field_reassign_with_default` in tests

### 0.2 Remove dead code

| Location | Symbol | Action |
|---|---|---|
| `src/env_setup.rs:76` | `r_version_at_least_42` | Remove or `#[allow(dead_code)]` with rationale |
| `src/r_runtime.rs:273` | `ConsoleState::history_arc` | Remove field if unused, or add comment explaining future use |
| `src/editing_hook.rs:363` | `fake_raw_event()` test helper | Remove (dead code, never called from any test) |

---

## Phase 1 — SEGFAULT Risks (must fix before production)

### 1.1 R protect/unprotect stack imbalance in `eval_code`

**Severity:** CRITICAL — causes use-after-free in R's GC.

**File:** `src/r_runtime.rs:574-605`

**Problem:** `eval_code` pushes 3 protections (input SEXP, expr SEXP, result SEXP). It calls `Rf_unprotect(2)` before returning a `ProtectedSexp` for the result. If any code between `eval_code`'s return and `ProtectedSexp::drop` calls `Rf_protect`, the protect stack shifts and `unprotect(1)` frees the wrong entry, leaving the result SEXP dangling.

**Fix strategy (pick one):**

- **Option A (recommended):** Replace `ProtectedSexp` with `R_PreserveObject`/`R_ReleaseObject` which use a separate global protection list, not the stack. Then protect/unprotect operations for input/expr are independent of result lifetime.
  ```rust
  // At result creation:
  ffi::R_PreserveObject(result);
  ffi::Rf_unprotect(2);  // release input + expr
  // Later, caller must call ReleaseObject explicitly or via a Drop wrapper
  ```

- **Option B:** Track absolute protect stack position:
  ```rust
  let protect_count = ffi::Rf_noprotect(); // hypothetical — R doesn't expose this
  // ... push/pop ...
  ffi::Rf_unprotect(ffi::Rf_noprotect() - protect_count);
  ```

- **Option C:** Change `ProtectedSexp::drop` to use `R_ReleaseObject` instead of `unprotect`, and keep all 3 SEXPs on the protect stack until the final drop.

**Files affected:** `src/r_runtime.rs` — `eval_code`, `ProtectedSexp`, all callers.

### 1.2 Signal handler reentrancy in `polled_events_handler`

**Severity:** CRITICAL — SIGALRM can fire during any `unsafe` R operation.

**File:** `src/r_runtime.rs:136-148`

**Problem:** The `SIGALRM` handler fires every 33ms and calls `R_PolledEvents()`. If this fires while `eval_code` is mid-way through protect/unprotect, or while `R_tryEval` is executing, the reentrant call can corrupt R's internal state.

**Fix:**

```rust
// In input_hook:
static REENTRANT: AtomicBool = AtomicBool::new(false);

extern "C" fn polled_events_handler(...) {
    if REENTRANT.swap(true, Ordering::SeqCst) {
        return;  // already inside, skip
    }
    unsafe {
        if let Some(polled) = super::ffi::R_PolledEvents {
            polled();
        }
    }
    REENTRANT.store(false, Ordering::SeqCst);
}
```

Also consider disabling the timer during `eval_code`'s critical section (after input/expr protect, before result protect).

**Files affected:** `src/r_runtime.rs` — `input_hook` module.

### 1.3 `sa_sigaction` union cast is platform-unsafe

**Severity:** HIGH — wrong function pointer installation on some platforms.

**File:** `src/r_runtime.rs:110`

**Problem:**
```rust
action.sa_sigaction = polled_events_handler as usize;
```
Clippy warning: "direct cast of function item into an integer." The `sigaction` struct uses a C union; `libc` exposes `sa_sigaction` as `usize`. This assumes `sa_handler` and `sa_sigaction` occupy the same union slot with the same size, which is true on mainstream Linux/macOS but not guaranteed.

**Fix:**
```rust
action.sa_sigaction = polled_events_handler as *const () as usize;
```
Or use `transmute`:
```rust
action.sa_sigaction = std::mem::transmute::<
    extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void),
    usize,
>(polled_events_handler);
```

**Files affected:** `src/r_runtime.rs:110`

---

## Phase 2 — Undefined Behavior (fix before release)

### 2.1 Concurrent `env::set_var` in tests

**Severity:** HIGH — UB when tests run in parallel (default).

**Files & call sites:**

| File | Line(s) | What mutates |
|---|---|---|
| `src/env_setup.rs` | 73, 194-199 | `RADIAN_VERSION`, `RADIAN_COMMAND_ARGS`, `R_HOME`, etc. |
| `src/dyld.rs` | 135-143 | `DYLD_INSERT_LIBRARIES`, `R_HOME`, etc. |
| `src/shell.rs` | 54 | `OLDPWD` |
| `src/editing.rs` | 372-387 | `EDITOR`, `VISUAL` |
| `src/shell.rs` (tests) | 123 | `RADIAN_RS_TEST_DIR` |

**Fix (pick one):**

- **Option A:** Add `#[serial_test::serial]` to all tests that call `unsafe { env::set_var }` and run with `--test-threads=1` or use `serial_test` crate.

- **Option B:** Extract env mutation into a helper that uses a global `Mutex<()>`:
  ```rust
  static ENV_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
  fn set_env_safe(key: &str, val: &str) {
      let _lock = ENV_MUTEX.lock().unwrap();
      unsafe { std::env::set_var(key, val) };
  }
  ```

  `env_setup.rs` already has an `ENV_LOCK` for tests — this pattern should be extended to cover all env mutations.

- **Option C:** Replace `unsafe { env::set_var }` with a `Command::env()`-based approach where environment changes are passed to subprocesses rather than modifying the current process's environment.

### 2.2 UTF-8 boundary read in `find_matching_bracket`

**Severity:** MEDIUM — produces wrong results for multi-byte chars.

**File:** `src/prompt.rs:378`

**Problem:**
```rust
let close = line.as_bytes()[cursor - 1] as char;
```
Reads a single byte. If cursor is after a multi-byte character (e.g., `é` = 2 bytes), this reads a continuation byte and casts it to `char`, producing a garbage character.

**Fix:**
```rust
fn char_before(line: &str, cursor: usize) -> Option<char> {
    line[..cursor].chars().last()
}
```

**Files affected:** `src/prompt.rs:374-397`

---

## Phase 3 — Logic & Robustness Bugs

### 3.1 Missing error handling in `utc_now()`

**File:** `src/history.rs:380-396`

**Problem:** `libc::time(&mut now)` can fail (returns -1). If it fails, `gmtime_r(-1, ...)` produces a valid-looking but wrong timestamp. History file timestamps become incorrect.

**Fix:** Check return value of `libc::time`:
```rust
if libc::time(&mut now) == -1 {
    // fallback: use current system time via std::time
    let now_std = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    now = now_std.as_secs() as libc::time_t;
}
```

Alternatively, replace the entire function with `chrono::Utc::now()` or `time::OffsetDateTime::now_utc()` from the `time` crate.

### 3.2 `remove_nested_parens` O(n²) worst case

**File:** `src/completion.rs:281-328`

**Problem:** For deeply nested parentheses (e.g., `(((a)))`), each iteration rebuilds the string from scratch. On pathological input this could cause significant slowdown.

**Fix:** Rewrite using a single-pass approach with position tracking rather than repeated string reconstruction.

### 3.3 `completion_prefix` unchecked `try_into().unwrap()`

**File:** `src/prompt.rs:330-336`

**Problem:**
```rust
fn completion_prefix(settings: &ConsoleSettings) -> usize {
    settings
        .completion_prefix_length
        .max(0)
        .try_into()
        .unwrap_or(0)
}
```
`completion_prefix_length` is `i32` and already `max(0)` — so `try_into()` always succeeds. The `unwrap()` is fine but is technically an `unwrap()` without a documented safety rationale. `unwrap_or(0)` handles it better.

---

## Phase 4 — Future Enhancements & Deprecations

### 4.1 Replace blocklisted raw libc calls

Consider replacing these `unsafe` libc calls with safe Rust equivalents where possible:

| Location | Current | Replacement |
|---|---|---|
| `src/history.rs:380-396` | `libc::time`, `libc::gmtime_r` | `std::time::SystemTime` + `chrono` |
| `src/r_runtime.rs:860-870` | `libc::ioctl` with `winsize` | `terminal_size` crate |
| `src/r_runtime.rs:104-149` | `libc::sigaction`, `libc::setitimer` | `signal-hook` crate |

### 4.2 Dependency audit

Run `cargo audit` (if `cargo-audit` installed) to check for vulnerable dependencies. Current direct deps include `bindgen`, `libc`, `memchr`, `regex`, `shell_words`, `clap`, `serde_json`, `nu-ansi-term`, `reedline` (vendored).

### 4.3 Add `#![deny(unsafe_op_in_unsafe_fn)]`

The crate widely uses `unsafe` blocks inside `unsafe fn`s. This lint was added in Rust 1.52 and is a future edition requirement. Running `cargo clippy` without additional `--deny` flags shows no warnings for this, but it should be explicitly enabled.

---

## Implementation Order

```
Phase 0 → cargo clippy --fix, remove dead code
    │
Phase 1.1 → Fix protect/unprotect stack (most crashes)
    │
Phase 1.2 → Signal handler reentrancy guard
    │
Phase 1.3 → Fix function-to-integer cast
    │
Phase 2.1 → Serialize env mutations in tests
    │
Phase 2.2 → Fix UTF-8 bracket matching
    │
Phase 3 → Logic bugs (utc_now, remove_nested_parens)
    │
Phase 4 → Modernization (chrono, terminal_size, signal-hook)
```

## Appendix: Recovery Incident (2026-07-02)

The project was accidentally deleted and recovered from OpenCode session database logs on 2026-07-02. All 76 source files were recovered, but some components were **reconstructed from fragmented tool-call output** and may differ from the originals.

### Regressed Components

| Component | Status | Work Required |
|-----------|--------|---------------|
| `src/magics/inspect.rs` | 18 of ~40 handlers return `Ok(Output::Text("not implemented"))` | Rewrite the `fn run` bodies for handlers: `Objects`, `Who`, `Whos`, `WhoLs`, `Rm`, `Clear`, `Str`, `Head`, `Skim`, `Dim`, `Names`, `Plot`, `Tidy`, `View`, `Pdoc`, `Pdef`, `Psource`, `Pfile`. The database fragments only captured the handler struct and `fn name` definitions — the R evaluation code was never read. Reference the Python third_party/radian-upstream for original magic behavior. |
| `src/history.rs:660` | `get_history_snapshot()` stubbed to return empty Vec | Reimplement to return actual history entries from the in-memory store. |
| `src/magics/history_magics.rs:5` | `get_history_snapshot()` stubbed to return empty Vec, `resolve_range()` returns `None` | Reimplement history filtering functions. |
| `src/magics/edit_magic.rs` | 5 call sites depend on stubbed history functions | The `%edit` magic will have no effect until history functions are reimplemented. |
| `vendor/reedline/src/engine.rs` | `pre_edit_hook` was reconstructed from API surface, not from original vendored code | Verify the hook is called at the correct point in the event loop and receives the right event types. |
| `src/magics/edit_magic.rs` | `tempfile` dependency removed during recovery | The `create_temp_file` function now writes to `/tmp/` directly. No behavioral change expected. |
| `src/magic.rs:104-105` | `Hist` and `HistN` handler registrations commented out (modules not present) | Either implement `%hist`/`%hist_n` handlers or remove from registry. |

### Recovery Methodology

Files were reconstructed by merging all available `read` tool outputs from the OpenCode session database (`~/.local/share/opencode/opencode.db`, `part` table). Where the Read tool truncated output (at 25 or 260 lines, or at 50 KB), files were reconstructed from multiple reads at different offsets. Handlers whose `fn run` bodies were never captured by any read received stub implementations. The project compiles (`cargo check` passes) but 18 handlers are non-functional stubs.

### Recovery Verification

After each regressed component is fixed:
```bash
cargo check
cargo test --lib         # unit tests
cargo test --test magic_framework  # magic handler tests
```

## Verification

After each phase:
```bash
cargo check
cargo clippy --all-targets -- -D warnings
cargo test --lib
cargo test  # for integration tests (requires R installed)
```
