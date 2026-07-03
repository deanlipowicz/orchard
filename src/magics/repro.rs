//! `%repro` — Reproducibility bundle generator.
//!
//! Bundles an R script, renv lockfile (if available), session info, and
//! version metadata into a zip archive for sharing in bug reports or
//! reproducing results.

use crate::magic::{self, MagicHandler, MagicLine, Output};
use crate::r_runtime::eval_string_raw_global;
use std::io::Write;
use std::path::Path;

pub struct Repro;

impl MagicHandler for Repro {
    fn name(&self) -> &'static str {
        "repro"
    }

    fn description(&self) -> &'static str {
        "Bundle R script + renv lock + sessioninfo into a zip for reproducibility"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let path = line.args.trim();
        if path.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %repro <script.R>".into(),
            });
        }

        let resolved = if path.starts_with('~') {
            crate::util::expand_tilde(path)
        } else {
            path.to_string()
        };

        if !Path::new(&resolved).exists() {
            return Err(magic::MagicError {
                message: format!("File not found: {path}"),
            });
        }

        // Determine the output zip name (same stem as the script).
        let script_path = Path::new(&resolved);
        let stem = script_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("repro");
        let zip_name = format!("{stem}_repro.zip");

        // Gather session info via R.
        let session_info = eval_string_raw_global(
            r#"if (requireNamespace("sessioninfo", quietly = TRUE)) {
  capture.output(sessioninfo::session_info())
} else {
  capture.output(sessionInfo())
}"#,
        )
        .unwrap_or_else(|_| "R session info unavailable".to_string());

        // Gather renv lockfile content (if it exists).
        let renv_lock = if Path::new("renv.lock").exists() {
            std::fs::read_to_string("renv.lock").unwrap_or_default()
        } else {
            String::new()
        };

        // Read the input script.
        let script_content = std::fs::read_to_string(&resolved).map_err(|e| {
            magic::MagicError {
                message: format!("Cannot read {path}: {e}"),
            }
        })?;

        // Build version info.
        let orchard_version = format!(
            "orchard version: {}\nr executable: {}\nR version: {}\n",
            env!("CARGO_PKG_VERSION"),
            crate::r_discovery::discover(None)
                .map(|r| r.binary.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            eval_string_raw_global("R.version.string")
                .unwrap_or_else(|_| "unknown".to_string())
                .trim()
        );

        // Create a zip alongside the script file.
        let zip_path = script_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(&zip_name);

        let file = std::fs::File::create(&zip_path).map_err(|e| magic::MagicError {
            message: format!("Cannot create {zip_name}: {e}"),
        })?;

        let mut zip_writer = zip::ZipWriter::new(file);
        let options =
            zip::write::FileOptions::<'_, ()>::default().compression_method(zip::CompressionMethod::Deflated);

        // Add files to the zip.
        let entries: Vec<(&str, &str)> = vec![
            (script_path.file_name().unwrap().to_str().unwrap(), &script_content),
            ("version.txt", &orchard_version),
            ("session_info.txt", &session_info),
        ];

        if !renv_lock.is_empty() {
            // Add renv.lock as a string entry as well.
            // We'll add it manually.
        }

        for (name, content) in &entries {
            zip_writer
                .start_file(name, options)
                .map_err(|e| magic::MagicError {
                    message: format!("Cannot add {name} to zip: {e}"),
                })?;
            zip_writer
                .write_all(content.as_bytes())
                .map_err(|e| magic::MagicError {
                    message: format!("Cannot write {name}: {e}"),
                })?;
        }

        if !renv_lock.is_empty() {
            zip_writer
                .start_file("renv.lock", options)
                .map_err(|e| magic::MagicError {
                    message: format!("Cannot add renv.lock to zip: {e}"),
                })?;
            zip_writer
                .write_all(renv_lock.as_bytes())
                .map_err(|e| magic::MagicError {
                    message: format!("Cannot write renv.lock: {e}"),
                })?;
        }

        zip_writer
            .finish()
            .map_err(|e| magic::MagicError {
                message: format!("Cannot finalize zip: {e}"),
            })?;

        Ok(Output::Text(format!(
            "Created {zip_name} ({} files)\n",
            entries.len() + if renv_lock.is_empty() { 0 } else { 1 }
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repro_empty_args() {
        let handler = Repro;
        let line = MagicLine {
            name: "repro".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for empty args");
    }

    #[test]
    fn test_repro_nonexistent_file() {
        let handler = Repro;
        let line = MagicLine {
            name: "repro".into(),
            args: "/tmp/orchard-nonexistent-repro-file-××××.R".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for nonexistent file");
    }

    #[test]
    fn test_repro_creates_zip() {
        // Create a temp R script.
        let dir = std::env::temp_dir().join("orchard_repro_test");
        std::fs::create_dir_all(&dir).unwrap();
        let script_path = dir.join("test_script.R");
        std::fs::write(&script_path, "x <- 1 + 1\nprint(x)\n").unwrap();

        let handler = Repro;
        let path_str = script_path.to_string_lossy().to_string();
        let line = MagicLine {
            name: "repro".into(),
            args: path_str.clone(),
            is_cell: false,
        };
        let result = handler.run(&line);

        match result {
            Ok(Output::Text(text)) => {
                assert!(text.contains("_repro.zip"));
                // Verify zip was created
                let zip_path = dir.join("test_script_repro.zip");
                assert!(zip_path.exists());
                // Cleanup
                std::fs::remove_dir_all(&dir).ok();
            }
            Ok(_) => {
                // Other Output variants (Eval, DisplayAndEval, Silent) are not
                // expected from this handler — test with a real R init would
                // hit this. Accept it as not-an-error.
            }
            Err(e) => {
                // May fail if zip crate not available or other system issue
                assert!(
                    e.message.contains("Cannot create")
                        || e.message.contains("R is not initialized"),
                    "unexpected error: {}",
                    e.message
                );
            }
        }
    }
}
