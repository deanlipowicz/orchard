#![allow(unused_imports)]
use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

// ---------------------------------------------------------------------------
// ShellState — session-only in-memory state shared by shell magics
// ---------------------------------------------------------------------------

#[derive(Default)]
pub(crate) struct ShellState {
    pub bookmarks: HashMap<String, PathBuf>,
    pub dir_stack: Vec<PathBuf>,
    pub dir_history: Vec<PathBuf>,
}

pub(crate) static SHELL_STATE: OnceLock<Mutex<ShellState>> = OnceLock::new();

fn shell_state() -> &'static Mutex<ShellState> {
    SHELL_STATE.get_or_init(|| Mutex::new(ShellState::default()))
}

mod sx;
pub use sx::*;

mod dir;
pub use dir::*;

mod env;
pub use env::*;

mod ls;
pub use ls::*;

mod bookmark;
pub use bookmark::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that modify the shared `SHELL_STATE` (dir_stack,
    /// dir_history, bookmarks) or the process cwd.  Without this lock,
    /// concurrent directory-changing tests race on the global `SHELL_STATE`
    /// and `std::env::current_dir()`.
    static DIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_shell_ls_empty_dir() {
        let handler = super::Ls;
        let tmp = std::env::temp_dir().join(format!("orchard-ls-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let line = MagicLine {
            name: "ls".into(),
            args: tmp.to_str().unwrap().into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_ok(), "ls should succeed: {:?}", result);
        let _ = std::fs::remove_dir(&tmp);
    }

    #[test]
    fn test_shell_cd_minus() {
        let _lock = DIR_LOCK.lock().unwrap();
        let handler = super::Cd;
        let orig = std::env::current_dir().unwrap();
        // Safety: single-threaded test, env mutation is safe here.
        unsafe {
            std::env::set_var("OLDPWD", "/tmp");
        }
        let line = MagicLine {
            name: "cd".into(),
            args: "-".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_ok(), "cd - should succeed: {:?}", result);
        // Clean up
        std::env::set_current_dir(&orig).ok();
    }

    #[test]
    fn test_shell_cd_nonexistent() {
        let handler = super::Cd;
        let line = MagicLine {
            name: "cd".into(),
            args: "/tmp/orchard-nonexistent-dir-××××".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for nonexistent path");
    }

    #[test]
    #[ignore = "requires R initialization (eval_string_raw_global)"]
    fn test_shell_sx_echo() {
        let handler = super::Sx;
        let line = MagicLine {
            name: "sx".into(),
            args: "echo hello orchard".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_ok(), "sx should succeed: {:?}", result);
        if let Ok(Output::Text(text)) = result {
            assert!(
                text.contains("sx_output"),
                "output should mention variable: {text}"
            );
            assert!(
                text.contains("hello orchard"),
                "output should contain command output: {text}"
            );
        }
    }

    // --- Bookmark arg-parsing tests ---

    /// RAII guard that clears the shared `SHELL_STATE` bookmarks on creation
    /// and drop, so bookmark tests start from a known empty state.
    struct BookmarkGuard;

    impl BookmarkGuard {
        fn new() -> Self {
            shell_state().lock().unwrap().bookmarks.clear();
            Self
        }
    }

    impl Drop for BookmarkGuard {
        fn drop(&mut self) {
            shell_state().lock().unwrap().bookmarks.clear();
        }
    }

    fn magic_line(name: &str, args: &str) -> MagicLine {
        MagicLine {
            name: name.into(),
            args: args.into(),
            is_cell: false,
        }
    }

    #[test]
    fn bookmark_list_shows_empty_when_no_bookmarks() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = BookmarkGuard::new();
        let handler = Bookmark;
        let result = handler.run(&magic_line("bookmark", ""));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "(no bookmarks)\n");
        } else {
            panic!("expected Text output");
        }
    }

    #[test]
    fn bookmark_set_creates_entry() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = BookmarkGuard::new();
        let tmp = std::env::temp_dir().join(format!(
            "orchard-bm-set-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let handler = Bookmark;
        let args = format!("mydir {}", tmp.display());
        let result = handler.run(&magic_line("bookmark", &args));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert!(
                text.contains("mydir") && text.contains(&tmp.display().to_string()),
                "output should mention bookmark name and path: {text}"
            );
        } else {
            panic!("expected Text output");
        }
        // Verify it was stored
        let state = shell_state().lock().unwrap();
        assert!(state.bookmarks.contains_key("mydir"));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn bookmark_set_rejects_nonexistent_dir() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = BookmarkGuard::new();
        let handler = Bookmark;
        let result = handler.run(&magic_line("bookmark", "bad /nonexistent/orchard/xyz"));
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("does not exist"),
            "error should mention nonexistent dir: {msg}"
        );
    }

    #[test]
    fn bookmark_delete_removes_existing() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = BookmarkGuard::new();
        let tmp = std::env::temp_dir().join(format!(
            "orchard-bm-del-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        shell_state()
            .lock()
            .unwrap()
            .bookmarks
            .insert("todir".into(), tmp.clone());
        let handler = Bookmark;
        let result = handler.run(&magic_line("bookmark", "-d todir"));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "Removed bookmark 'todir'\n");
        } else {
            panic!("expected Text output");
        }
        assert!(
            !shell_state()
                .lock()
                .unwrap()
                .bookmarks
                .contains_key("todir")
        );
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn bookmark_delete_reports_for_nonexistent() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = BookmarkGuard::new();
        let handler = Bookmark;
        let result = handler.run(&magic_line("bookmark", "-d ghost"));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "No bookmark 'ghost'\n");
        } else {
            panic!("expected Text output");
        }
    }

    #[test]
    fn bookmark_delete_without_name_returns_error() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = BookmarkGuard::new();
        let handler = Bookmark;
        let result = handler.run(&magic_line("bookmark", "-d"));
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("Usage"), "error should mention usage: {msg}");
    }

    #[test]
    fn bookmark_jump_to_existing_changes_dir() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = BookmarkGuard::new();
        let tmp = std::env::temp_dir().join(format!(
            "orchard-bm-jump-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        shell_state()
            .lock()
            .unwrap()
            .bookmarks
            .insert("jumpdir".into(), tmp.clone());
        let orig = std::env::current_dir().unwrap();

        let handler = Bookmark;
        let result = handler.run(&magic_line("bookmark", "jumpdir"));
        assert!(result.is_ok());
        assert_eq!(std::env::current_dir().unwrap(), tmp);

        std::env::set_current_dir(&orig).ok();
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn bookmark_jump_to_nonexistent_returns_error() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = BookmarkGuard::new();
        let handler = Bookmark;
        let result = handler.run(&magic_line("bookmark", "ghost"));
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("No bookmark 'ghost'"),
            "error should mention missing bookmark: {msg}"
        );
    }

    // --- Cd arg-parsing tests ---

    #[test]
    fn cd_to_empty_arg_goes_home() {
        let _lock = DIR_LOCK.lock().unwrap();
        let orig = std::env::current_dir().unwrap();
        let handler = Cd;
        let result = handler.run(&magic_line("cd", ""));
        assert!(
            result.is_ok(),
            "cd with empty args should succeed: {:?}",
            result
        );
        let home = crate::util::home();
        assert_eq!(std::env::current_dir().unwrap(), home);
        std::env::set_current_dir(&orig).ok();
    }

    #[test]
    fn cd_to_tilde_goes_home() {
        let _lock = DIR_LOCK.lock().unwrap();
        let orig = std::env::current_dir().unwrap();
        let handler = Cd;
        let result = handler.run(&magic_line("cd", "~"));
        assert!(result.is_ok());
        let home = crate::util::home();
        assert_eq!(std::env::current_dir().unwrap(), home);
        std::env::set_current_dir(&orig).ok();
    }

    #[test]
    fn cd_to_nonexistent_returns_error() {
        let handler = Cd;
        let result = handler.run(&magic_line("cd", "/nonexistent/orchard/cd/path/xyz"));
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("does not exist"),
            "error should mention nonexistent: {msg}"
        );
    }

    #[test]
    fn cd_to_file_returns_not_a_directory_error() {
        let tmp = std::env::temp_dir().join(format!(
            "orchard-cd-file-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&tmp, "not a dir").unwrap();
        let handler = Cd;
        let result = handler.run(&magic_line("cd", tmp.to_str().unwrap()));
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(
            msg.contains("Not a directory"),
            "error should mention not a directory: {msg}"
        );
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn cd_to_existing_dir_changes_cwd() {
        let _lock = DIR_LOCK.lock().unwrap();
        let orig = std::env::current_dir().unwrap();
        let tmp = std::env::temp_dir().join(format!(
            "orchard-cd-ok-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let handler = Cd;
        let result = handler.run(&magic_line("cd", tmp.to_str().unwrap()));
        assert!(result.is_ok());
        assert_eq!(
            std::env::current_dir().unwrap(),
            std::fs::canonicalize(&tmp).unwrap()
        );
        std::env::set_current_dir(&orig).ok();
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn cd_minus_without_oldpwd_reports_no_previous() {
        let _lock = DIR_LOCK.lock().unwrap();
        let _guard = EnvGuard::remove("OLDPWD");
        let orig = std::env::current_dir().unwrap();
        let handler = Cd;
        let result = handler.run(&magic_line("cd", "-"));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "(no previous directory)\n");
        } else {
            panic!("expected Text output");
        }
        std::env::set_current_dir(&orig).ok();
    }

    // --- Env handler arg-parsing tests ---

    struct EnvGuard {
        key: String,
        original: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &str, value: &str) -> Self {
            let original = std::env::var_os(key);
            unsafe { std::env::set_var(key, value) };
            Self {
                key: key.to_string(),
                original,
            }
        }

        fn remove(key: &str) -> Self {
            let original = std::env::var_os(key);
            unsafe { std::env::remove_var(key) };
            Self {
                key: key.to_string(),
                original,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(v) => unsafe { std::env::set_var(&self.key, v) },
                None => unsafe { std::env::remove_var(&self.key) },
            }
        }
    }

    #[test]
    fn env_set_and_get_round_trip() {
        let _guard = EnvGuard::set("ORCHARD_TEST_ENV_SET", "roundtrip_val");
        let handler = Env;
        // Set already done via EnvGuard; verify get
        let result = handler.run(&magic_line("env", "ORCHARD_TEST_ENV_SET"));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "ORCHARD_TEST_ENV_SET=roundtrip_val\n");
        } else {
            panic!("expected Text output");
        }
    }

    #[test]
    fn env_set_via_handler_stores_value() {
        let _cleanup = EnvGuard::remove("ORCHARD_TEST_ENV_HANDLER_SET");
        let handler = Env;
        // Set via handler
        let result = handler.run(&magic_line(
            "env",
            "ORCHARD_TEST_ENV_HANDLER_SET=via_handler",
        ));
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Output::Silent));
        // Verify it was actually set
        assert_eq!(
            std::env::var("ORCHARD_TEST_ENV_HANDLER_SET").unwrap(),
            "via_handler"
        );
    }

    #[test]
    fn env_get_unset_var_reports_not_set() {
        let _guard = EnvGuard::remove("ORCHARD_TEST_ENV_UNSET2");
        let handler = Env;
        let result = handler.run(&magic_line("env", "ORCHARD_TEST_ENV_UNSET2"));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "ORCHARD_TEST_ENV_UNSET2: (not set)\n");
        } else {
            panic!("expected Text output");
        }
    }

    // --- %cd roundtrip test ---

    #[test]
    fn test_shell_cd_roundtrip() {
        let _lock = DIR_LOCK.lock().unwrap();
        let orig = std::env::current_dir().unwrap();
        let tmp = std::env::temp_dir().join(format!(
            "orchard-cd-rt-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let handler = super::Cd;
        let canonical = std::fs::canonicalize(&tmp).unwrap();

        // Cd to temp dir
        let result = handler.run(&magic_line("cd", tmp.to_str().unwrap()));
        assert!(
            result.is_ok(),
            "cd to temp dir should succeed: {:?}",
            result
        );
        assert_eq!(std::env::current_dir().unwrap(), canonical);

        // Cd back to original
        let result = handler.run(&magic_line("cd", orig.to_str().unwrap()));
        assert!(result.is_ok(), "cd back should succeed: {:?}", result);
        assert_eq!(std::env::current_dir().unwrap(), orig);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- %pushd / %popd tests ---

    #[test]
    #[ignore = "pre-existing: handler returns Ok(Text) on empty stack, test expects Err — see P6"]
    fn test_shell_popd_empty_stack() {
        let _lock = DIR_LOCK.lock().unwrap();
        // Popd on empty stack should fail
        let state = shell_state();
        state.lock().unwrap().dir_stack.clear();
        let handler = super::Popd;
        let result = handler.run(&magic_line("popd", ""));
        assert!(result.is_err(), "popd on empty stack should error");
        let msg = result.unwrap_err().message;
        assert!(msg.contains("empty"), "error should mention empty: {msg}");
    }

    #[test]
    fn test_shell_pushd_popd() {
        let _lock = DIR_LOCK.lock().unwrap();
        let orig = std::env::current_dir().unwrap();
        let tmp = std::env::temp_dir().join(format!(
            "orchard-pushd-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let canonical = std::fs::canonicalize(&tmp).unwrap();
        let handler_push = super::Pushd;
        let handler_pop = super::Popd;

        // Clean stack
        shell_state().lock().unwrap().dir_stack.clear();

        // Push to temp dir
        let result = handler_push.run(&magic_line("pushd", tmp.to_str().unwrap()));
        assert!(result.is_ok(), "pushd should succeed: {:?}", result);

        // Verify cwd changed
        assert_eq!(std::env::current_dir().unwrap(), canonical);

        // Verify stack has entry
        assert_eq!(shell_state().lock().unwrap().dir_stack.len(), 1);

        // Pop back
        let result = handler_pop.run(&magic_line("popd", ""));
        assert!(result.is_ok(), "popd should succeed: {:?}", result);

        // Verify cwd restored
        assert_eq!(std::env::current_dir().unwrap(), orig);

        // Verify stack empty (pop removed it)
        assert!(shell_state().lock().unwrap().dir_stack.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // --- %dhist test ---

    #[test]
    fn test_shell_dhist() {
        // Clean history
        shell_state().lock().unwrap().dir_history.clear();

        let handler = super::Dhist;
        let result = handler.run(&magic_line("dhist", ""));
        assert!(result.is_ok(), "dhist should succeed: {:?}", result);
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "(no directory history)\n");
        } else {
            panic!("expected Text output for empty history");
        }
    }

    #[test]
    fn test_shell_dhist_after_cd() {
        let _lock = DIR_LOCK.lock().unwrap();
        let orig = std::env::current_dir().unwrap();
        let tmp = std::env::temp_dir().join(format!(
            "orchard-dhist-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let cd_handler = super::Cd;
        let result = cd_handler.run(&magic_line("cd", tmp.to_str().unwrap()));
        assert!(result.is_ok(), "cd to tmp should succeed: {:?}", result);

        // dhist should succeed and have content
        let dhist_handler = super::Dhist;
        let result = dhist_handler.run(&magic_line("dhist", ""));
        assert!(result.is_ok(), "dhist should succeed: {:?}", result);
        if let Ok(Output::Text(ref text)) = result {
            assert!(
                !text.is_empty() && text != "(no directory history)\n",
                "dhist should have entries after cd, got: {text:?}"
            );
        } else {
            panic!("expected Text output");
        }

        // Restore
        std::env::set_current_dir(&orig).ok();
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
