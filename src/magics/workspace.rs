use crate::magic::{self, MagicHandler, MagicLine, Output};

fn eval_r_captured(code: &str) -> Result<Output, magic::MagicError> {
    let text = crate::r_runtime::eval_string_raw_global(code).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    Ok(Output::Text(text))
}

fn eval_r_silent(code: &str) -> Result<(), magic::MagicError> {
    crate::r_runtime::eval_string_raw_global(code).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    Ok(())
}

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
            eval_r_captured("ls()")
        } else {
            eval_r_captured(&format!("ls(pattern=\"{}\")", line.args.trim()))
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
        eval_r_captured("ls.str()")
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
            eval_r_captured(&format!(
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
            eval_r_captured(&format!(r#"saveRDS({obj}, file = "{file}")"#,))?;
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
            eval_r_silent("rm(list = ls(all = TRUE))")?;
            Ok(Output::Text("Workspace cleared.\n".into()))
        } else {
            // Selective reset: remove only matching objects
            eval_r_silent(&format!("rm(list = ls(pattern = '{args}', all = TRUE))"))?;
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
        eval_r_captured(&code)
    }
}
