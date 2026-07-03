use super::r_utils;
use crate::magic::{self, MagicHandler, MagicLine, Output};

// ---------------------------------------------------------------------------
// %pinfo — Show object info (alias for %whos)
// ---------------------------------------------------------------------------

pub struct Pinfo;

impl MagicHandler for Pinfo {
    fn name(&self) -> &'static str {
        "pinfo"
    }
    fn description(&self) -> &'static str {
        "Show information about objects in the workspace"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.trim().is_empty() {
            r_utils::eval_r_captured("ls()")
        } else {
            r_utils::eval_r_captured(&format!("ls(pattern=\"{}\")", line.args.trim()))
        }
    }
}

// ---------------------------------------------------------------------------
// %pinfo2 — Alternative object info display
// ---------------------------------------------------------------------------

pub struct Pinfo2;

impl MagicHandler for Pinfo2 {
    fn name(&self) -> &'static str {
        "pinfo2"
    }
    fn description(&self) -> &'static str {
        "Show extended object information"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        r_utils::eval_r_captured("ls.str()")
    }
}

// ---------------------------------------------------------------------------
// %store — Persistently save/load R objects via saveRDS/readRDS
// ---------------------------------------------------------------------------

pub struct Store;

impl MagicHandler for Store {
    fn name(&self) -> &'static str {
        "store"
    }
    fn description(&self) -> &'static str {
        "Save or load objects to/from a file: %store objname filename, %store -l filename"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %store objname <filename> | %store -l <filename>".into(),
            });
        }
        if let Some(rest) = args.strip_prefix("-l ") {
            let file = rest.trim();
            if file.is_empty() {
                return Err(magic::MagicError {
                    message: "Usage: %store -l <filename>".into(),
                });
            }
            r_utils::eval_r_captured(&format!(
                r#"load_or_error <- function(f) {{ if (!file.exists(f)) stop("file not found: ", f); readRDS(f) }}; print(load_or_error("{file}"))"#,
            ))
        } else {
            let parts: Vec<&str> = args.splitn(2, ' ').collect();
            if parts.len() < 2 || parts[1].trim().is_empty() {
                return Err(magic::MagicError {
                    message: "Usage: %store objname <filename>".into(),
                });
            }
            let obj = parts[0].trim();
            let file = parts[1].trim();
            r_utils::eval_r_captured(&format!(r#"saveRDS({obj}, file = "{file}")"#,))?;
            Ok(Output::Text(format!("Saved {obj} to {file}\n")))
        }
    }
}

// ---------------------------------------------------------------------------
// %reset — Clear the workspace
// ---------------------------------------------------------------------------

pub struct Reset;

impl MagicHandler for Reset {
    fn name(&self) -> &'static str {
        "reset"
    }
    fn description(&self) -> &'static str {
        "Remove all objects from the workspace"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            r_utils::eval_r_silent("rm(list = ls(all = TRUE))")?;
            Ok(Output::Text("Workspace cleared.\n".into()))
        } else {
            // Selective reset: remove only matching objects
            r_utils::eval_r_silent(&format!("rm(list = ls(pattern = '{args}', all = TRUE))"))?;
            Ok(Output::Text(format!(
                "Removed objects matching '{args}'.\n"
            )))
        }
    }
}

// ---------------------------------------------------------------------------
// %xdel — Delete object with undo support
// ---------------------------------------------------------------------------

pub struct Xdel;

impl MagicHandler for Xdel {
    fn name(&self) -> &'static str {
        "xdel"
    }
    fn description(&self) -> &'static str {
        "Delete an object, saving a backup to .last_del"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let obj = line.args.trim();
        if obj.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %xdel <object_name>".into(),
            });
        }
        // Save backup, then delete
        let code = format!(
            r#"
if (exists("{obj}", envir = .GlobalEnv)) {{
  .GlobalEnv$.last_del <- get("{obj}", envir = .GlobalEnv)
  rm("{obj}", envir = .GlobalEnv)
  cat("{obj} deleted. Use .last_del to restore.\n")
}} else {{
  cat("Object '{obj}' not found.\n")
}}"#,
        );
        r_utils::eval_r_captured(&code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Registration tests ---

    #[test]
    fn store_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("store").is_some());
    }

    #[test]
    fn reset_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("reset").is_some());
    }

    #[test]
    fn xdel_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("xdel").is_some());
    }

    #[test]
    fn pinfo_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("pinfo").is_some());
    }

    // --- Store arg parsing (no R needed) ---

    #[test]
    fn store_empty_args_errors() {
        let line = MagicLine {
            name: "store".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = Store.run(&line);
        assert!(result.is_err());
    }

    #[test]
    fn store_load_no_filename_errors() {
        let line = MagicLine {
            name: "store".into(),
            args: "-l ".into(),
            is_cell: false,
        };
        let result = Store.run(&line);
        assert!(result.is_err());
    }

    #[test]
    fn store_save_no_filename_errors() {
        let line = MagicLine {
            name: "store".into(),
            args: "myobj".into(),
            is_cell: false,
        };
        let result = Store.run(&line);
        assert!(result.is_err());
    }

    // --- Reset arg parsing (no R needed) ---

    #[test]
    #[ignore = "requires R initialization"]
    fn reset_clears_workspace() {
        let line = MagicLine {
            name: "reset".into(),
            args: "".into(),
            is_cell: false,
        };
        let _result = Reset.run(&line);
    }

    // --- Xdel arg parsing (no R needed) ---

    #[test]
    fn xdel_empty_args_errors() {
        let line = MagicLine {
            name: "xdel".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = Xdel.run(&line);
        assert!(result.is_err());
    }

    // --- Handler identities ---

    #[test]
    fn store_name() {
        assert_eq!(Store.name(), "store");
    }

    #[test]
    fn reset_name() {
        assert_eq!(Reset.name(), "reset");
    }

    #[test]
    fn xdel_name() {
        assert_eq!(Xdel.name(), "xdel");
    }
}
