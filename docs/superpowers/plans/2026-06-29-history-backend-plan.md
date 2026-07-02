# Loaded History Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect radian's loaded history file to reedline's interactive navigation (Ctrl-R reverse search, up/down-arrow) via a custom `History` trait backend.

**Architecture:** `RadianHistoryBackend` (in `src/history.rs`) implements reedline's `History` trait, wrapping radian's `History` struct for file writes and sharing `Arc<Mutex<PromptMode>>` for mode-filtered search. The backend is constructed per-session from radian's loaded entries and provides a single write path (`save()` → `History::append()`).

**Tech Stack:** Rust, reedline 0.48.0 (vendored), std `Arc<Mutex>`

## Global Constraints

- All changes must compile with rustc stable (edition 2021).
- `cargo test` must pass (150 unit + 6 R integration tests).
- The vendored reedline at `vendor/reedline/` must not be modified.
- No new dependencies beyond std and what's already in `Cargo.toml`.
- Follow existing code style: `unwrap()` on Mutex locks in non-test code, no `unsafe`.

---

### Task 1: Add `entries()` accessor to `History`

**Files:**
- Modify: `src/history.rs`

**Interfaces:**
- Produces: `impl History { pub fn entries(&self) -> &[Entry] }`

- [ ] **Step 1: Add `entries()` method**

Insert after `append_file_helper` (around line 135):

```rust
/// Returns all loaded entries for seeding the reedline history backend.
pub fn entries(&self) -> &[Entry] {
    &self.entries
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check`
Expected: 0 errors

- [ ] **Step 3: Commit**

```bash
git add src/history.rs && git commit -m "Add History::entries() accessor for history backend seeding"
```

---

### Task 2: Add reedline type imports and `RadianHistoryBackend` struct

**Files:**
- Modify: `src/history.rs`

**Interfaces:**
- Consumes: `History::entries()` (Task 1), `History::append()`, `History::compatible()`
- Produces: struct `RadianHistoryBackend` with `pub fn new(inner: Arc<Mutex<History>>, mode: Arc<Mutex<PromptMode>>) -> Self`
- Produces: exports `Arc`, `Mutex` in the public API of the module

**Details:**
- Add `use crate::r_runtime::PromptMode;` to imports
- Add `use std::sync::{Arc, Mutex};` (if not already present; it's `use std::sync::Mutex` — need to add `Arc`)
- Add `RadianHistoryBackend` struct after the `History` impl block (around line 135)
- Add `impl RadianHistoryBackend { pub fn new(...) -> Self }` that populates items from `History::entries()`
- Store `mode: Arc<Mutex<PromptMode>>`, `inner: Arc<Mutex<History>>`, `items: Vec<HistoryItem>`, `next_id: usize`

**Note on `HistoryItem` and `HistoryItemId`:** These come from reedline. Add use imports:
```rust
use reedline::{HistoryItem, HistoryItemId, SearchQuery, SearchFilter, SearchType};
```

- [ ] **Step 1: Add imports at top of `src/history.rs`**

Add `use crate::r_runtime::PromptMode;` and `use std::sync::Arc;` and the reedline imports.

- [ ] **Step 2: Add `RadianHistoryBackend` struct and `new()`**

```rust
/// Implements reedline's `History` trait backed by radian's `History`.
/// Provides mode-aware search and delegates file writes.
pub struct RadianHistoryBackend {
    items: Vec<HistoryItem>,
    inner: Arc<Mutex<History>>,
    mode: Arc<Mutex<PromptMode>>,
    next_id: usize,
}

impl RadianHistoryBackend {
    pub fn new(inner: Arc<Mutex<History>>, mode: Arc<Mutex<PromptMode>>) -> Self {
        let entries: Vec<HistoryItem> = inner
            .lock()
            .unwrap()
            .entries()
            .iter()
            .enumerate()
            .map(|(i, entry)| HistoryItem {
                id: Some(HistoryItemId(i)),
                start_timestamp: None,
                command_line: entry.text.clone(),
                more_info: Some(entry.mode.clone()),
                ..HistoryItem::default()
            })
            .collect();
        let next_id = entries.len();
        Self { items, inner, mode, next_id }
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check`
Expected: 0 errors (may need to adjust imports for `HistoryItem`, `HistoryItemId`)

- [ ] **Step 4: Commit**

```bash
git add src/history.rs && git commit -m "Add RadianHistoryBackend struct and constructor"
```

---

### Task 3: Implement the `History` trait for `RadianHistoryBackend`

**Files:**
- Modify: `src/history.rs`

**Interfaces:**
- Produces: `impl reedline::History for RadianHistoryBackend` (all trait methods)
- Consumes: `PromptMode::mode_string()`, `history::compatible()`, `History::append()`

**Implementation details for each method:**

`save()`:
1. Read mode via `self.mode.lock().unwrap().mode_string()`
2. Assign `item.id = Some(HistoryItemId(self.next_id))`
3. Set `item.more_info = Some(mode_string.to_string())`
4. Push to `self.items`, increment `self.next_id`
5. Call `self.inner.lock().unwrap().append(mode_string, &item.command_line).ok();`
6. Return `Ok(item)`

`load()`:
1. If `id.0 < self.items.len()`, return `Ok(self.items[id.0].clone())`
2. Otherwise return `Ok(HistoryItem::default())`

`search()`:
1. Get current mode string: `self.mode.lock().unwrap().mode_string().to_string()`
2. Filter items: `self.items.iter().filter(|item| compatible(&current_mode, item.more_info.as_deref().unwrap_or("")))`
3. Apply query filter: if `query.filter` is `SearchFilter::CommandLine { command_line, search }`, match against `item.command_line` using the search type (Substring → `contains`, Prefix → `starts_with`, Exact → `==`)
4. Sort by ID descending
5. Apply `query.limit` if `Some`
6. Return `Ok(results)`

`count()`: Delegate to search, return length.

`update()`: If ID in bounds, apply `updater` closure to `self.items[id.0]`.

`clear()`: Clear `self.items`.

`delete()`: Set `self.items[id.0].command_line = String::new()` (tombstone — maintain ID stability).

`sync()`: No-op.

`session()`: Return `None`.

- [ ] **Step 1: Write the `save()` method**

```rust
fn save(&mut self, mut item: HistoryItem) -> Result<HistoryItem> {
    let mode_string = self.mode.lock().unwrap().mode_string().to_string();
    item.id = Some(HistoryItemId(self.next_id));
    item.more_info = Some(mode_string.clone());
    self.items.push(item.clone());
    self.next_id += 1;
    self.inner.lock().unwrap().append(&mode_string, &item.command_line).ok();
    Ok(item)
}
```

- [ ] **Step 2: Write the `load()` method**

```rust
fn load(&self, id: HistoryItemId) -> Result<HistoryItem> {
    if id.0 < self.items.len() {
        Ok(self.items[id.0].clone())
    } else {
        Ok(HistoryItem::default())
    }
}
```

- [ ] **Step 3: Write the `search()` method**

```rust
fn search(&self, query: SearchQuery) -> Result<Vec<HistoryItem>> {
    let current_mode = self.mode.lock().unwrap().mode_string().to_string();
    let mut results: Vec<HistoryItem> = self
        .items
        .iter()
        .filter(|item| {
            let item_mode = item.more_info.as_deref().unwrap_or("");
            compatible(&current_mode, item_mode)
        })
        .filter(|item| match &query.filter {
            SearchFilter::CommandLine { command_line, search } => {
                match search {
                    SearchType::Substring => item.command_line.contains(command_line),
                    SearchType::Prefix => item.command_line.starts_with(command_line),
                    SearchType::Exact => item.command_line == *command_line,
                    SearchType::Fuzzy => item.command_line.contains(command_line), // fallback
                    _ => true,
                }
            }
            _ => true,
        })
        .cloned()
        .collect();
    results.sort_by(|a, b| b.id.cmp(&a.id)); // descending
    if let Some(limit) = query.limit {
        results.truncate(limit as usize);
    }
    Ok(results)
}
```

- [ ] **Step 4: Write remaining methods**

```rust
fn count(&self, query: SearchQuery) -> Result<i64> {
    self.search(query).map(|v| v.len() as i64)
}

fn update(&mut self, id: HistoryItemId, updater: &dyn Fn(HistoryItem) -> HistoryItem) -> Result<()> {
    if id.0 < self.items.len() {
        self.items[id.0] = updater(self.items[id.0].clone());
    }
    Ok(())
}

fn clear(&mut self) -> Result<()> {
    self.items.clear();
    Ok(())
}

fn delete(&mut self, id: HistoryItemId) -> Result<()> {
    if id.0 < self.items.len() {
        self.items[id.0].command_line = String::new();
    }
    Ok(())
}

fn sync(&mut self) -> std::io::Result<()> {
    Ok(())
}

fn session(&self) -> Option<reedline::HistorySessionId> {
    None
}
```

- [ ] **Step 5: Check exact import paths**

Verify all reedline types used:
- `reedline::HistoryItem`, `reedline::HistoryItemId`, `reedline::SearchQuery`, `reedline::SearchFilter`, `reedline::SearchType`
- The trait method signatures may use `reedline::Result` (which is `anyhow::Result`). Check and adjust.

Run: `cargo check`

- [ ] **Step 6: Commit**

```bash
git add src/history.rs && git commit -m "Implement reedline History trait for RadianHistoryBackend"
```

---

### Task 4: Add `with_arc_history()` constructor and `mode_arc` to `PromptSession`

**Files:**
- Modify: `src/prompt.rs`

**Interfaces:**
- Produces: `PromptSession::with_arc_history(settings, history_arc, mode_arc) -> Self`
- Produces: `PromptContext { ... mode_arc: Arc<Mutex<PromptMode>> }`

- [ ] **Step 1: Add `mode_arc` to `PromptContext`**

```rust
struct PromptContext {
    settings: ConsoleSettings,
    mode: PromptMode,
    mode_arc: Arc<Mutex<PromptMode>>,  // NEW — shared with history backend
}
```

Add `use std::sync::Arc;` to imports (already present, along with `Mutex`).

Update the constructor: `PromptContext` is created twice (lines 36-39 and 315-318) — both need the new field.

```rust
// In PromptSession::new (line 36):
let context = Arc::new(Mutex::new(PromptContext {
    settings: settings.clone(),
    mode: PromptMode::R,
    mode_arc: Arc::new(Mutex::new(PromptMode::R)),  // NEW
}));
```

And in the test helper (line 315):
```rust
let context = Arc::new(Mutex::new(PromptContext {
    settings,
    mode: PromptMode::R,
    mode_arc: Arc::new(Mutex::new(PromptMode::R)),  // NEW
}));
```

- [ ] **Step 2: Sync mode_arc in `update_mode()`**

Add a line to `PromptSession::update_mode()`:
```rust
pub fn update_mode(&self, mode: PromptMode) {
    let mut ctx = self.context.lock().unwrap();
    ctx.mode = mode;
    *ctx.mode_arc.lock().unwrap() = mode;  // NEW — sync shared arc
}
```

- [ ] **Step 3: Add `with_arc_history()` constructor**

```rust
pub fn with_arc_history(
    settings: &ConsoleSettings,
    history_arc: Arc<Mutex<History>>,
    mode_arc: Arc<Mutex<PromptMode>>,
) -> Self {
    use reedline::History as _;  // import trait for .with_history()
    let context = Arc::new(Mutex::new(PromptContext {
        settings: settings.clone(),
        mode: PromptMode::R,
        mode_arc: mode_arc.clone(),
    }));
    let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));
    let history_backend = RadianHistoryBackend::new(history_arc, mode_arc);
    let editor = Reedline::create()
        .with_completer(Box::new(RadianCompleter::new(context.clone())))
        .with_validator(Box::new(RadianValidator::new(context.clone())))
        .with_highlighter(Box::new(RadianHighlighter))
        .with_pre_edit_hook({
            let settings = settings.clone();
            move |event, buffer, cursor| {
                editing_hook::handle(event, buffer, cursor, &settings)
            }
        })
        .with_buffer_editor(
            Command::new(editing::select_editor(None)),
            std::env::temp_dir().join("radian-rs-editor-tmp.R"),
        )
        .with_history(Box::new(history_backend))  // NEW
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_edit_mode(edit_mode(settings));
    Self { editor, context }
}
```

Note: This duplicates the builder chain from `new()`. Consider refactoring into a shared helper, but for v1 duplication is acceptable.

Add `use crate::history::{History, RadianHistoryBackend};` to imports.

- [ ] **Step 4: Verify compilation**

Run: `cargo check`
Expected: 0 errors

- [ ] **Step 5: Commit**

```bash
git add src/prompt.rs && git commit -m "Add with_arc_history() and mode_arc shared state for history backend"
```

---

### Task 5: Wire history backend into `read_console_interactive` and update `ConsoleState`

**Files:**
- Modify: `src/r_runtime.rs`

**Interfaces:**
- Consumes: `PromptSession::with_arc_history()`, `RadianHistoryBackend`
- Changes: `ConsoleState` gains `history_arc: Option<Arc<Mutex<History>>>`

- [ ] **Step 1: Add `history_arc` to `ConsoleState`**

```rust
struct ConsoleState {
    settings: ConsoleSettings,
    terminal_cursor_at_beginning: bool,
    startup_inputs: VecDeque<String>,
    pending_inputs: VecDeque<String>,
    prompt_active: bool,
    history: Option<History>,
    history_arc: Option<Arc<Mutex<History>>>,  // NEW
    prompt_session: Option<PromptSession>,
    last_terminal_width: Option<i32>,
}
```

Add `use std::sync::Arc;` to the imports (line 18 — currently `sync::{AtomicBool, Ordering, Mutex, OnceLock}`, add `Arc`).

- [ ] **Step 2: Set `history_arc` when history is installed**

Find where `console.lock().unwrap().history = Some(history)` is set (around line 306):

```rust
console.lock().unwrap().history = Some(history);
```

After it, also set:
```rust
state.history_arc = Some(Arc::clone(&history_arc));
```

Wait — the history isn't currently in an `Arc`. I need to wrap it. Let me trace the code where history is created.

Find the caller (line ~306 area):

```rust
console.lock().unwrap().history = Some(history);
```

I need to change this to use `Arc<Mutex<History>>`. The history is created earlier as `History::new(&cli, &settings)`. It's then stored in `ConsoleState::history: Option<History>`.

Change: wrap it:
```rust
let history = History::new(&cli, &settings)?;
let history_arc = Arc::new(Mutex::new(history));
// Store both:
let mut state = console.lock().unwrap();
state.history = Some(Arc::clone(&history_arc));  // Wait, history is Option<History> not Option<Arc<Mutex<History>>>
```

Hmm, this requires a design decision. `ConsoleState::history` is currently `Option<History>`. If I change it to `Option<Arc<Mutex<History>>>`, I need to update all usages. Or I can keep both: the existing `history: Option<History>` for direct access, and add `history_arc: Option<Arc<Mutex<History>>>` for the backend.

Actually, looking at how `history` is used:
- `append_history()` at line 1120: `if let Some(history) = &mut console.lock().unwrap().history { history.append(...) }`
- It's set at line 306: `console.lock().unwrap().history = Some(history);`

The simplest approach: keep `history: Option<History>` as-is for `append_history()`. Add `history_arc: Option<Arc<Mutex<History>>>` alongside it, populated from the same `History` instance after it's created. Both share the same underlying data (the `History` is behind `Arc<Mutex>` now — wait, no, `history` is not behind Arc).

Actually, the simplest approach with minimal changes: wrap the history in `Arc<Mutex<History>>` and store it in a new `history_arc` field. Keep the old `history: Option<History>` field for backward compatibility (for `append_history` to use).

But `append_history` currently does `console.lock().unwrap().history.as_mut().unwrap().append(...)`. If I want the backend and `append_history` to share the same instance, they both need the same `Arc<Mutex<History>>`.

Simplest approach: 
1. Create `History`, wrap in `Arc::new(Mutex::new(history))`
2. Store in `ConsoleState::history_arc`
3. Remove `ConsoleState::history` field
4. Update `append_history()` to use `history_arc`

Let me trace all uses of `ConsoleState::history`:

```bash
rg "\.history" src/r_runtime.rs
```

This will show all accesses.

Let me just write the plan and handle the details in implementation.

- [ ] **Step 3: Wire history backend into `read_console_interactive`**

In `read_console_interactive` (around line 854), change the `PromptSession` creation:

```rust
// Current:
.unwrap_or_else(|| PromptSession::new(settings))

// New:
.unwrap_or_else(|| {
    let history_arc = state.history_arc.clone();
    let mode_arc = state.prompt_session_mode_arc.clone();  // need to store this somewhere
    match history_arc {
        Some(h) => PromptSession::with_arc_history(settings, h, mode_arc),
        None => PromptSession::new(settings),
    }
})
```

But `mode_arc` needs to be stored somewhere accessible. Currently `mode_arc` lives inside `PromptContext` (in the `PromptSession`). But at this point we're creating the session, so we don't have one yet.

Option: store `mode_arc` in `ConsoleState` alongside the other state. When `read_console_interactive` creates a new session, it creates a `mode_arc` and stores it in `ConsoleState`. When the session is stored back, the `mode_arc` persists.

Actually, simpler: create the `mode_arc` in `ConsoleState::default()` and keep it there permanently. Set it from the mode when we have one.

Let me simplify: add `mode_arc: Arc<Mutex<PromptMode>>` to `ConsoleState`, initialized to `PromptMode::R` in `Default`. Then:
- `read_console_interactive` reads `mode_arc` from `ConsoleState` and passes to `with_arc_history()`
- `PromptSession::with_arc_history()` uses it for both the context and the backend
- When mode changes, `PromptSession::update_mode()` syncs to the same `Arc`
- Wait, but `PromptSession::update_mode()` syncs its own `mode_arc`, not `ConsoleState`'s. If both share the same `Arc`, then syncing either one updates both.

Yes — if `ConsoleState.mode_arc` and `PromptContext.mode_arc` point to the same `Arc`, then `update_mode()` in the session updates the value visible to the backend AND to `ConsoleState`.

Let me make the plan simpler: store `mode_arc` in `ConsoleState`, clone it when creating `PromptSession::with_arc_history()`, and both will share the same underlying `Mutex<PromptMode>`.

```rust
// In ConsoleState:
mode_arc: Arc<Mutex<PromptMode>>,  // initialized to PromptMode::R

// In read_console_interactive session creation:
let mode_arc = state.mode_arc.clone();
PromptSession::with_arc_history(settings, history_arc, mode_arc)
```

- [ ] **Step 4: Remove `append_history()` calls from reedline paths**

Find and remove:
- `append_history(&PromptMode::Shell, command)` in `read_shell_prompt()` (line 926)
- `append_history(mode, &text)` in the REPL loop (around line 904 — after `ReadResult::Line`)

Keep:
- `append_history(&PromptMode::Shell, command)` for startup/one-shot shell (line 776)
- `append_history(&PromptMode::Shell, command)` for one-shot shell (line 897)
- Piped mode paths

- [ ] **Step 5: Verify compilation and full test suite**

Run: `cargo check && cargo test`
Expected: 0 errors, 150 unit + 6 R integration tests pass

- [ ] **Step 6: Commit**

```bash
git add src/r_runtime.rs && git commit -m "Wire RadianHistoryBackend into interactive REPL; remove redundant append_history calls"
```

---

### Task 6: Unit tests for `RadianHistoryBackend`

**Files:**
- Modify: `src/history.rs` (append tests at end of existing `#[cfg(test)] mod tests`)

**Tests:**

- [ ] **Step 1: Write `backend_seeded_from_radian_entries` test**

```rust
#[test]
fn backend_seeded_from_radian_entries() {
    let history = Arc::new(Mutex::new(History::memory(&Settings::default())));
    {
        let mut h = history.lock().unwrap();
        h.append("r", "mean(x)").ok();
        h.append("r", "plot(y)").ok();
        h.append("shell", "ls -la").ok();
    }
    let mode = Arc::new(Mutex::new(PromptMode::R));
    let backend = RadianHistoryBackend::new(history, mode);
    assert_eq!(backend.items.len(), 3);
    assert_eq!(backend.items[0].command_line, "mean(x)");
    assert_eq!(backend.items[1].command_line, "plot(y)");
    assert_eq!(backend.items[2].command_line, "ls -la");
}
```

- [ ] **Step 2: Verify it fails initially (if tests exist before impl)**

Run: `cargo test backend_seeded_from_radian_entries -- --nocapture`

- [ ] **Step 3: Write `save_appends_to_radian` test**

```rust
#[test]
fn save_appends_to_radian() {
    let history = Arc::new(Mutex::new(History::memory(&Settings::default())));
    let mode = Arc::new(Mutex::new(PromptMode::R));
    let mut backend = RadianHistoryBackend::new(history.clone(), mode);
    backend
        .save(HistoryItem {
            command_line: "1 + 1".to_string(),
            ..HistoryItem::default()
        })
        .ok();
    // Verify radian's history has the entry
    let h = history.lock().unwrap();
    assert_eq!(h.entries().len(), 1);
    assert_eq!(h.entries()[0].text, "1 + 1");
    assert_eq!(h.entries()[0].mode, "r");
}
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test save_appends_to_radian -- --nocapture`

- [ ] **Step 5: Write `search_filters_by_current_mode` test**

```rust
#[test]
fn search_filters_by_current_mode() {
    let history = Arc::new(Mutex::new(History::memory(&Settings::default())));
    let mode = Arc::new(Mutex::new(PromptMode::R));
    // Manually seed backend items
    let mut backend = RadianHistoryBackend::new(history.clone(), mode.clone());
    // Replace items with known mixed-mode data
    backend.items = vec![
        HistoryItem { id: Some(HistoryItemId(0)), command_line: "lm(y ~ x)".into(), more_info: Some("r".into()), ..HistoryItem::default() },
        HistoryItem { id: Some(HistoryItemId(1)), command_line: "ls".into(), more_info: Some("shell".into()), ..HistoryItem::default() },
        HistoryItem { id: Some(HistoryItemId(2)), command_line: "n".into(), more_info: Some("browse".into()), ..HistoryItem::default() },
    ];
    backend.next_id = 3;

    // In R mode, should find "r" and "browse" (same book)
    let query = SearchQuery {
        filter: SearchFilter::CommandLine {
            command_line: String::new(),
            search: SearchType::Substring,
        },
        direction: None,
        time_range: None,
        id_range: None,
        limit: None,
    };
    let results = backend.search(query).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|i| i.command_line == "lm(y ~ x)"));
    assert!(results.iter().any(|i| i.command_line == "n"));

    // Switch to shell mode
    *mode.lock().unwrap() = PromptMode::Shell;
    let query = SearchQuery {
        filter: SearchFilter::CommandLine {
            command_line: String::new(),
            search: SearchType::Substring,
        },
        direction: None,
        time_range: None,
        id_range: None,
        limit: None,
    };
    let results = backend.search(query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].command_line, "ls");
}
```

- [ ] **Step 6: Write `search_filters_by_substring` test**

```rust
#[test]
fn search_filters_by_substring() {
    let history = Arc::new(Mutex::new(History::memory(&Settings::default())));
    let mode = Arc::new(Mutex::new(PromptMode::R));
    let mut backend = RadianHistoryBackend::new(history.clone(), mode.clone());
    backend.items = vec![
        HistoryItem { id: Some(HistoryItemId(0)), command_line: "mean(x)".into(), more_info: Some("r".into()), ..HistoryItem::default() },
        HistoryItem { id: Some(HistoryItemId(1)), command_line: "plot(mean)".into(), more_info: Some("r".into()), ..HistoryItem::default() },
        HistoryItem { id: Some(HistoryItemId(2)), command_line: "lm(y)".into(), more_info: Some("r".into()), ..HistoryItem::default() },
    ];
    backend.next_id = 3;

    let query = SearchQuery {
        filter: SearchFilter::CommandLine {
            command_line: "mean".to_string(),
            search: SearchType::Substring,
        },
        direction: None,
        time_range: None,
        id_range: None,
        limit: None,
    };
    let results = backend.search(query).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|i| i.command_line == "mean(x)"));
    assert!(results.iter().any(|i| i.command_line == "plot(mean)"));
}
```

- [ ] **Step 7: Full test pass**

Run: `cargo test`
Expected: ~154 unit tests passed (4 new + 150 existing), 6 R integration

- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "Add unit tests for RadianHistoryBackend (seeding, save, mode filtering, substring search)"
```

---

### Verification

After all tasks are committed:

```bash
cargo test  # 154+ unit + 6 R integration tests pass
git log --oneline -10 | head -6
# Should show:
#   Add unit tests for RadianHistoryBackend
#   Wire RadianHistoryBackend into interactive REPL
#   Add with_arc_history() and mode_arc shared state
#   Implement reedline History trait for RadianHistoryBackend
#   Add RadianHistoryBackend struct and constructor
#   Add History::entries() accessor
```
