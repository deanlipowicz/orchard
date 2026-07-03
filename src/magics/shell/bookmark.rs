//! %bookmark — Manage directory bookmarks.

use crate::magic::{self, MagicHandler, MagicLine, Output};
use super::shell_state;
use std::path::PathBuf;

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
