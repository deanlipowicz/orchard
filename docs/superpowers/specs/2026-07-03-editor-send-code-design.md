# Editor Send-Code Protocol — Design Spec

**Status:** Approved | **Milestone:** v0.7 | **Effort:** 4h
**Driver:** The single most impactful feature for terminal+editor IDE workflow.

## Overview

A Unix domain socket server embedded in orchard that lets text editors (neovim
iron.nvim, vim-slime, emacs ESS, helix, tmux) send R code to the running REPL
for evaluation. Combined with a `orchard --send "expr"` CLI wrapper for easy
editor integration.

## Architecture

```
orchard process
├── Main thread: REPL loop (read_console_interactive)
│   └── On each iteration, drains editor code queue → eval → respond
├── Listener thread: UnixListener accept loop
│   └── Per-connection handler thread
│       ├── Read JSON-line → oneshot channel → REPL queue
│       └── Block on response → write JSON-line → close
└── CLI client (separate process)
    └── orchard --send "expr" → connect to socket → JSON exchange → exit
```

### New module: `src/editor_bridge.rs`

Contains:
- `EditorRequest` / `EditorResponse` structs (serde Serialize/Deserialize)
- `EditorJob` — internal struct holding `(code, echo, response_tx)` where
  `response_tx: oneshot::Sender<EditorResponse>`
- `EDITOR_QUEUE` — a `OnceLock<Mutex<VecDeque<EditorJob>>>` shared between the
  listener thread and the REPL loop. The listener pushes jobs, the REPL loop
  drains them.
- `run_listener()` — spawns the listener thread, returns handle
- `try_recv_editor_code()` — non-blocking pop from `EDITOR_QUEUE`, called by
  REPL loop
- `handle_connection()` — per-connection protocol handler
- `send_code()` — client-side function used by `--send` CLI
- `resolve_socket_path()` — path selection logic
- `SocketGuard` — RAII cleanup of socket file

## Socket Lifecycle

**Path resolution (first-existing wins):**
1. `$XDG_RUNTIME_DIR/orchard.sock` (preferred — automatic cleanup on logout)
2. `/tmp/orchard-<uid>.sock` (fallback — no XDG_RUNTIME_DIR)
3. `~/.local/share/orchard/orchard.sock` (final fallback)

**Startup sequence (in `main.rs`, after R initialization):**
1. Call `resolve_socket_path()` to determine path
2. Remove any stale socket file at that path
3. `UnixListener::bind(path)`
4. `chmod 0700` on socket file (owner-only access)
5. Create `SocketGuard` that removes the file on `Drop`
6. Spawn listener thread with `std::thread::spawn`

**Crash recovery:** Stale socket removed at startup (step 2). `SocketGuard`
ensures cleanup on normal exit. SIGKILL leaves stale socket — cleaned on next
startup.

## JSON Message Format

One JSON object per line, `\n` delimited. All fields UTF-8.

### Request (editor → orchard)

```json
{"id": "uuid-or-counter", "code": "summary(mtcars)", "echo": true}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `id` | string | yes | — | Echoed in response for request-response matching |
| `code` | string | yes | — | R code to evaluate via `eval_string_raw_global()` |
| `echo` | bool | no | `true` | If true, print code + output to REPL console |

### Response (orchard → editor)

```json
{"id": "uuid-or-counter", "status": "ok", "output": "> summary(mtcars)\n      mpg        ...\n"}
```

```json
{"id": "uuid-or-counter", "status": "error", "output": "Error: object 'x' not found\n"}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Echoed from request |
| `status` | string | yes | `"ok"` or `"error"` |
| `output` | string | yes | Captured stdout/stderr from R evaluation |

## REPL Loop Integration

In `r_runtime.rs::read_console_interactive()`, at the top of the main loop
before `session.read_line()`:

```rust
loop {
    // Drain any editor-sent code — evaluate silently, respond, loop back
    while let Some(req) = try_recv_editor_code() {
        let result = eval_string_raw_global(&req.code);
        let status = if result.is_ok() { "ok" } else { "error" };
        let output = result.unwrap_or_else(|e| e.to_string());
        let _ = req.response_tx.send(EditorResponse { id: req.id, status, output });
        if req.echo { print!("> {}\n{}", req.code, output); }
        io::stdout().flush().ok();
        continue;  // skip reedline — recheck for more messages
    }
    // ... session.read_line(), shell/?/magic/R dispatch ...
}
```

**Rationale:** Evaluating at the top of the loop means editor code runs while R
is idle (waiting for the next prompt). No need to interrupt reedline. The
`continue` prevents `read_line()` from blocking while there are pending
messages.

**Thread safety:** All R FFI calls occur on the main thread. The listener
thread only handles I/O and channel communication.

## `orchard --send` CLI

New flag in `src/cli.rs`:

```
orchard --send "summary(mtcars)"
```

Implementation:
1. Parse `--send <CODE>` argument
2. Connect to socket at resolved path
3. Write JSON request: `{"id":"cli-1","code":"...","echo":true}\n`
4. Read JSON response line
5. Print output to stdout
6. Exit 0 on success, 1 on error

Reuses `send_code()` from `editor_bridge.rs` — the client half of the protocol.

## Error Handling & Security

| Concern | Approach |
|---------|----------|
| Socket file cleanup on crash | Remove stale socket at startup. `SocketGuard` removes on `Drop`. |
| Access control | `chmod 0700` after bind — same Unix user only. |
| Malformed JSON | Return error response, close connection. |
| Missing fields | Serde `#[derive(Deserialize)]` with validation — reject missing `id`/`code`. |
| Large payloads | Max 1MB per connection — reject larger. |
| Slow REPL / channel full | 30s timeout on oneshot receiver — connection drops if REPL doesn't respond. |
| R evaluation error | Captured as `"error"` status in JSON response. |

## Dependencies

No new crates. Reuses existing:
- `serde` + `serde_json` — JSON serialization (already in `Cargo.toml`)
- `std::os::unix::net::{UnixListener, UnixStream}` — socket I/O
- `std::sync::mpsc` — oneshot channels for request-response pairing
- `std::thread` — listener and per-connection threads
- `clap` — `--send` flag parsing (already in `Cargo.toml`)

## Testing Strategy

| Level | Tests | Count |
|-------|-------|-------|
| Unit | `EditorRequest` / `EditorResponse` serde round-trips | 4 |
| Unit | `resolve_socket_path()` with various env states | 4 |
| Unit | `SocketGuard` cleanup on Drop | 1 |
| Integration | End-to-end: spawn listener, connect, send JSON, read response | 2 |
| Integration | `orchard --send` via CLI against test listener | 1 |
| Integration | Error cases: invalid JSON, missing fields, connection refused | 3 |

## Future Considerations (Out of Scope for v0.7)

- **Editor plugins:** iron.nvim, vim-slime, ESS integrations are separate
  projects — not built into orchard
- **TLS encryption:** Unnecessary for local Unix socket (kernel enforces
  same-user access)
- **Multiple concurrent editor connections:** Already supported (thread-per-connection)
- **Async response streaming:** Not needed for MVP — single response per request
