# orchard — R without the weight

[![CI](https://github.com/deanlipowicz/orchard/actions/workflows/ci.yml/badge.svg)](https://github.com/deanlipowicz/orchard/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/deanlipowicz/orchard)

**orchard** is the R REPL you keep open in a tmux pane while you work. Write your
R, DuckDB, Stan, or C in files — maybe alongside a Quarto document or a README —
and use orchard for the other half of the job: poking at data, testing
one-liners, inspecting objects, and iterating fast. No weight, no lock-in. Just a
REPL that loves living in a terminal.

It draws from the best. Magic commands from IPython. Syntax highlighting and
multiline editing from radian. Introspective help from the Julia REPL.
Autocomplete and autosuggest from zsh and fish. orchard doesn't try to be an
IDE — it tries to be the best REPL you've ever paired with your editor.

- **Repository:** [github.com/deanlipowicz/orchard](https://github.com/deanlipowicz/orchard)
- **Status:** v0.2 · 47 magic handlers · 307 tests · Linux
- **Docs:** [Development plan](docs/development-plan.md) · [Developer log](docs/developer-log.md) · [Specs](docs/superpowers/specs/) · [Plans](docs/superpowers/plans/)

---

## The workflow

Files are for the things you want to keep. You write your analysis, your model,
your pipeline as `.R` (or `.stan`, or `.sql`, or `.c`) under version control
with a proper project structure. Your editor — neovim, emacs, micro, whatever you
like — handles the heavy editing.

**orchard handles the other side.** The moment you wonder what that column looks
like, or whether a quick `aggregate()` does what you think, or hey I forgot the
arguments to `lm()` again — that's orchard's job. One tmux pane, zero context
switches, instant feedback.

```
┌─────────────────────────────────────────────────┐
│  tmux                                            │
│  ┌─────────────────┐ ┌─────────────────────────┐ │
│  │  neovim / emacs  │ │  orchard                │ │
│  │                  │ │                          │ │
│  │  analysis.R      │ │  > library(dplyr)        │ │
│  │  model.stan      │ │  > df |>                 │ │
│  │  queries.sql     │ │    summarise(...)        │ │
│  │  README.md       │ │  > View(iris)            │ │
│  │                  │ │  > %hist                 │ │
│  └─────────────────┘ └─────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

You source your files into orchard when you're ready. You explore. You find
bugs, fix them in your editor, and source again. The REPL is the sandbox; the
files are the record.

---

## What's inside

### 🪄 IPython-style magic commands (47 and counting)

Work across languages and tools without leaving the R session.

```
%reticulate        # hop into Python
%sql               # run SQL queries inline
%shell             # drop to a shell
%system            # run a command and capture output
%cd                # navigate your filesystem
%hist              # search your session history
%timeit            # benchmark R expressions
%pr                # profile R code with Rprof
%who / %whos       # list objects in scope
```

Every magic that IPython users know and love, reimagined for R.

### 🧠 Completion that actually understands R

Fourteen completion backends. Not a typo.

| What you type        | What orchard gives you                                 |
|----------------------|--------------------------------------------------------|
| `df$`                | Column names from schema data, live-loaded             |
| `df[[" `             | Same, for bracket access                               |
| `ggplot(df, aes(|`   | Column names from `df`, the ggplot function signature  |
| `lm(`                | Function argument completion via `formals()`           |
| `mtcars |>`          | Column names piped through from the left               |
| `\alpha` + Tab       | 1,983 LaTeX symbols, rendered inline                   |
| `;`                  | Filesystem paths, shell commands                       |
| misspelled function  | Fuzzy matching + "did you mean?" suggestions           |

Plus: R6 and refClass method completion, magic argument completion,
frequency-boosted ranking, and a static fast-path for 36 common datasets
and 10 packages — no R FFI needed.

### 🐚 A shell that lives inside R

Toggle between R and shell with `;` — no subshell, no friction.

```r
> ;ls *.csv
data.csv  results.csv

> cd ~/projects

> ;git log --oneline -5
```

Shell commands, `cd`, environment variable expansion, all right there. orchard
ships with a full shell mode that remembers its own working directory.

### ✨ Terminal polish that feels like home

- **Syntax highlighting** for R code, strings, comments, and operators
- **Multiline editing** that handles indentation and bracket pairing
- **Autosuggest** from your history, ghosted in grey, accepted with a keypress
- **Smart history** — compatible with radian's history format, filtered search, snapshot support
- **30+ configurable settings** via R's own `options()` system
- **13 custom keybindings** including smart backspace, kill-line, and prompt navigation

### ⚡ Why Rust?

orchard is a self-contained binary. No Python runtime, no R wrappers, no
dependency tangles. It links directly to libR via bindgen and runs the R event
loop in Rust. Same speed as the base R REPL, same session fidelity, zero
startup overhead you didn't ask for.

This is a ground-up Rust rewrite of the Python radian REPL — all the features
you loved, running closer to the metal with a stricter safety discipline
(`#![deny(unsafe_op_in_unsafe_fn)]` in every crate).

---

## Quick start

```bash
# Build from source (requires Rust and R >= 4.0)
cargo build --release
./target/release/orchard -q

# With a profile
./target/release/orchard --vanilla

# Point it at a specific R installation
ORCHARD_R_HOME=/usr/lib/R ./target/release/orchard
```

orchard discovers R automatically on Linux. macOS support is behind a feature
flag and in progress.

---

## Companion editors

orchard doesn't care which editor you use. But here are a few setups people
have enjoyed:

- **neovim** with `Nvim-R` or `R.nvim` — send lines to orchard with `<leader>l`
- **emacs** with ESS — set `inferior-R-program-name` to orchard's path
- **micro** — the built-in tmux integration pipe sends selections straight across
- **tmux** — bind a key to `send-keys` and never leave your editor

The pattern is always the same: write in your editor, explore in orchard, commit
the parts worth keeping.

---

## Contributing

orchard is young and welcomes contributions. The codebase is organized around a
few clear subsystems: the R runtime bridge (`r_runtime.rs`), the magic registry
(`magic.rs` / `magics/`), the 14-backend completion engine (`completion.rs`),
and the reedline prompt layer (`prompt.rs`).

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for build instructions, test commands,
and a tour of the architecture. Good first issues are tagged in the tracker.

---

## License

MIT OR Apache-2.0, at your option.
