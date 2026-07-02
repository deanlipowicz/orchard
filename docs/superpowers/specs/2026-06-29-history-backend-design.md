# Milestone C — Loaded History Navigation via Custom `History` Backend

## Status

Approved. Design approach (implement reedline `History` trait wrapping radian's `History` struct) confirmed.

## Goal

Connect radian's loaded history file to reedline's interactive navigation so that Ctrl-R (reverse search), up-arrow, and down-arrow search and cycle through radian's history entries instead of reedline's empty default history. Mode filtering — Ctrl-R in R mode shows only R-mode entries; Ctrl-R in shell mode shows only shell-mode entries.

## Background

Radian's `History` struct (`src/history.rs`) reads/writes a rich file format with timestamps, mode labels (`r`/`browse`/`shell`/`unknown`), and multiline support. It is loaded at startup and stored in `ConsoleState::history`. When commands are submitted, `append_history(mode, line)` is called to persist to the file.

Reedline's `PromptSession` (`src/prompt.rs`) builds a `Reedline` instance but never calls `.with_history()`. Reedline defaults to an in-memory `FileBackedHistory` (capacity 1000) that starts empty and is lost on restart. Ctrl-R searches this empty history, never reaching radian's loaded entries.

## Architecture

```
reedline::Reedline
  │  calls history.save() / history.search()
  ▼
RadianHistoryBackend      (implements reedline::History trait)
  │                        owns Vec<HistoryItem> for search
  │                        owns Arc<Mutex<PromptMode>> (shared)
  │                        owns Arc<Mutex<History>> for file writes
  ▼
History                   (radian's struct — file I/O, rich format)
```

### Concepts

- **RadianHistoryBackend** — a struct in `src/history.rs` that implements the reedline `History` trait. It is constructed at session start from radian's loaded entries and shares mode state with `PromptSession`.
- **Single write path** — when reedline calls `save()`, the backend appends to its in-memory search index AND delegates to radian's `History::append()` for file persistence. The REPL's existing `append_history()` call is removed to prevent double-writes.
- **Mode-filtered search** — each `HistoryItem` carries its mode in `more_info`. The backend reads the current `PromptMode` from shared context at search time and filters entries to those with a compatible mode.
- **Ephemeral index** — the backend is rebuilt from radian's file on every session startup. It is a transient search index, not a durable store.

## File Changes

### `src/history.rs` — New `RadianHistoryBackend`

Add these items to the existing `history` module:

```rust
/// Implements reedline's `History` trait backed by radian's `History`
/// struct.  Provides mode-aware search and delegates file writes to
/// radian's History::append().
pub struct RadianHistoryBackend {
    /// Searchable in-memory entries.  Index in vec = HistoryItemId.
    items: Vec<HistoryItem>,
    /// Radian's history store for file writes.
    inner: Arc<Mutex<History>>,
    /// Current prompt mode, shared with PromptSession.
    mode: Arc<Mutex<PromptMode>>,
    /// Next ID to assign in save().
    next_id: usize,
}

impl RadianHistoryBackend {
    pub fn new(
        inner: Arc<Mutex<History>>,
        mode: Arc<Mutex<PromptMode>>,
    ) -> Self {
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

### `History` trait implementation (see "Trait Implementation" section below)

Add a helper module function for mode compatibility:

```rust
/// Returns true if `candidate` mode should appear in search results for
/// the `current` mode.  "r" and "browse" share the same history book.
fn mode_compatible(current: &str, candidate: &str) -> bool {
    current == candidate
        || (current == "r" && candidate == "browse")
        || (current == "browse" && candidate == "r")
}
```

### `src/prompt.rs` — `PromptSession` changes

1. **Add `with_arc_history()` constructor** that takes `settings`, `Arc<Mutex<History>>`, and `Arc<Mutex<PromptMode>>`, builds the reedline instance with `.with_history(Box::new(RadianHistoryBackend::new(...)))`.

2. **Add `mode_arc` field** to `PromptContext` — an `Arc<Mutex<PromptMode>>` alongside the existing `mode: PromptMode`. The `mode` field remains for direct access; the `mode_arc` is shared with the history backend.

3. `PromptContext::update_mode()` syncs the arc when mode changes.

### `src/r_runtime.rs` — Console state and REPL changes

1. **ConsoleState changes**: Add `history_arc: Arc<Mutex<History>>` alongside the existing `history: Option<History>`. The history is now wrapped in `Arc<Mutex<>>` so both `ConsoleState` and `RadianHistoryBackend` can share it.

2. **`read_console_interactive` changes**: When creating a new `PromptSession`, pass `session.with_arc_history(settings, history_arc, mode_arc)` instead of `PromptSession::new(settings)`.

3. **Remove `append_history()` calls** from:
   - `run_repl()` — R-mode line submission
   - `read_shell_prompt()` — shell-mode line submission
   (The backend's `save()` now triggers the file write through `History::append()`.)

4. **Keep `append_history()` calls** for non-reedline paths:
   - Piped startup input (goes through `read_console_piped`, not reedline)
   - These are infrequent and will not be in the search index for that session, which is acceptable for v1.

### Trait Implementation

### `save(&mut self, item: HistoryItem) -> Result<HistoryItem>`

1. Read current mode from `self.mode.lock().unwrap()` → convert to mode string via `PromptMode::label()`.
2. Assign item.id = Some(HistoryItemId(self.next_id)).
3. Set item.more_info = Some(mode_string.clone()).
4. Push item.clone() to self.items; increment self.next_id.
5. Call `self.inner.lock().unwrap().append(&mode_string, &item.command_line)`.
6. Return Ok(item).

### `load(&self, id: HistoryItemId) -> Result<HistoryItem>`

If `id.0 < self.items.len()`, return `Ok(self.items[id.0].clone())`. Otherwise return an empty `HistoryItem` (reedline expects `Ok` with a default item for missing IDs in some implementations — verify during implementation).

### `search(&self, query: SearchQuery) -> Result<Vec<HistoryItem>>`

1. Read current mode from `self.mode.lock().unwrap()`.
2. Determine compatible modes (see `mode_compatible()` above).
3. Filter `self.items` to entries where `more_info` matches a compatible mode.
4. Apply the query filter (SearchFilter::CommandLine with SearchType::Substring, Prefix, Exact, or Fuzzy) to `item.command_line`.
5. Sort by ID descending (most recent first).
6. Apply `query.limit` if set.
7. Return matching items.

### `count(&self, query: SearchQuery) -> Result<i64>`

Call search, return `Ok(items.len() as i64)`.

### `update(&mut self, id: HistoryItemId, updater: &dyn Fn(HistoryItem) -> HistoryItem) -> Result<()>`

If `id.0 < self.items.len()`, apply updater to `self.items[id.0]`.

### `clear(&mut self) -> Result<()>`

Clear `self.items`. Call `self.inner.lock().unwrap().clear_memory()`.

### `delete(&mut self, id: HistoryItemId) -> Result<()>`

Set `self.items[id.0].command_line = String::new()` (tombstone — maintains ID stability). Do NOT remove from the vec to avoid shifting indices.

### `sync(&mut self) -> io::Result<()>`

No-op. Radian's `History::append()` writes synchronously.

### `session(&self) -> Option<HistorySessionId>`

Returns `None`.

## Mode Compatibility

| Current mode | Included in search |
|---|---|
| `r` | `r`, `browse` (same history book) |
| `browse` | `browse`, `r` |
| `shell` | `shell` |
| `unknown` | All modes (no filtering) — when mode is unknown, show everything |

## Edge Cases

- **Multiline entries**: Radian's `History` stores multiline entries as a single entry with embedded `\n`. Reedline's `HistoryItem.command_line` also uses embedded newlines. The backend stores and returns them as-is — internal newlines inside a command will be preserved.
- **Double-write prevention**: The REPL's `append_history()` calls are removed. The only write path is `RadianHistoryBackend::save()` → `History::append()`. Non-reedline paths (piped startup, browser input) still call `append_history()` directly — these entries are not searchable via Ctrl-R in the current session, which is acceptable for v1.
- **Startup/piped commands**: Commands entered during startup or via piped stdin don't go through reedline, so they bypass the backend. They're still written to the file by the existing `append_history()` call. On the next interactive session, they'll be loaded into the backend normally.
- **History file rotation/trimming**: `History::append()` calls `trim()` after writing, which may rewrite the file. The backend's in-memory index is unaffected (it grows unboundedly during the session). On restart, the index is rebuilt from the trimmed file. Acceptable for v1.

## Testing

- **Unit test: `backend_seeded_from_radian_entries`**: Create a `History` with known entries, construct `RadianHistoryBackend`, verify all entries are searchable.
- **Unit test: `save_appends_to_radian`**: Create backend, call `save()`, verify entry appears in radian's `History` entries.
- **Unit test: `search_filters_by_current_mode`**: Create backend with mixed-mode entries, set mode to `r`, verify only `r`/`browse` entries returned. Set to `shell`, verify only shell entries.
- **Unit test: `search_filters_by_substring`**: Verify Ctrl-R substring matching works.
- **Integration test**: Run a command interactively (via `read_line`), then verify Ctrl-R (simulated via `ReedlineEvent::SearchHistory`) can find it. This requires more setup and can be deferred.

## Risks and Mitigations

| Risk | Mitigation |
|---|---|
| reedline `History` trait API changes in future versions | Vendored reedline at 0.48.0; pinned version |
| `HistoryItemId` must match index into `self.items`; removal shifts indices | Use tombstone (empty string) instead of removal |
| `HistoryItem.more_info` not used by reedline's search UI | It's ignored by reedline; only our backend reads it |
| Performance with 20k+ history entries | Search is O(n) scan; acceptable for v1 (< 1ms on typical desktop). Optimize with prefix index if needed later. |
