# Milestone D Phase 2 — Pre-Edit Hook for Context-Aware Editing

## Status

Approved. Design approach (fork reedline, add pre-edit hook) confirmed.

## Goal

Implement the deferred editing features that require buffer-context-aware
keypress handling: smart backspace, closing-delimiter skip, Enter
auto-indentation, context-aware Tab, context-aware auto-pairs, and
bracketed-paste auto-submit.

## Background

Phase 1 delivered always-insert auto-pairs, external editor, and native
bracketed paste using reedline 0.48's static keybinding overlay API.
However, reedline's `EditMode::parse_event()` receives only the raw
`KeyEvent` — never the buffer contents or cursor position. The remaining
editing transforms (`backspace`, `type_closing`, `type_closing_on_blank_indent`,
`indent_after_enter`, `insert_tab`, and `bracketed_paste` with auto-submit)
all require inspecting `(text, cursor)` at keypress time.

This limitation cannot be overcome within the public reedline 0.48 API.
The solution is a lightweight fork that adds a **pre-edit hook** — a
callback that fires before the edit mode, receives the buffer state, and
can return an override `ReedlineEvent`.

## Architecture

### 1. Vendored reedline copy

- Copy reedline 0.48.0 source into `vendor/reedline/` (the exact version
  already in `Cargo.lock`).
- Replace the crates.io dependency with a path dependency:
  ```toml
  [dependencies]
  reedline = { path = "vendor/reedline" }
  ```
- All existing reedline sub-dependencies (crossterm, nu-ansi-term, etc.)
