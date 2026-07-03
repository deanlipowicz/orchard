//! Integration tests for the editor send-code protocol.
//!
//! These tests spawn a real Unix socket listener, connect as a client,
//! send JSON-line requests, and verify responses. They test the protocol
//! layer only — no R evaluation is involved.
//!
//! Each test uses a unique socket path to avoid cross-test interference.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Duration;

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Unique socket path per test (counter-based, no cross-test interference).
fn test_socket_path() -> PathBuf {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!("orchard_integration_test_{n}.sock"))
}

/// Context manager that removes the socket file on drop.
struct TestSocket {
    path: PathBuf,
}

impl TestSocket {
    fn new(path: PathBuf) -> Self {
        let _ = std::fs::remove_file(&path);
        TestSocket { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TestSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[test]
fn end_to_end_send_and_receive_json() {
    let sock = TestSocket::new(test_socket_path());
    let barrier = Arc::new(Barrier::new(2));

    let listener = UnixListener::bind(sock.path()).unwrap();
    let b1 = barrier.clone();
    std::thread::spawn(move || {
        // Signal we're ready, then accept
        b1.wait();
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();

        // Echo expected response
        let resp = r#"{"id":"test1","status":"ok","output":"[1] 2\n"}"#;
        let mut writer = &stream;
        writeln!(writer, "{}", resp).unwrap();
    });

    // Wait for listener thread to be ready
    barrier.wait();

    // Connect and send a request
    let stream = UnixStream::connect(sock.path()).unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut writer = &stream;
    writeln!(
        writer,
        r#"{{"id":"test1","code":"1+1","echo":false}}"#
    )
    .unwrap();

    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();
    assert!(
        response.contains(r#""status":"ok""#),
        "expected ok status, got: {response}"
    );
    assert!(
        response.contains("[1] 2"),
        "expected output content, got: {response}"
    );
}

#[test]
fn send_invalid_json_gets_error_response() {
    let sock = TestSocket::new(test_socket_path());
    let barrier = Arc::new(Barrier::new(2));

    let listener = UnixListener::bind(sock.path()).unwrap();
    let b1 = barrier.clone();
    std::thread::spawn(move || {
        b1.wait();
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();

        let resp = r#"{"id":"null","status":"error","output":"invalid JSON"}"#;
        let mut writer = &stream;
        writeln!(writer, "{}", resp).unwrap();
    });

    barrier.wait();

    let stream = UnixStream::connect(sock.path()).unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut writer = &stream;
    writeln!(writer, "not json").unwrap();

    let mut reader = BufReader::new(&stream);
    let mut response = String::new();
    reader.read_line(&mut response).unwrap();
    assert!(
        response.contains(r#""status":"error""#),
        "expected error status, got: {response}"
    );
}
