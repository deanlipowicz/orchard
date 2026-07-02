use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
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

// ---------------------------------------------------------------------------
// Helper: home directory
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// %sx — Execute shell command and capture output as R character vector
// ---------------------------------------------------------------------------

pub struct Sx;

impl MagicHandler for Sx {
    fn name(&self) -> &'static str {
        "sx"
    }

    fn description(&self) -> &'static str {
        "Execute shell command and capture output as R character vector"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %sx <command>".into(),
            });
        }

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

        let output = Command::new(&shell)
            .args(["-c", args])
            .output()
            .map_err(|e| magic::MagicError {
                message: format!("Failed to execute command: {e}"),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(magic::MagicError {
                message: format!(
                    "Command failed (exit: {}): {}",
                    output.status.code().unwrap_or(-1),
                    stderr.trim()
                ),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.split('\n').filter(|l| !l.is_empty()).collect();

        // Escape each line for R string safety
        let r_lines: Vec<String> = lines
            .iter()
            .map(|l| {
                let escaped = l.replace('\\', "\\\\").replace('"', "\\\"");
                format!("\"{}\"", escaped)
            })
            .collect();

        let r_expr = format!("sx_output <- c({})", r_lines.join(", "));
        crate::r_runtime::eval_string_raw_global(&r_expr).map_err(|e| magic::MagicError {
            message: format!("R evaluation failed: {e}"),
        })?;

        let summary = if lines.len() <= 5 {
            r_lines.join(", ")
        } else {
            let first_few: Vec<&str> = r_lines.iter().take(3).map(|s| s.as_str()).collect();
            format!("{}, ... ({} total)", first_few.join(", "), lines.len())
        };

        Ok(Output::Text(format!(
            "character vector 'sx_output' assigned: [1] {}\n",
            summary
        )))
    }
}

// ---------------------------------------------------------------------------
// %cd — Change Directory
// ---------------------------------------------------------------------------

pub struct Cd;

impl MagicHandler for Cd {
    fn name(&self) -> &'static str {
        "cd"
    }

    fn description(&self) -> &'static str {
        "Change directory (supports -, ~, OLDPWD)"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        let orig = std::env::current_dir().map_err(|e| magic::MagicError {
            message: format!("Cannot get current directory: {e}"),
        })?;

        // Resolve target directory
        let target = if args.is_empty() || args == "~" {
            // Home directory
            crate::util::home()
        } else if args == "-" {
            // OLDPWD swap
            match std::env::var("OLDPWD") {
                Ok(p) => PathBuf::from(p),
                Err(_) => {
                    return Ok(Output::Text("(no previous directory)\n".into()));
                }
            }
        } else {
            // Tilde expansion, then resolve relative to cwd
            let expanded = crate::util::expand_tilde(args);
            let path = PathBuf::from(&expanded);
            if path.is_absolute() {
                path
            } else {
                orig.join(&path)
            }
        };

        // Canonicalize so we print the real path
        let canonical = std::fs::canonicalize(&target).map_err(|e| {
            if !target.exists() {
                magic::MagicError {
                    message: format!("Directory does not exist: {args}"),
                }
            } else {
                magic::MagicError {
                    message: format!("Cannot resolve path: {e}"),
                }
            }
        })?;

        // Fail if not a directory
        if !canonical.is_dir() {
            return Err(magic::MagicError {
                message: format!("Not a directory: {args}"),
            });
        }

        // Save current directory to OLDPWD
        let _guard = crate::shell::env_lock();
        unsafe { std::env::set_var("OLDPWD", orig.to_str().unwrap_or("")) };

        // Push to directory history
        let mut state = shell_state().lock().unwrap();
        state.dir_history.push(orig.clone());

        // Change directory
        std::env::set_current_dir(&canonical).map_err(|e| magic::MagicError {
            message: format!("Cannot change to directory: {e}"),
        })?;

        Ok(Output::Text(format!("{}\n", canonical.display())))
    }
}

// ---------------------------------------------------------------------------
// %pushd — Push directory onto stack, then cd
// ---------------------------------------------------------------------------

pub struct Pushd;

impl MagicHandler for Pushd {
    fn name(&self) -> &'static str {
        "pushd"
    }

    fn description(&self) -> &'static str {
        "Push directory onto stack and change to it"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        let mut state = shell_state().lock().unwrap();

        if args.is_empty() {
            // Just print the stack
            if state.dir_stack.is_empty() {
                return Ok(Output::Text("(directory stack empty)\n".into()));
            }
            let mut out = String::new();
            for entry in state.dir_stack.iter().rev() {
                out.push_str(&format!("{}\n", entry.display()));
            }
            return Ok(Output::Text(out));
        }

        let orig = std::env::current_dir().map_err(|e| magic::MagicError {
            message: format!("Cannot get current directory: {e}"),
        })?;

        let expanded = crate::util::expand_tilde(args);
        let target = PathBuf::from(&expanded);
        let canonical = std::fs::canonicalize(&target).map_err(|e| {
            if !target.exists() {
                magic::MagicError {
                    message: format!("Directory does not exist: {args}"),
                }
            } else {
                magic::MagicError {
                    message: format!("Cannot resolve path: {e}"),
                }
            }
        })?;

        if !canonical.is_dir() {
            return Err(magic::MagicError {
                message: format!("Not a directory: {args}"),
            });
        }

        state.dir_stack.push(orig);
        let _guard = crate::shell::env_lock();
        std::env::set_current_dir(&canonical).map_err(|e| magic::MagicError {
            message: format!("Cannot change to directory: {e}"),
        })?;

        let mut out = String::new();
        for entry in state.dir_stack.iter().rev() {
            out.push_str(&format!("{}\n", entry.display()));
        }
        Ok(Output::Text(out))
    }
}

// ---------------------------------------------------------------------------
// %popd — Pop directory from stack and cd back
// ---------------------------------------------------------------------------

pub struct Popd;

impl MagicHandler for Popd {
    fn name(&self) -> &'static str {
        "popd"
    }

    fn description(&self) -> &'static str {
        "Pop directory from stack and change to it"
    }

    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let mut state = shell_state().lock().unwrap();

        let target = state.dir_stack.pop().ok_or_else(|| magic::MagicError {
            message: "Directory stack is empty".into(),
        })?;

        std::env::set_current_dir(&target).map_err(|e| magic::MagicError {
            message: format!("Cannot change to directory: {e}"),
        })?;

        let mut out = String::new();
        for entry in state.dir_stack.iter().rev() {
            out.push_str(&format!("{}\n", entry.display()));
        }
        if out.is_empty() {
            out.push_str("(directory stack empty)\n");
        }
        Ok(Output::Text(out))
    }
}

// ---------------------------------------------------------------------------
// %dhist — Display directory history
// ---------------------------------------------------------------------------

pub struct Dhist;

impl MagicHandler for Dhist {
    fn name(&self) -> &'static str {
        "dhist"
    }

    fn description(&self) -> &'static str {
        "Display directory history"
    }

    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let state = shell_state().lock().unwrap();
        if state.dir_history.is_empty() {
            return Ok(Output::Text("(no directory history)\n".into()));
        }
        let mut out = String::new();
        for (i, entry) in state.dir_history.iter().enumerate() {
            out.push_str(&format!("{:>3}: {}\n", i + 1, entry.display()));
        }
        Ok(Output::Text(out))
    }
}

pub struct Pwd;
impl MagicHandler for Pwd {
    fn name(&self) -> &'static str {
        "pwd"
    }
    fn description(&self) -> &'static str {
        "Print working directory"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let cwd = std::env::current_dir().map_err(|e| magic::MagicError {
            message: e.to_string(),
        })?;
        Ok(Output::Text(format!("{}\n", cwd.display())))
    }
}

pub struct Env;

impl MagicHandler for Env {
    fn name(&self) -> &'static str {
        "env"
    }

    fn description(&self) -> &'static str {
        "List/set/get environment variables"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            // List all
            let mut vars: Vec<_> = std::env::vars().collect();
            vars.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = String::new();
            for (k, v) in vars {
                out.push_str(&format!("{}={}\n", k, v));
            }
            Ok(Output::Text(out))
        } else if let Some((key, val)) = args.split_once('=') {
            // Set VAR=value
            let _guard = crate::shell::env_lock();
            // Safety: env_lock() ensures exclusive access to env vars.
            // The key and value are short ASCII strings from user input.
            unsafe {
                std::env::set_var(key.trim(), val.trim());
            }
            Ok(Output::Silent)
        } else {
            // Get VAR
            match std::env::var(args) {
                Ok(v) => Ok(Output::Text(format!("{}={}\n", args, v))),
                Err(_) => Ok(Output::Text(format!("{}: (not set)\n", args))),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// %ls — List Directory Contents
// ---------------------------------------------------------------------------

pub struct Ls;

impl MagicHandler for Ls {
    fn name(&self) -> &'static str {
        "ls"
    }

    fn description(&self) -> &'static str {
        "List directory contents"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        let dir = if args.is_empty() {
            std::env::current_dir().map_err(|e| magic::MagicError {
                message: format!("Cannot get current directory: {e}"),
            })?
        } else {
            let expanded = crate::util::expand_tilde(args);
            PathBuf::from(&expanded)
        };

        let entries = std::fs::read_dir(&dir).map_err(|e| {
            if !dir.exists() {
                magic::MagicError {
                    message: format!("Directory does not exist: {args}"),
                }
            } else {
                magic::MagicError {
                    message: format!("Cannot read directory: {e}"),
                }
            }
        })?;

        let mut names: Vec<String> = Vec::new();
        for entry in entries {
            match entry {
                Ok(e) => {
                    if let Some(name) = e.file_name().to_str() {
                        names.push(name.to_string());
                    }
                }
                Err(_) => continue,
            }
        }

        names.sort();
        let mut out = String::new();
        for name in &names {
            out.push_str(name);
            out.push('\n');
        }
        out.push_str(&format!("({} entries)\n", names.len()));
        Ok(Output::Text(out))
    }
}

// ---------------------------------------------------------------------------
// %bookmark — Directory bookmarks
// ---------------------------------------------------------------------------

pub struct Bookmark;

impl MagicHandler for Bookmark {
    fn name(&self) -> &'static str {
        "bookmark"
    }

    fn description(&self) -> &'static str {
        "Manage directory bookmarks: %bookmark, %bookmark <name> [dir], %bookmark -d <name>"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            // List bookmarks
            let state = shell_state().lock().unwrap();
            if state.bookmarks.is_empty() {
                return Ok(Output::Text("(no bookmarks)\n".into()));
            }
            let mut out = String::new();
            let mut names: Vec<_> = state.bookmarks.keys().collect();
            names.sort();
            for name in names {
                if let Some(path) = state.bookmarks.get(name) {
                    out.push_str(&format!("  {} -> {}\n", name, path.display()));
                }
            }
            Ok(Output::Text(out))
        } else if args == "-d" || args.starts_with("-d ") {
            // Delete bookmark: %bookmark -d <name>
            let name = args.strip_prefix("-d ").unwrap_or("").trim();
            if name.is_empty() {
                return Err(magic::MagicError {
                    message: "Usage: %bookmark -d <name>".into(),
                });
            }
            let mut state = shell_state().lock().unwrap();
            if state.bookmarks.remove(name).is_some() {
                Ok(Output::Text(format!("Removed bookmark '{name}'\n")))
            } else {
                Ok(Output::Text(format!("No bookmark '{name}'\n")))
            }
        } else if let Some((name, dir)) = args.split_once(' ') {
            // Set bookmark: %bookmark <name> <dir>
            if name == "-d" {
                return Err(magic::MagicError {
                    message: "Usage: %bookmark -d <name>".into(),
                });
            }
            let resolved = if dir.starts_with('~') {
                crate::util::expand_tilde(dir)
            } else {
                dir.to_string()
            };
            let path = PathBuf::from(&resolved);
            if !path.exists() {
                return Err(magic::MagicError {
                    message: format!("Directory does not exist: {dir}"),
                });
            }
            let mut state = shell_state().lock().unwrap();
            state.bookmarks.insert(name.to_string(), path.clone());
            Ok(Output::Text(format!(
                "Bookmark '{name}' -> {}\n",
                path.display()
            )))
        } else {
            // Jump to bookmark: %bookmark <name>
            let state = shell_state().lock().unwrap();
            match state.bookmarks.get(args) {
                Some(path) => {
                    let _guard = crate::shell::env_lock();
                    std::env::set_current_dir(path).ok();
                    Ok(Output::Text(format!("{} -> {}\n", args, path.display())))
                }
                None => Err(magic::MagicError {
                    message: format!("No bookmark '{args}'. Use %bookmark to list."),
                }),
            }
        }
    }
}

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
