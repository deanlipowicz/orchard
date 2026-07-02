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
// %where — Show current call stack
// ---------------------------------------------------------------------------
pub struct Where;

impl MagicHandler for Where {
    fn name(&self) -> &'static str {
        "where"
    }
    fn description(&self) -> &'static str {
        "Show the current call stack (debugger context)"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        eval_r_captured("where")
    }
}

// ---------------------------------------------------------------------------
// %c — Continue execution (debugger)
// ---------------------------------------------------------------------------
pub struct Continue;

impl MagicHandler for Continue {
    fn name(&self) -> &'static str {
        "c"
    }
    fn description(&self) -> &'static str {
        "Continue execution in the debugger"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        eval_r_silent("c")?;
        Ok(Output::Silent)
    }
}

// ---------------------------------------------------------------------------
// %tb — Print the last traceback
// ---------------------------------------------------------------------------
pub struct Traceback;

impl MagicHandler for Traceback {
    fn name(&self) -> &'static str {
        "tb"
    }
    fn description(&self) -> &'static str {
        "Print the last traceback"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        eval_r_captured("traceback()")
    }
}
