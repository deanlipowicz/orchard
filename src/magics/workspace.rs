use crate::magic::{self, MagicHandler, MagicLine, Output};

fn eval_r_captured(code: &str) -> Result<Output, magic::MagicError> {
    let text = crate::r_runtime::eval_string_raw_global(code).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    Ok(Output::Text(text))
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
