# Autocomplete Upgrades Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raise orchard's completion quality from prefix-only to zsh/fish level by adding fuzzy matching, magic context completion, R argument descriptions, improved `[[` handling, R6 method completion, and correction suggestions.

**Architecture:** All changes land in `src/completion.rs` (new matchers and backends) and `src/prompt.rs` (wiring into `OrchardCompleter`). No new dependencies. R calls go through the existing `r_runtime::eval_string_raw_global` / `with_suppressed_stderr` globals.

**Tech Stack:** Rust, R (via C API), reedline

## Global Constraints

- All `unwrap()` calls in production code must have a safety-rationale comment.
- Follow existing code patterns in `completion.rs` and `prompt.rs` (no restructuring).
- Every new function must have unit tests in `completion::tests`.
- `cargo check` must be clean at every commit; `cargo clippy` must be clean on final commit.
- R-dependent functions (those calling `eval_string_raw_global`) get unit tests only when R is available (`#[cfg(not(target_arch = "wasm32"))]` or similar). Pure Rust logic gets unconditional tests.

---

### Task 1: Fuzzy / Substring Matching

**Files:**
- Modify: `src/completion.rs` (add `fuzzy_match` function, update existing filter calls)
- Test: `src/completion.rs` tests (new test module for `fuzzy_match`)

**Interfaces:**
- Produces: `fn fuzzy_match(name: &str, query: &str) -> bool` — returns true if query matches name via case-insensitive substring or character-skip scoring.

- [ ] **Step 1: Write tests for fuzzy_match**

```rust
#[test]
fn fuzzy_match_exact() {
    assert!(fuzzy_match("select", "select"));
}
#[test]
fn fuzzy_match_case_insensitive() {
    assert!(fuzzy_match("SELECT", "select"));
    assert!(fuzzy_match("select", "SELECT"));
}
#[test]
fn fuzzy_match_substring() {
    assert!(fuzzy_match("select", "sel"));
    assert!(fuzzy_match("select", "ect"));
}
#[test]
fn fuzzy_match_skip_chars() {
    // "sl" matches "select" — s...l
    assert!(fuzzy_match("select", "sl"));
    // "slt" matches "select" — s...l...ect
    assert!(fuzzy_match("select", "slt"));
}
#[test]
fn fuzzy_match_no_match() {
    assert!(!fuzzy_match("select", "xyz"));
    assert!(!fuzzy_match("select", "sx"));
}
```

- [ ] **Step 2: Run tests, expect failures**

Run: `cargo test --lib fuzzy_match`
Expected: compile error (function not defined)

- [ ] **Step 3: Implement fuzzy_match in src/completion.rs**

```rust
/// Case-insensitive fuzzy match.
/// Returns true if all characters of `query` appear in `name` in order
/// (not necessarily consecutively — the "subsequence" test).
pub fn fuzzy_match(name: &str, query: &str) -> bool {
    let name = name.to_lowercase();
    let query = query.to_lowercase();
    if query.is_empty() {
        return true;
    }
    let mut ni = name.chars().peekable();
    for qc in query.chars() {
        loop {
            match ni.next() {
                Some(nc) if nc == qc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --lib fuzzy_match`
Expected: 5/5 passed

- [ ] **Step 5: Update all completion filters to use fuzzy_match**

In `schema_completions`, `pipe_completions`, `package_completions`, `variable_selector_completions`, and anywhere else using `.starts_with(prefix)`:

- Replace `.filter(|n| n.starts_with(prefix))` with `.filter(|n| fuzzy_match(n, prefix))`
- Replace `.filter(|p| p.starts_with(prefix))` with `.filter(|p| fuzzy_match(p, prefix))`
- Keep LaTeX completions on exact prefix only (LaTeX is sensitive)

- [ ] **Step 6: Run full test suite**

Run: `cargo test --lib`
Expected: 278+ passed

- [ ] **Step 7: Commit**

---

### Task 2: Magic Context Completion

**Files:**
- Modify: `src/completion.rs` (add `magic_completions`)
- Modify: `src/prompt.rs` (wire in `OrchardCompleter`)
- Test: `src/completion.rs` tests

**Interfaces:**
- Produces: `fn magic_completions(line: &str) -> Option<(Vec<Completion>, usize)>`

- [ ] **Step 1: Write tests for magic context detection**
- [ ] **Step 2: Implement dispatch to file/dir/variable completers**
- [ ] **Step 3: Wire into OrchardCompleter before fallthrough**
- [ ] **Step 4: Test and commit**

---

### Task 3: R Function Argument Descriptions

**Files:**
- Modify: `src/completion.rs` (add `argument_completions`)
- Modify: `src/prompt.rs` (wire in)
- Test: `src/completion.rs`

**Interfaces:**
- Produces: `fn argument_completions(line: &str, cursor: usize) -> Option<(Vec<Completion>, usize)>`

- [ ] **Step 1: Write tests for argument context detection** (`lm(`, `data.frame(`, etc.)
- [ ] **Step 2: Implement R code to fetch formals() with descriptions**
- [ ] **Step 3: Wire into OrchardCompleter**
- [ ] **Step 4: Test and commit**

---

### Task 4: Improved `[[` Quoted Completion

**Files:**
- Modify: `src/completion.rs` (extend `extract_bracket_context`)
- Test: `src/completion.rs`

- [ ] **Step 1: Write tests for `obj[["partial` and `obj[[`"` contexts**
- [ ] **Step 2: Extend `extract_bracket_context` to handle quoted column names**
- [ ] **Step 3: Test and commit**

---

### Task 5: R6 / Reference Class Method Completion

**Files:**
- Modify: `src/completion.rs` (extend `resolve_schema`)
- Test: Requires R (`#[cfg]` gated)

- [ ] **Step 1: Write R code to detect R6 / refClass and extract method names**
- [ ] **Step 2: Integrate into `resolve_schema` under `$` operator**
- [ ] **Step 3: Test and commit**

---

### Task 6: Correction / "Did You Mean" Suggestions

**Files:**
- Modify: `src/completion.rs` (add `spellcheck_suggestions`)
- Modify: `src/prompt.rs` (wire in)
- Test: `src/completion.rs`

- [ ] **Step 1: Implement Levenshtein distance**
- [ ] **Step 2: Query R for `ls(all.names = TRUE)` on `package:base` + attached packages**
- [ ] **Step 3: Return top-3 nearest matches as completions**
- [ ] **Step 4: Wire into OrchardCompleter (lowest priority, only when other completions empty)**
- [ ] **Step 5: Test and commit**
