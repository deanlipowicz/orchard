# Contributing to orchard

Hello! orchard is a young project and we'd love your help. Whether you're
reporting a bug, writing a new magic command, or adding a completion backend,
this guide should get you oriented.

## Getting started

You'll need Rust (stable) and R >= 4.0.

```bash
git clone git@github.com:deanlipowicz/orchard.git
cd orchard
cargo build
```

That's it. orchard discovers R on your system automatically. If R is installed
somewhere non-standard, point it there:

```bash
ORCHARD_R_HOME=/path/to/R cargo run
```

## Running the tests

```bash
# Fast: library + magic framework (no R needed)
cargo test --lib
cargo test --test magic_framework

# Slower: integration tests with actual embedded R
ORCHARD_TEST_R=1 cargo test --test embedded_r -- --test-threads=1 --nocapture

# Full check before a PR
cargo fmt --check
cargo clippy -- -D warnings
cargo test --lib && cargo test --test magic_framework
```

## How the codebase fits together

orchard is organized around a few big pieces. Here's a quick tour:

### The REPL loop (`src/r_runtime.rs`)
The heart of the project. Sets up embedded R via bindgen, registers console
callbacks (stdout, stderr, Ctrl-C, resizing), and runs the read-eval-print
loop. This is where magic dispatch happens — every line of input is checked
for shell mode (`;`), introspection (`?`), magic commands (`%`), or inline
shell (`!`) before it ever reaches R.

### Magic registry (`src/magic.rs`, `src/magics/`)
All 47 magic commands live here. The registry maps command names (like `%sql`
or `%timeit`) to handler structs that implement the `MagicHandler` trait. Each
handler lives in its own file under `magics/`. Adding a new magic is
straightforward — define the struct, implement `MagicHandler`, and register it
in `mod.rs`.

### Completion engine (`src/completion.rs`)
Fourteen completion backends, coordinated through a single completer. When you
hit Tab, the engine checks your context — are you after a `$`? Inside a function
call? Typing a LaTeX symbol? — and picks the right backend. The fuzzy-matcher
and frequency tracker provide scoring and ranking. Static data (dataset schemas,
package symbols) lives in `src/data/` as TSV files for fast zero-FFI lookups.

### Prompt and editor (`src/prompt.rs`)
Wraps reedline (the line editor) with orchard's syntax highlighter, completer,
input validator, and keybindings. This is where the editing experience gets
built.

### Shell integration (`src/shell.rs`)
Handles `;` shell mode (persistent and one-shot), `!` inline execution, `cd`,
and environment variable expansion. Shell state is tracked separately from R's
working directory.

### Settings (`src/settings.rs`)
Everything configurable — prompt colors, completion behavior, history size —
lives in R's own `options()` system. orchard reads them at startup and keeps
them in sync.

## What makes a good contribution

- **New magic commands** — if you use a workflow that deserves a `%something`,
  we probably want it. Check `src/magics/` for examples.
- **Completion improvements** — better dataset detection, smarter argument
  completion, support for more package ecosystems.
- **Bug fixes** — especially around R C API safety, memory management, or
  cross-platform behavior.
- **Documentation** — the README, dev log, and inline doc comments.

## Before you open a PR

1. Run `cargo fmt` and `cargo clippy -- -D warnings`
2. Run the test suites (lib + magic framework at minimum; embedded_r if you
   touched the R runtime)
3. Write a clear PR description — what problem does it solve, how did you test
   it
4. If you added unsafe code, document *exactly* why each unsafe block is safe
   (we enforce `#![deny(unsafe_op_in_unsafe_fn)]` crate-wide)
5. All `unwrap()` and `expect()` calls need a comment explaining why the
   None/Err case is unreachable

## Code style

We follow standard Rust conventions. A few project-specific notes:

- Use `anyhow::Result` in application code, `thiserror` for library error types
- `OnceLock<Mutex<...>>` for module-level shared state
- Prefer `eval_string_raw_global` for R evaluation from magic handlers — it's
  the safe, auditable path
- No unsafe blocks without an explicit safety comment

## Need help?

Open an issue with your question. We're friendly and we answer. Tag it with
"question" and we'll get back to you.
