use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::path::Path;

pub struct Run;

impl MagicHandler for Run {
    fn name(&self) -> &'static str {
        "run"
    }

    fn description(&self) -> &'static str {
        "Run an R script from a file"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let path = line.args.trim();
        if path.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %run <filepath>".into(),
            });
        }
        let resolved = if path.starts_with('~') {
            crate::magics::shell::expand_tilde(path)
        } else {
            path.to_string()
        };
        if !Path::new(&resolved).exists() {
            return Err(magic::MagicError {
                message: format!("File not found: {path}"),
            });
        }
        crate::r_runtime::eval_string_raw_global(&format!("source({:?})", resolved))
            .map_err(|e| magic::MagicError {
                message: e.to_string(),
            })?;
        Ok(Output::Text(format!("Sourced {path}\n")))
    }
}

pub struct Load;

impl MagicHandler for Load {
    fn name(&self) -> &'static str {
        "load"
    }

    fn description(&self) -> &'static str {
        "Load file contents into the REPL"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let path = line.args.trim();
        if path.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %load <filepath>".into(),
            });
        }
        let resolved = if path.starts_with('~') {
            crate::magics::shell::expand_tilde(path)
        } else {
            path.to_string()
        };
        let contents = std::fs::read_to_string(&resolved).map_err(|e| {
            if !Path::new(&resolved).exists() {
                magic::MagicError {
                    message: format!("File not found: {path}"),
                }
            } else {
                magic::MagicError {
                    message: format!("Cannot read {path}: {e}"),
                }
            }
        })?;
        Ok(Output::Text(contents))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_load_nonexistent() {
        let handler = super::Load;
        let line = MagicLine {
            name: "load".into(),
            args: "/tmp/orchard-nonexistent-load-file-××××.R".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for nonexistent file");
    }
}
