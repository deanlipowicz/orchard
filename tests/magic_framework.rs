use orchard::magic::{self, MagicLine, Output};

#[test]
fn list_all_returns_sorted_names() {
    let registry = magic::magic_registry().lock().unwrap();
    let names = registry.list_all();
    assert!(!names.is_empty(), "should have at least one handler");
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "list_all should return sorted names");
}

#[test]
fn get_returns_handler_by_name() {
    let registry = magic::magic_registry().lock().unwrap();
    let handler = registry.get("pwd").expect("pwd handler should exist");
    assert_eq!(handler.name(), "pwd");
    assert!(!handler.description().is_empty());
}

#[test]
fn dispatch_unknown_returns_error() {
    let registry = magic::magic_registry().lock().unwrap();
    let result = registry.get("nonexistent");
    assert!(result.is_none(), "unknown magic should return None");
}

#[test]
fn pwd_handler_returns_exact_cwd() {
    let registry = magic::magic_registry().lock().unwrap();
    let handler = registry.get("pwd").unwrap();
    let line = MagicLine {
        name: "pwd".into(),
        args: String::new(),
        is_cell: false,
    };
    let result = handler.run(&line);
    assert!(result.is_ok(), "pwd should succeed");
    if let Ok(Output::Text(text)) = result {
        let expected = std::env::current_dir().unwrap();
        let expected_str = format!("{}\n", expected.display());
        assert_eq!(text, expected_str, "pwd output should exactly match cwd");
    } else {
        panic!("expected Text output from pwd");
    }
}

#[test]
fn env_handler_lists_vars_in_sorted_order() {
    let registry = magic::magic_registry().lock().unwrap();
    let handler = registry.get("env").unwrap();
    // Set a known env var so we can verify it appears in the output
    let _guard = EnvGuard::set("ORCHARD_TEST_ENV_VAR", "test_value_123");
    let line = MagicLine {
        name: "env".into(),
        args: String::new(),
        is_cell: false,
    };
    let result = handler.run(&line);
    assert!(result.is_ok(), "env should succeed");
    if let Ok(Output::Text(text)) = result {
        assert!(
            text.contains("ORCHARD_TEST_ENV_VAR=test_value_123"),
            "env output should contain the set variable: {text}"
        );
        // Verify sorted: extract keys and check they are sorted
        let keys: Vec<&str> = text
            .lines()
            .filter_map(|l| l.split_once('=').map(|(k, _)| k))
            .collect();
        let mut sorted_keys = keys.clone();
        sorted_keys.sort();
        assert_eq!(keys, sorted_keys, "env vars should be sorted");
    } else {
        panic!("expected Text output from env");
    }
}

#[test]
fn env_handler_gets_specific_var() {
    let registry = magic::magic_registry().lock().unwrap();
    let handler = registry.get("env").unwrap();
    let _guard = EnvGuard::set("ORCHARD_TEST_GET_VAR", "get_value_456");
    let line = MagicLine {
        name: "env".into(),
        args: "ORCHARD_TEST_GET_VAR".into(),
        is_cell: false,
    };
    let result = handler.run(&line);
    assert!(result.is_ok());
    if let Ok(Output::Text(text)) = result {
        assert_eq!(text, "ORCHARD_TEST_GET_VAR=get_value_456\n");
    } else {
        panic!("expected Text output from env get");
    }
}

#[test]
fn env_handler_reports_unset_var() {
    let registry = magic::magic_registry().lock().unwrap();
    let handler = registry.get("env").unwrap();
    let _guard = EnvGuard::remove("ORCHARD_TEST_UNSET_VAR");
    let line = MagicLine {
        name: "env".into(),
        args: "ORCHARD_TEST_UNSET_VAR".into(),
        is_cell: false,
    };
    let result = handler.run(&line);
    assert!(result.is_ok());
    if let Ok(Output::Text(text)) = result {
        assert_eq!(text, "ORCHARD_TEST_UNSET_VAR: (not set)\n");
    } else {
        panic!("expected Text output from env for unset var");
    }
}

struct EnvGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let original = std::env::var_os(key);
        // Safety: single-threaded test, no concurrent env access.
        unsafe { std::env::set_var(key, value) };
        Self { key, original }
    }

    fn remove(key: &'static str) -> Self {
        let original = std::env::var_os(key);
        unsafe { std::env::remove_var(key) };
        Self { key, original }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(val) => unsafe { std::env::set_var(self.key, val) },
            None => unsafe { std::env::remove_var(self.key) },
        }
    }
}
