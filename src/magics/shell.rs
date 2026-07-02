use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::env::home_dir;
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

/// Expand `~` and `~/` to the home directory path.
pub(crate) fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return format!("{}/{}", home.display(), rest);
        }
    } else if input == "~"
        && let Some(home) = home_dir() {
            return home.display().to_string();
        }
    input.to_string()
}

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
                message: format!("Command failed (exit: {}): {}",
                    output.status.code().unwrap_or(-1),
                    stderr.trim()),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout
            .split('\n')
            .filter(|l| !l.is_empty())
            .collect();

        // Escape each line for R string safety
        let r_lines: Vec<String> = lines
            .iter()
            .map(|l| {
                let escaped = l
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"");
                format!("\"{}\"", escaped)
            })
            .collect();

        let r_expr = format!("sx_output <- c({})", r_lines.join(", "));
        crate::r_runtime::eval_string_raw_global(&r_expr)
            .map_err(|e| magic::MagicError {
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
            home_dir().ok_or_else(|| magic::MagicError {
                message: "Cannot determine home directory".into(),
            })?
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
            let expanded = expand_tilde(args);
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

        let expanded = expand_tilde(args);
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
    fn name(&self) -> &'static str { "pwd" }
    fn description(&self) -> &'static str { "Print working directory" }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let cwd = std::env::current_dir()
            .map_err(|e| magic::MagicError { message: e.to_string() })?;
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
            unsafe { std::env::set_var(key.trim(), val.trim()); }
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
            let expanded = expand_tilde(args);
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
                return Err(magic::MagicError { message: "Usage: %bookmark -d <name>".into() });
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
                return Err(magic::MagicError { message: "Usage: %bookmark -d <name>".into() });
            }
            let resolved = if dir.starts_with('~') {
                expand_tilde(dir)
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
            Ok(Output::Text(format!("Bookmark '{name}' -> {}\n", path.display())))
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
        let handler = super::Cd;
        let orig = std::env::current_dir().unwrap();
        // Safety: single-threaded test, env mutation is safe here.
        unsafe { std::env::set_var("OLDPWD", "/tmp"); }
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
}
