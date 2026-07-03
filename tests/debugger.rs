//! Integration tests for debugger magic handlers.
//!
//! These tests spawn orchard as a child process with a real R installation
//! and exercise debugger workflows via stdin/stdout. Requires
//! `ORCHARD_TEST_R=1` (same gating as `tests/embedded_r.rs`).

use std::{
    io::Write,
    process::{Command, Stdio},
};

fn r_test_enabled() -> bool {
    std::env::var_os("ORCHARD_TEST_R").is_some()
}

macro_rules! r_test {
    ($name:ident, $body:block) => {
        #[test]
        #[ignore = "requires ORCHARD_TEST_R=1 env var and a working R installation"]
        fn $name() {
            if !r_test_enabled() {
                return;
            }
            $body
        }
    };
}

fn run_orchard(args: &[&str], stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_orchard"))
        .args(args)
        .env_remove("ORCHARD_TEST_R")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child.stdin.as_mut().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

r_test!(xmode_sets_verbosity, {
    let stdin = b"%xmode verbose\ngetOption(\"error\")\nq(\"no\")\n";
    let output = run_orchard(&["-q"], stdin);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Xmode set to 'verbose'"),
        "expected xmode confirmation in: {stdout}"
    );
});

r_test!(xmode_rejects_invalid, {
    let stdin = b"%xmode bogus\nq(\"no\")\n";
    let output = run_orchard(&["-q"], stdin);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Invalid xmode") || stderr.contains("Valid modes"),
        "expected invalid xmode error in: {stderr}"
    );
});

r_test!(tb_shows_traceback_after_error, {
    // Cause an error inside tryCatch (so R doesn't abort), then print traceback.
    let stdin = b"tryCatch(stop(\"inner error\"), error = function(e) {})\n%tb\nq(\"no\")\n";
    let output = run_orchard(&["-q"], stdin);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("traceback") || stdout.contains("stop"),
        "expected traceback output in: {stdout}"
    );
});
