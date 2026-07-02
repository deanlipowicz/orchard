use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn evaluates_r_expression() {
    if std::env::var_os("ORCHARD_TEST_R").is_none() {
        return;
    }

    let output = run_radian(&["-q"], b"1 + 1\nq(\"no\")\n");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[1] 2"), "{stdout}");
}

#[test]
fn sources_profile_and_reads_option() {
    if std::env::var_os("ORCHARD_TEST_R").is_none() {
        return;
    }

    let profile = temp_file("profile.R");
    fs::write(&profile, "options(radian.test.value = 42L)\n").unwrap();

    let output = run_radian(
        &["-q", "--profile", profile.to_str().unwrap()],
        b"getOption(\"radian.test.value\")\nq(\"no\")\n",
    );
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[1] 42"), "{stdout}");
}

#[test]
fn captures_r_stderr_formatted() {
    if std::env::var_os("ORCHARD_TEST_R").is_none() {
        return;
    }

    let output = run_radian(&["-q"], b"message(\"hello stderr\")\nq(\"no\")\n");
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // stderr_format wraps content in red ANSI: \x1b[31m{}\x1b[0m
    assert!(stderr.contains("hello stderr"), "{stderr}");
}

#[test]
fn captures_r_stdout_via_cat() {
    if std::env::var_os("ORCHARD_TEST_R").is_none() {
        return;
    }

    let output = run_radian(&["-q"], b"cat(\"hello stdout\\n\")\nq(\"no\")\n");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello stdout"), "{stdout}");
}

#[test]
fn waits_for_multiline_r_input() {
    if std::env::var_os("ORCHARD_TEST_R").is_none() {
        return;
    }

    let output = run_radian(&["-q"], b"1 +\n1\nq(\"no\")\n");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[1] 2"), "{stdout}");
}

#[test]
fn r_completion_returns_base_function_and_installed_package() {
    if std::env::var_os("ORCHARD_TEST_R").is_none() {
        return;
    }

    let output = run_radian(
        &["-q"],
        br#"utils:::.assignLinebuffer("mea")
utils:::.assignEnd(3)
invisible(utils:::.guessTokenFromLine())
utils:::.completeToken()
cat(any(utils:::.retrieveCompletions() == "mean"), "\n")
cat(any(.packages(all.available = TRUE) == "base"), "\n")
q("no")
"#,
    );
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.matches("TRUE").count(), 2, "{stdout}");
}

#[cfg(unix)]
#[test]
#[ignore = "manual SIGINT acceptance is environment-sensitive"]
fn sigint_interrupts_running_r_expression() {
    if std::env::var_os("ORCHARD_TEST_R").is_none() {
        return;
    }

    let mut child = Command::new(env!("CARGO_BIN_EXE_orchard"))
        .arg("-q")
        .env_remove("ORCHARD_TEST_R")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"Sys.sleep(100)\n").unwrap();
    thread::sleep(Duration::from_millis(300));
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
    thread::sleep(Duration::from_millis(300));
    let _ = stdin.write_all(b"q(\"no\")\n");
    drop(stdin);

    for _ in 0..50 {
        if child.try_wait().unwrap().is_some() {
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success());
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(combined.to_lowercase().contains("interrupt"), "{combined}");
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }

    let _ = child.kill();
    panic!("orchard did not exit after SIGINT");
}

fn run_radian(args: &[&str], stdin: &[u8]) -> std::process::Output {
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

fn temp_file(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    std::env::temp_dir().join(format!("orchard-{millis}-{name}"))
}
