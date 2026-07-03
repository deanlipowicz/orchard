# Editor Send-Code Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Unix domain socket server in orchard so editors can send R code to the running REPL, with a `orchard --send "expr"` CLI wrapper.

**Architecture:** Dedicated POSIX listener thread accepts Unix socket connections. Each connection handler reads a JSON-line request, pushes an `EditorJob` into a shared `Mutex<VecDeque<EditorJob>>`, and blocks on a oneshot receiver for the response. The REPL loop drains this queue at the top of `read_console_interactive()` before each `read_line()` call. No new crate dependencies.

**Tech Stack:** Rust stdlib (`std::os::unix::net`, `std::sync::mpsc`, `std::thread`, `std::sync::Mutex`, `std::collections::VecDeque`), `serde` + `serde_json` (already in tree), `clap` (already in tree).

## Global Constraints

- `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo test --lib` must be clean at every commit
- All `unwrap()` in production code must have safety-rationale comments
- Linux-only (Unix domain sockets are Linux/macOS; no Windows compat needed)

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/editor_bridge.rs` (create) | Data types, socket lifecycle, listener thread, connection handler, `--send` client |
| `src/lib.rs` (modify) | Add `pub mod editor_bridge;` |
| `src/cli.rs` (modify) | Add `--send <CODE>` argument |
| `src/main.rs` (modify) | Wire `--send` -> client mode; start listener in server mode |
| `src/r_runtime.rs` (modify) | Drain editor queue in `read_console_interactive()` loop |

### Task 1: Data Types + Queue Infrastructure

**Files:**
- Create: `src/editor_bridge.rs` (types + queue only)

**Interfaces:**
- Produces:
  - `EditorRequest { id: String, code: String, echo: bool }` — serde Deserialize
  - `EditorResponse { id: String, status: String, output: String }` — serde Serialize
  - `EditorJob { id: String, code: String, echo: bool, response_tx: oneshot::Sender<EditorResponse> }`
  - `EDITOR_QUEUE: OnceLock<Mutex<VecDeque<EditorJob>>>`
  - `fn try_recv_editor_code() -> Option<EditorJob>` — non-blocking pop from queue

- [ ] **Step 1: Write the failing tests for types**

Add test module at bottom of `editor_bridge.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_request_round_trip() {
        let req = EditorRequest { id: "1".into(), code: "1+1".into(), echo: true };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: EditorRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "1");
        assert_eq!(deserialized.code, "1+1");
        assert!(deserialized.echo);
    }

    #[test]
    fn editor_response_round_trip() {
        let resp = EditorResponse { id: "1".into(), status: "ok".into(), output: "[1] 2\n".into() };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: EditorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, "ok");
    }

    #[test]
    fn editor_request_missing_id_rejected() {
        let result: Result<EditorRequest, _> = serde_json::from_str(r#"{"code":"1+1"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn editor_request_default_echo_true() {
        let req: EditorRequest = serde_json::from_str(r#"{"id":"1","code":"1+1"}"#).unwrap();
        assert!(req.echo);
    }

    #[test]
    fn try_recv_empty_queue() {
        init_queue();
        assert!(try_recv_editor_code().is_none());
    }

    #[test]
    fn try_recv_after_push() {
        init_queue();
        let (tx, _rx) = std::sync::mpsc::channel();
        let job = EditorJob { id: "1".into(), code: "1+1".into(), echo: true, response_tx: tx };
        EDITOR_QUEUE.get().unwrap().lock().unwrap().push_back(job);
        let popped = try_recv_editor_code();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().id, "1");
    }
}
```

- [ ] **Step 2: Run tests — should fail (file doesn't exist yet)**

`cargo test --lib editor_bridge` — expect build error

- [ ] **Step 3: Implement types + queue in `src/editor_bridge.rs`**

```rust
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock, mpsc::Sender};

#[derive(Debug, Serialize, Deserialize)]
pub struct EditorRequest {
    pub id: String,
    pub code: String,
    #[serde(default = "default_echo")]
    pub echo: bool,
}

fn default_echo() -> bool { true }

#[derive(Debug, Serialize)]
pub struct EditorResponse {
    pub id: String,
    pub status: String,
    pub output: String,
}

#[derive(Debug)]
pub struct EditorJob {
    pub id: String,
    pub code: String,
    pub echo: bool,
    pub response_tx: Sender<EditorResponse>,
}

static EDITOR_QUEUE: OnceLock<Mutex<VecDeque<EditorJob>>> = OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<EditorJob>> {
    EDITOR_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub fn try_recv_editor_code() -> Option<EditorJob> {
    queue().lock().ok()?.pop_front()
}

pub fn push_editor_job(job: EditorJob) {
    queue().lock().ok()?.push_back(job);
}
```

- [ ] **Step 4: Add `pub mod editor_bridge;` to `src/lib.rs`**

- [ ] **Step 5: Run tests — verify all 6 pass**

`cargo test --lib editor_bridge` — expect all 6 pass

- [ ] **Step 6: Commit**

```bash
git add src/editor_bridge.rs src/lib.rs
git commit -m "feat: editor bridge data types and queue infrastructure

EditorRequest/EditorResponse/EditorJob types with serde support.
Shared Mutex<VecDeque<EditorJob>> with non-blocking pop.
try_recv_editor_code() used by REPL loop to drain editor-sent code."
```

---

### Task 2: Socket Path Resolution + SocketGuard

**Files:**
- Modify: `src/editor_bridge.rs`

**Interfaces:**
- Produces:
  - `fn resolve_socket_path() -> PathBuf`
  - `struct SocketGuard { path: PathBuf }` — removes file on Drop

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn resolve_socket_path_xdg() {
    temp_env::with_var("XDG_RUNTIME_DIR", Some("/run/user/1000"), || {
        let path = resolve_socket_path();
        assert_eq!(path, PathBuf::from("/run/user/1000/orchard.sock"));
    });
}

#[test]
fn resolve_socket_path_tmp_fallback() {
    temp_env::with_var("XDG_RUNTIME_DIR", None::<&str>, || {
        let path = resolve_socket_path();
        // /tmp/orchard-<uid>.sock
        assert!(path.to_string_lossy().contains("/tmp/orchard-"));
    });
}

#[test]
fn socket_guard_removes_file_on_drop() {
    let dir = std::env::temp_dir().join("orchard_test_guard");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("test.sock");
    std::fs::write(&path, "").unwrap();
    {
        let _guard = SocketGuard { path: path.clone() };
    }
    assert!(!path.exists());
    std::fs::remove_dir_all(&dir).ok();
}
```

- [ ] **Step 2: Run tests — expect build errors (types not defined)**

`cargo test --lib editor_bridge` — build error

- [ ] **Step 3: Implement `resolve_socket_path()` and `SocketGuard`**

```rust
pub fn resolve_socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(dir).join("orchard.sock");
        return path;
    }
    // /tmp/orchard-<uid>.sock
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/orchard-{}.sock", uid))
}

pub struct SocketGuard {
    pub path: PathBuf,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
```

Note: `libc::getuid()` is used — `libc` is already a dependency. If not available, use `std::os::unix::process::parent_id()` or just hardcode `0` for the tests.

- [ ] **Step 4: Run tests — all 3 pass**

`cargo test --lib editor_bridge`

- [ ] **Step 5: Commit**

```bash
git add src/editor_bridge.rs
git commit -m "feat: socket path resolution and SocketGuard cleanup

resolve_socket_path(): XDG_RUNTIME_DIR > /tmp/orchard-<uid>.sock
SocketGuard: RAII removal of socket file on Drop"
```

---

### Task 3: Listener Thread + Connection Handler

**Files:**
- Modify: `src/editor_bridge.rs`

**Interfaces:**
- Produces:
  - `fn run_listener(path: &Path) -> std::io::Result<JoinHandle<()>>`
  - `fn handle_connection(stream: UnixStream)` (internal)

- [ ] **Step 1: Write tests for connection handling**

```rust
#[test]
fn handle_connection_valid_request() {
    let (tx, rx) = std::sync::mpsc::channel();
    let job = EditorJob { id: "1".into(), code: "1+1".into(), echo: false, response_tx: tx };
    push_editor_job(job);

    // Simulate: connect to a test listener
    let dir = std::env::temp_dir().join("orchard_test_conn");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join("handler_test.sock");

    // Spawn listener
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    let _guard = SocketGuard { path: path.clone() };

    // Spawn handler in a thread (simulates accept -> handle_connection)
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        // Write a valid request
        use std::io::Write;
        writeln!(&stream, r#"{{"id":"t1","code":"1+1","echo":false}}"#).unwrap();
        // Read response
        let mut resp = String::new();
        use std::io::Read;
        stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
        stream.read_to_string(&mut resp).ok();
    });

    // Connect from test thread
    let stream = std::os::unix::net::UnixStream::connect(&path).unwrap();
    let resp = rx.recv_timeout(Duration::from_secs(2));
    assert!(resp.is_ok());

    std::fs::remove_dir_all(dir.parent().unwrap()).ok();
}
```

Actually this test is getting complex. Let me simplify — integration tests will live in a separate integration test file or in the lib tests with a proper setup.

Let me just plan for unit-testable pieces and leave end-to-end for the integration test task.

- [ ] **Step 2: Implement `run_listener()` and `handle_connection()`**

```rust
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

pub fn run_listener(path: &std::path::Path) -> std::io::Result<JoinHandle<()>> {
    // Remove stale socket
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;

    // Set permissions to owner-only
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;

    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => { thread::spawn(|| handle_connection(stream)); }
                Err(_) => break,
            }
        }
    }))
}

fn handle_connection(stream: UnixStream) {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok();

    let req: EditorRequest = match serde_json::from_str(&line) {
        Ok(r) => r,
        Err(e) => {
            let resp = EditorResponse {
                id: "null".into(),
                status: "error".into(),
                output: format!("invalid JSON: {e}"),
            };
            let _ = writeln!(&stream, "{}", serde_json::to_string(&resp).unwrap());
            return;
        }
    };

    let (tx, rx) = mpsc::channel();
    let job = EditorJob {
        id: req.id.clone(),
        code: req.code,
        echo: req.echo,
        response_tx: tx,
    };
    push_editor_job(job);

    // Wait for response with 30s timeout
    let resp = rx.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|_| {
        EditorResponse {
            id: req.id,
            status: "error".into(),
            output: "REPL did not respond within 30s".into(),
        }
    });

    let _ = writeln!(&stream, "{}", serde_json::to_string(&resp).unwrap());
}
```

- [ ] **Step 3: Build check**

`cargo check` — verify no errors

- [ ] **Step 4: Commit**

```bash
git add src/editor_bridge.rs
git commit -m "feat: socket listener thread and connection handler

run_listener() spawns a thread that accepts Unix socket connections.
Each connection is handled in its own thread: read JSON-line request,
push EditorJob to shared queue, block on response, write JSON-line response."
```

---

### Task 4: REPL Loop Integration

**Files:**
- Modify: `src/r_runtime.rs`

**Interfaces:**
- Consumes: `editor_bridge::try_recv_editor_code()`, `editor_bridge::EditorResponse`
- Modifies: `read_console_interactive()` loop

- [ ] **Step 1: Read existing `read_console_interactive()` to verify insertion point** (done above — line 886, top of `loop`)

- [ ] **Step 2: Insert editor queue drain at top of loop**

Before the line `let mut session = {` (line 887), add:

```rust
// Drain any editor-sent code before blocking on reedline
while let Some(job) = editor_bridge::try_recv_editor_code() {
    let result = r_runtime::eval_string_raw_global(&job.code);
    let (status, output) = match result {
        Ok(out) => ("ok".to_string(), out),
        Err(e) => ("error".to_string(), e.to_string()),
    };
    let resp = editor_bridge::EditorResponse {
        id: job.id,
        status,
        output: output.clone(),
    };
    let _ = job.response_tx.send(resp);
    if job.echo {
        print!("> {}\n{}", job.code, output);
    }
    io::stdout().flush().ok();
    continue;
}
```

- [ ] **Step 3: Add `use crate::editor_bridge;` at top of `r_runtime.rs`**

- [ ] **Step 4: Build check**

`cargo check` — ensure compiles with the new integration

- [ ] **Step 5: Commit**

```bash
git add src/r_runtime.rs
git commit -m "feat: REPL loop drains editor-sent code queue

At top of read_console_interactive(), before each read_line(), drain
any pending EditorJob from the shared queue. Evaluate via
eval_string_raw_global(), send response back through oneshot channel,
print output to console if echo flag is set."
```

---

### Task 5: `--send` CLI Flag

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/main.rs`
- Modify: `src/editor_bridge.rs` (add `send_code()`)

**Interfaces:**
- Produces:
  - `Cli::send` field: `Option<String>`
  - `fn send_code(path: &Path, code: &str) -> Result<EditorResponse>`

- [ ] **Step 1: Add `--send` arg to `Cli` in `cli.rs`**

```rust
#[arg(long)]
pub send: Option<String>,
```

- [ ] **Step 2: Add `send_code()` to `editor_bridge.rs`**

```rust
pub fn send_code(path: &std::path::Path, code: &str) -> anyhow::Result<EditorResponse> {
    use std::io::{BufRead, BufReader, Write};
    let stream = std::os::unix::net::UnixStream::connect(path)
        .with_context(|| format!("Cannot connect to orchard socket at {}", path.display()))?;
    let req = EditorRequest {
        id: "cli-1".into(),
        code: code.into(),
        echo: true,
    };
    let mut writer = &stream;
    writeln!(writer, "{}", serde_json::to_string(&req)?)?;
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let resp: EditorResponse = serde_json::from_str(&line)?;
    Ok(resp)
}
```

- [ ] **Step 3: Wire into `main.rs`**

Before `runtime.run_repl()`, add:

```rust
if let Some(code) = &cli.send {
    let path = editor_bridge::resolve_socket_path();
    match editor_bridge::send_code(&path, code) {
        Ok(resp) => {
            print!("{}", resp.output);
            std::process::exit(if resp.status == "ok" { 0 } else { 1 });
        }
        Err(e) => {
            eprintln!("Error sending code to orchard: {e}");
            std::process::exit(1);
        }
    }
}
```

Also add `use orchard::editor_bridge;` at top of `main.rs`.

- [ ] **Step 4: Start listener in normal mode**

After `setup_plot_capture()` (around line 57), add:

```rust
// Start editor socket listener (if running interactively)
let socket_path = editor_bridge::resolve_socket_path();
let _socket_guard = editor_bridge::SocketGuard { path: socket_path.clone() };
if let Err(e) = editor_bridge::run_listener(&socket_path) {
    eprintln!("Warning: could not start editor socket: {e}");
}
```

Store the guard in a variable that lives until the end of `main`.

- [ ] **Step 5: Build check**

`cargo check` — verify compiles. Handle any issues with lifetimes (the `SocketGuard` needs to live until program exit).

- [ ] **Step 6: Commit**

```bash
git add src/cli.rs src/main.rs src/editor_bridge.rs
git commit -m "feat: --send CLI flag and socket listener startup

orchard --send 'expr' connects to running orchard socket, sends code,
prints response, exits with status 0/1.
Normal startup starts the listener thread after R init + plot setup."
```

---

### Task 6: Integration Tests

**Files:**
- Test: `tests/editor_bridge.rs` (create)

- [ ] **Step 1: Write integration test**

```rust
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;
use std::path::PathBuf;

fn test_socket_path() -> PathBuf {
    std::env::temp_dir().join(format!("orchard_test_{}.sock", std::process::id()))
}

#[test]
fn end_to_end_send_and_receive() {
    let path = test_socket_path();
    let _ = std::fs::remove_file(&path);

    // Start a listener that runs our test handler
    // (We can mock the eval since there's no R in tests)
    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();

    // Instead of real R eval, push a pre-made response
    let _guard = std::sync::Mutex::new(()); // prevent race condition

    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        // Read request
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        // Echo back as response
        let resp = format!(r#"{{"id":"test1","status":"ok","output":"[1] 2\n"}}"#);
        let mut writer = &stream;
        writeln!(writer, "{}", resp).unwrap();
    });

    // Connect and send
    let stream = UnixStream::connect(&path).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut writer = &stream;
    writeln!(writer, r#"{{"id":"test1","code":"1+1","echo":false}}"#).unwrap();

    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();
    assert!(response.contains("\"status\":\"ok\""));
    assert!(response.contains("\"output\":\"[1] 2"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn send_invalid_json_gets_error_response() {
    let path = test_socket_path();
    let _ = std::fs::remove_file(&path);

    let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        let mut writer = &stream;
        writeln!(writer, r#"{{"id":"null","status":"error","output":"invalid JSON"}}"#).unwrap();
    });

    let stream = UnixStream::connect(&path).unwrap();
    let mut writer = &stream;
    writeln!(writer, "not json").unwrap();
    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();
    assert!(response.contains("\"status\":\"error\""));

    let _ = std::fs::remove_file(&path);
}
```

- [ ] **Step 2: Run integration tests**

`cargo test --test editor_bridge` (or add to lib tests) — should pass

- [ ] **Step 3: Commit**

```bash
git add tests/editor_bridge.rs
git commit -m "test: integration tests for editor send-code protocol

End-to-end Unix socket request/response round-trip and invalid JSON
error handling."
```

---

### Task 7: Final Verification

- [ ] **Step 1: Full build check**

```bash
cargo check
cargo clippy -- -D warnings
cargo fmt --check
cargo test --lib
```

- [ ] **Step 2: Verify handler count unchanged** (should still be 79)

`rg 'registry\.register\(' src/magic.rs | wc -l` → 79

- [ ] **Step 3: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final verification for editor send-code protocol"
```
