//! Directory operations: %cd, %pushd, %popd, %dhist, %pwd.

use crate::magic::{self, MagicHandler, MagicLine, Output};
use super::shell_state;
use std::path::PathBuf;

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

        let target = if args.is_empty() || args == "~" {
            crate::util::home()
        } else if args == "-" {
            match std::env::var("OLDPWD") {
                Ok(p) => PathBuf::from(p),
                Err(_) => {
                    return Ok(Output::Text("(no previous directory)\n".into()));
                }
            }
        } else {
            let expanded = crate::util::expand_tilde(args);
            let path = PathBuf::from(&expanded);
            if path.is_absolute() {
                path
            } else {
                orig.join(&path)
            }
        };

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

        let _guard = crate::shell::env_lock();
        unsafe { std::env::set_var("OLDPWD", orig.to_str().unwrap_or("")) };

        let mut state = shell_state().lock().unwrap();
        state.dir_history.push(orig.clone());

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
            if state.dir_stack.is_empty() {
                return Ok(Output::Text("(directory stack empty)\n".into()));
            }
            let mut out = String::new();
            for entry in state.dir_stack.iter().rev() {
                out.push_str(&format!("{}\n", entry.display()));
            }
            return Ok(Output::Text(out));
        }

        let cwd = std::env::current_dir().map_err(|e| magic::MagicError {
            message: format!("Cannot get current directory: {e}"),
        })?;

        state.dir_stack.push(cwd.clone());

        let target = if args == "~" {
            crate::util::home()
        } else {
            let expanded = crate::util::expand_tilde(args);
            let path = PathBuf::from(&expanded);
            if path.is_absolute() { path } else { cwd.join(&path) }
        };

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

        std::env::set_current_dir(&canonical).map_err(|e| magic::MagicError {
            message: format!("Cannot change to directory: {e}"),
        })?;

        Ok(Output::Text(format!(
            "{} ({})\n",
            canonical.display(),
            state.dir_stack.len()
        )))
    }
}

// ---------------------------------------------------------------------------
// %popd — Pop directory from stack and cd
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
        if state.dir_stack.is_empty() {
            return Ok(Output::Text("(directory stack empty)\n".into()));
        }

        let target = state.dir_stack.pop().unwrap();
        std::env::set_current_dir(&target).map_err(|e| magic::MagicError {
            message: format!("Cannot change to directory: {e}"),
        })?;

        let mut out = format!("{}\n", target.display());
        for entry in state.dir_stack.iter().rev() {
            out.push_str(&format!("{}\n", entry.display()));
        }
        if state.dir_stack.is_empty() {
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

// ---------------------------------------------------------------------------
// %pwd — Print working directory
// ---------------------------------------------------------------------------

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
