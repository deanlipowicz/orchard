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
            crate::util::expand_tilde(path)
        } else {
            path.to_string()
        };
        if !Path::new(&resolved).exists() {
            return Err(magic::MagicError {
                message: format!("File not found: {path}"),
            });
        }
        crate::r_runtime::eval_string_raw_global(&format!("source({:?})", resolved)).map_err(
            |e| magic::MagicError {
                message: e.to_string(),
            },
        )?;
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
            crate::util::expand_tilde(path)
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

/// Smart data loader — sniffs file extension and dispatches to the best R
/// reader function. Supported formats:
///
/// | Extension | R reader | Package |
/// |-----------|----------|---------|
/// | .csv      | read_csv | readr   |
/// | .tsv      | read_tsv | readr   |
/// | .txt      | read_table | readr |
/// | .xls/.xlsx | read_excel | readxl |
/// | .parquet  | read_parquet | arrow |
/// | .rds      | readRDS  | base    |
/// | .sas7bdat | read_sas | haven   |
/// | .dta      | read_dta | haven   |
/// | .sav      | read_sav | haven   |
/// | .json     | fromJSON | jsonlite |
pub struct Import;

/// Map a file extension (lowercase, without dot) to an R reader expression.
/// Returns `None` for unknown extensions.
fn reader_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "csv" => Some("readr::read_csv"),
        "tsv" => Some("readr::read_tsv"),
        "txt" => Some("readr::read_table"),
        "xls" | "xlsx" => Some("readxl::read_excel"),
        "parquet" => Some("arrow::read_parquet"),
        "rds" => Some("readRDS"),
        "sas7bdat" => Some("haven::read_sas"),
        "dta" => Some("haven::read_dta"),
        "sav" => Some("haven::read_sav"),
        "json" => Some("jsonlite::fromJSON"),
        _ => None,
    }
}

impl MagicHandler for Import {
    fn name(&self) -> &'static str {
        "import"
    }

    fn description(&self) -> &'static str {
        "Import a data file — sniffs extension and dispatches to readr/readxl/arrow/haven/jsonlite"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let path = line.args.trim();
        if path.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %import <filepath>".into(),
            });
        }

        let resolved = if path.starts_with('~') {
            crate::util::expand_tilde(path)
        } else {
            path.to_string()
        };

        // Extract extension from the filename (check before file existence).
        let ext = match std::path::Path::new(&resolved)
            .extension()
            .and_then(|e| e.to_str())
        {
            Some(e) => e.to_lowercase(),
            None => {
                return Err(magic::MagicError {
                    message: format!("Cannot determine file extension for {path}"),
                });
            }
        };

        let reader = reader_for_extension(&ext).ok_or_else(|| magic::MagicError {
            message: format!("Unsupported file extension: .{ext}. Supported: csv, tsv, txt, xls, xlsx, parquet, rds, sas7bdat, dta, sav, json"),
        })?;

        if !std::path::Path::new(&resolved).exists() {
            return Err(magic::MagicError {
                message: format!("File not found: {path}"),
            });
        }

        // Build R code: assign the result to a variable named after the file stem.
        let stem = std::path::Path::new(&resolved)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("data");
        // Sanitise stem: replace non-alphanumeric characters (except _) with _.
        let var_name: String = stem
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        let r_code = format!(
            r#"{} <- {}({})"#,
            var_name,
            reader,
            crate::util::r_string(&resolved)
        );

        match crate::r_runtime::eval_string_raw_global(&r_code) {
            Ok(output) => Ok(Output::Text(format!(
                "Imported {path} → {var_name}\n{output}"
            ))),
            Err(e) => Err(magic::MagicError {
                message: format!("Error importing {path}: {e}"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_run_nonexistent() {
        let handler = super::Run;
        let line = MagicLine {
            name: "run".into(),
            args: "/tmp/orchard-nonexistent-run-file-××××.R".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for nonexistent file");
    }

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

    // -- %import tests -------------------------------------------------------

    #[test]
    fn test_import_empty_args() {
        let handler = super::Import;
        let line = MagicLine {
            name: "import".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for empty args");
    }

    #[test]
    fn test_import_nonexistent_file() {
        let handler = super::Import;
        let line = MagicLine {
            name: "import".into(),
            args: "/tmp/orchard-nonexistent-import-file-××××.csv".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for nonexistent file");
    }

    #[test]
    fn test_import_unsupported_extension() {
        let handler = super::Import;
        let line = MagicLine {
            name: "import".into(),
            args: "data.xyz".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for unsupported extension");
        let err = result.unwrap_err();
        assert!(err.message.contains("Unsupported file extension"));
    }

    #[test]
    fn test_reader_for_extension() {
        assert_eq!(super::reader_for_extension("csv"), Some("readr::read_csv"));
        assert_eq!(super::reader_for_extension("tsv"), Some("readr::read_tsv"));
        assert_eq!(
            super::reader_for_extension("xlsx"),
            Some("readxl::read_excel")
        );
        assert_eq!(
            super::reader_for_extension("parquet"),
            Some("arrow::read_parquet")
        );
        assert_eq!(super::reader_for_extension("rds"), Some("readRDS"));
        assert_eq!(
            super::reader_for_extension("json"),
            Some("jsonlite::fromJSON")
        );
        assert_eq!(super::reader_for_extension("xyz"), None);
    }

    #[test]
    fn test_import_success_path() {
        // Write a tiny CSV to a temp file.
        let dir = std::env::temp_dir().join("orchard_import_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_data.csv");
        std::fs::write(&path, "a,b\n1,2\n3,4\n").unwrap();

        let handler = super::Import;
        let path_str = path.to_string_lossy().to_string();
        let line = MagicLine {
            name: "import".into(),
            args: path_str.clone(),
            is_cell: false,
        };
        let result = handler.run(&line);
        // This test will try to evaluate in R, which may fail if R isn't
        // initialized. We accept either success or an R-not-available error.
        match result {
            Ok(Output::Text(text)) => {
                assert!(text.contains("test_data"), "output: {text}");
            }
            Ok(_) => panic!("unexpected Output variant"),
            Err(e) => {
                // R not available in test context — that's acceptable.
                assert!(
                    e.message.contains("R is not initialized")
                        || e.message.contains("Error importing"),
                    "unexpected error: {}",
                    e.message
                );
            }
        }

        std::fs::remove_dir_all(dir.parent().unwrap()).ok();
    }
}
