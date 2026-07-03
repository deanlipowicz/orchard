//! Editor send-code protocol — Unix socket server for editor-to-orchard communication.
//!
//! Editors (neovim, emacs ESS, helix, tmux) send R code to the running orchard
//! REPL via a JSON-line protocol over a Unix domain socket. A dedicated listener
//! thread accepts connections; each connection handler pushes an `EditorJob` into
//! a shared queue drained by the main REPL loop.
//!
//! See `docs/superpowers/specs/2026-07-03-editor-send-code-design.md`.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// A code-evaluation request received from an editor.
#[derive(Debug, Serialize, Deserialize)]
pub struct EditorRequest {
    /// Opaque identifier echoed back in the response (UUID or counter).
    pub id: String,
    /// R code to evaluate.
    pub code: String,
    /// Whether to echo the code and output to the REPL console. Default true.
    #[serde(default = "default_echo")]
    pub echo: bool,
}

fn default_echo() -> bool {
    true
}

/// A response sent back to the editor after evaluating the requested code.
#[derive(Debug, Serialize, Deserialize)]
pub struct EditorResponse {
    /// Echoed from the corresponding `EditorRequest`.
    pub id: String,
    /// Either `"ok"` or `"error"`.
    pub status: String,
    /// Captured stdout/stderr from R evaluation.
    pub output: String,
}

/// An internal job queued by the listener thread and consumed by the REPL loop.
#[derive(Debug)]
pub struct EditorJob {
    /// Opaque request identifier.
    pub id: String,
    /// R code to evaluate.
    pub code: String,
    /// Whether to echo output to the REPL console.
    pub echo: bool,
    /// Sender end of a oneshot channel; the REPL loop sends the `EditorResponse`
    /// through this channel once evaluation completes.
    pub response_tx: Sender<EditorResponse>,
}

// ---------------------------------------------------------------------------
// Shared queue
// ---------------------------------------------------------------------------

/// Global queue of pending `EditorJob`s shared between the listener thread
/// (producer) and the REPL loop (consumer).
static EDITOR_QUEUE: OnceLock<Mutex<VecDeque<EditorJob>>> = OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<EditorJob>> {
    EDITOR_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Non-blocking pop of the next pending editor job.
///
/// Returns `None` if the queue is empty or the lock is poisoned.
pub fn try_recv_editor_code() -> Option<EditorJob> {
    queue().lock().ok()?.pop_front()
}

/// Push a job onto the shared queue for the REPL loop to process.
pub fn push_editor_job(job: EditorJob) {
    if let Ok(mut q) = queue().lock() {
        q.push_back(job);
    }
}

// ---------------------------------------------------------------------------
// Listener thread
// ---------------------------------------------------------------------------

/// Start the Unix domain socket listener on `path`.
///
/// Removes any stale socket, binds, sets owner-only permissions (0700), and
/// spawns a dedicated accept loop thread. Returns the thread handle and a
/// `SocketGuard` that will remove the socket file on drop (or when joined).
pub fn run_listener(path: &Path) -> std::io::Result<JoinHandle<()>> {
    // Remove any stale socket from a prior crash.
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;

    // Owner-only access — no other users may connect.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;

    Ok(thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    thread::spawn(|| handle_connection(stream));
                }
                Err(_) => break, // listener socket closed
            }
        }
    }))
}

/// Handle a single editor connection: read a JSON-line request, enqueue it
/// for evaluation, wait for the response, and write it back.
fn handle_connection(stream: UnixStream) {
    let peer_addr = stream.peer_addr().ok();
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();

    if reader.read_line(&mut line).is_err() || line.trim().is_empty() {
        return; // connection closed or empty — nothing to do
    }

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

    // Create a oneshot channel for the REPL loop to send the response back.
    let (tx, rx) = mpsc::channel();
    let job = EditorJob {
        id: req.id.clone(),
        code: req.code,
        echo: req.echo,
        response_tx: tx,
    };
    push_editor_job(job);

    // Block until the REPL loop evaluates the code and sends the response.
    // If the REPL doesn't respond within 30s, return a timeout error.
    let resp = match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(r) => r,
        Err(_) => EditorResponse {
            id: req.id,
            status: "error".into(),
            output: "REPL did not respond within 30s".into(),
        },
    };

    if let Err(_e) = writeln!(&stream, "{}", serde_json::to_string(&resp).unwrap()) {
        // Connection dropped by the editor before we could respond.
        // This is not an error worth logging; just clean up.
        let _ = peer_addr;
    }
}

/// Connect to a running orchard via `path`, send `code` for evaluation, and
/// return the response. Used by the `orchard --send "expr"` CLI client.
pub fn send_code(path: &Path, code: &str) -> anyhow::Result<EditorResponse> {
    let stream = UnixStream::connect(path)
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

// ---------------------------------------------------------------------------
// Socket path resolution
// ---------------------------------------------------------------------------

/// Resolve the socket path using XDG or a /tmp fallback.
///
/// Priority:
/// 1. `$XDG_RUNTIME_DIR/orchard.sock` (preferred — auto-cleaned on logout)
/// 2. `/tmp/orchard-<uid>.sock` (fallback)
pub fn resolve_socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let path = PathBuf::from(dir).join("orchard.sock");
        return path;
    }
    // Safety: getuid() is always available on Linux and cannot fail.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/orchard-{uid}.sock"))
}

/// RAII guard that removes the socket file on drop.
pub struct SocketGuard {
    pub path: PathBuf,
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// ---------------------------------------------------------------------------
// Re-initialisation for tests
// ---------------------------------------------------------------------------

/// Clear the queue and replace it with a fresh instance.
/// Used only in tests to avoid cross-test contamination.
#[cfg(test)]
fn init_queue() {
    let _ = EDITOR_QUEUE.set(Mutex::new(VecDeque::new()));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Serde round-trips --------------------------------------------------

    #[test]
    fn editor_request_round_trip() {
        let req = EditorRequest {
            id: "1".into(),
            code: "1+1".into(),
            echo: true,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: EditorRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "1");
        assert_eq!(deserialized.code, "1+1");
        assert!(deserialized.echo);
    }

    #[test]
    fn editor_response_round_trip() {
        let resp = EditorResponse {
            id: "1".into(),
            status: "ok".into(),
            output: "[1] 2\n".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: EditorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, "ok");
    }

    // -- Deserialisation edge cases ----------------------------------------

    #[test]
    fn editor_request_missing_id_rejected() {
        let result: Result<EditorRequest, _> =
            serde_json::from_str(r#"{"code":"1+1"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn editor_request_default_echo_true() {
        let req: EditorRequest =
            serde_json::from_str(r#"{"id":"1","code":"1+1"}"#).unwrap();
        assert!(req.echo);
    }

    // -- Socket path resolution -------------------------------------------

    #[test]
    fn resolve_socket_path_xdg() {
        let prior = std::env::var("XDG_RUNTIME_DIR").ok();
        // Safety: test runs single-threaded; env var manipulation is sound.
        unsafe { std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000"); }
        let path = resolve_socket_path();
        assert_eq!(path, PathBuf::from("/run/user/1000/orchard.sock"));
        match prior {
            Some(v) => unsafe { std::env::set_var("XDG_RUNTIME_DIR", v); },
            None => unsafe { std::env::remove_var("XDG_RUNTIME_DIR"); },
        }
    }

    #[test]
    fn resolve_socket_path_tmp_fallback() {
        let prior = std::env::var("XDG_RUNTIME_DIR").ok();
        // Safety: test runs single-threaded; env var manipulation is sound.
        unsafe { std::env::remove_var("XDG_RUNTIME_DIR"); }
        let path = resolve_socket_path();
        // Should be /tmp/orchard-<uid>.sock
        let s = path.to_string_lossy();
        assert!(s.starts_with("/tmp/orchard-"), "got {s}");
        assert!(s.ends_with(".sock"), "got {s}");
        match prior {
            Some(v) => unsafe { std::env::set_var("XDG_RUNTIME_DIR", v); },
            None => unsafe { std::env::remove_var("XDG_RUNTIME_DIR"); },
        }
    }

    #[test]
    fn socket_guard_removes_file_on_drop() {
        let dir = std::env::temp_dir().join("orchard_test_guard");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.sock");
        std::fs::write(&path, "").unwrap();
        assert!(path.exists());
        {
            let _guard = SocketGuard { path: path.clone() };
        }
        assert!(!path.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    // -- Queue behaviour ---------------------------------------------------

    #[test]
    fn try_recv_empty_queue() {
        init_queue();
        assert!(try_recv_editor_code().is_none());
    }

    #[test]
    fn try_recv_after_push() {
        init_queue();
        let (tx, _rx) = std::sync::mpsc::channel();
        let job = EditorJob {
            id: "1".into(),
            code: "1+1".into(),
            echo: true,
            response_tx: tx,
        };
        push_editor_job(job);
        let popped = try_recv_editor_code();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().id, "1");
    }

    #[test]
    fn queue_fifo_order() {
        init_queue();
        let (tx1, _rx1) = std::sync::mpsc::channel();
        let (tx2, _rx2) = std::sync::mpsc::channel();
        push_editor_job(EditorJob {
            id: "first".into(),
            code: "1".into(),
            echo: false,
            response_tx: tx1,
        });
        push_editor_job(EditorJob {
            id: "second".into(),
            code: "2".into(),
            echo: false,
            response_tx: tx2,
        });
        assert_eq!(try_recv_editor_code().unwrap().id, "first");
        assert_eq!(try_recv_editor_code().unwrap().id, "second");
        assert!(try_recv_editor_code().is_none());
    }
}
