# Timing/Profiling Magic Handlers

**Date:** 2026-07-02
**Context:** Phase 2 of handler uplift (46 → 72+). Adds R expression timing and profiling.

## Scope

Add 3 new handlers:

| Handler | name() | Description |
|---------|--------|-------------|
| `%time` | `"time"` | Time a single R expression via `system.time()` |
| `%timeit` | `"timeit"` | Time an expression N times (default 7), report min/mean/max |
| `%prun` | `"prun"` | Profile an R expression via `Rprof()` + `summaryRprof()` |

**Running total:** 46 → 49 handlers.

## Architecture

**New file:** `src/magics/timing.rs` — Contains all 3 handler structs + `MagicHandler` impls.

**No new dependencies** — uses only `r_runtime::eval_string_raw_global`, string formatting, and `std` lib.

## Handler Specifications

### `%time`

- **Arguments:** Remaining R expression string
- **Behavior:** Calls `eval_string_raw_global("system.time({...})")`, parses user/system/elapsed output from the result, formats as:
  ```
  user: 0.123s  system: 0.045s  elapsed: 0.168s
  ```
- **Error:** Empty args → usage error. R eval failure → MagicError.

### `%timeit`

- **Arguments:** Remaining R expression string (optional `-n <count>` flag to set iterations)
- **Behavior:** Runs expression N times (default 7), each wrapped in `system.time()`. Collects elapsed seconds. Reports min/mean/max.
- **Output format:**
  ```
  7 loops, best of 3:
  min: 0.012s  mean: 0.015s  max: 0.019s
  ```
- **Error:** Empty args → usage error.

### `%prun`

- **Arguments:** Remaining R expression string
- **Behavior:** 
  1. `eval_string_raw_global("Rprof(tmp <- tempfile())")` 
  2. `eval_string_raw_global("{...}")` (the user's expression)
  3. `eval_string_raw_global("Rprof(NULL)")`
  4. `eval_string_raw_global("summaryRprof(tmp)")` and parse output
  5. Return formatted profiling table
- **Output format:** Text table showing function, calls, total time, self time.

## Files Changed

| File | Change |
|------|--------|
| `src/magics/timing.rs` | **Create** — 3 handler structs + impls |
| `src/magics/mod.rs` | Add `pub mod timing;` |
| `src/magic.rs` | Register 3 handlers in P3 section |

## Testing

- All 3 handlers call `eval_string_raw_global()` — unit tests marked `#[ignore]` (require R runtime)
- Parse-level tests verify arg handling and error cases
