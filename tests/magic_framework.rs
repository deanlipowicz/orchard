use orchard::magic::{self, MagicLine, Output};

#[test]
fn registry_starts_with_expected_handlers() {
    let registry = magic::magic_registry().lock().unwrap();
    // Verify handlers from register_all() are present
    assert!(registry.get("pwd").is_some(), "pwd should be registered");
    assert!(registry.get("env").is_some(), "env should be registered");
    assert!(registry.get("alias").is_some(), "alias should be registered");
    assert!(registry.get("unalias").is_some(), "unalias should be registered");
    assert!(registry.get("edit").is_some(), "edit should be registered");
    assert!(registry.get("debug").is_some(), "debug should be registered");
    assert!(registry.get("pdb").is_some(), "pdb should be registered");
    assert!(registry.get("tb").is_some(), "tb should be registered");
    assert!(registry.get("pinfo").is_some(), "pinfo should be registered");
    assert!(registry.get("pinfo2").is_some(), "pinfo2 should be registered");
}

#[test]
fn list_all_returns_sorted_names() {
    let registry = magic::magic_registry().lock().unwrap();
    let names = registry.list_all();
    assert!(!names.is_empty(), "should have at least one handler");
    // Verify sorted
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
fn pwd_handler_returns_cwd() {
    let registry = magic::magic_registry().lock().unwrap();
    let handler = registry.get("pwd").unwrap();
    let line = MagicLine {
        name: "pwd".into(),
        args: String::new(),
        is_cell: false,
    };
    // We just check that it doesn't error and returns text
    let result = handler.run(&line);
    assert!(result.is_ok(), "pwd should succeed");
    if let Ok(Output::Text(text)) = result {
        assert!(text.contains('/'), "pwd output should contain a path separator");
    } else {
        panic!("expected Text output from pwd");
    }
}

#[test]
fn env_handler_lists_vars() {
    let registry = magic::magic_registry().lock().unwrap();
    let handler = registry.get("env").unwrap();
    let line = MagicLine {
        name: "env".into(),
        args: String::new(),
        is_cell: false,
    };
    let result = handler.run(&line);
    assert!(result.is_ok(), "env should succeed");
    if let Ok(Output::Text(text)) = result {
        assert!(!text.is_empty(), "env output should not be empty");
    } else {
        panic!("expected Text output from env");
    }
}
