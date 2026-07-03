//! %ls — List directory contents.

use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::path::PathBuf;

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
