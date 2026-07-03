//! %sx — Execute shell command and capture output as R character vector.

use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::process::Command;

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
