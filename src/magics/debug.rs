use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::sync::atomic::{AtomicBool, Ordering};

fn eval_r_captured(code: &str) -> Result<Output, magic::MagicError> {
    let text = crate::r_runtime::eval_string_raw_global(code)
        .map_err(|e| magic::MagicError { message: e.to_string() })?;
    Ok(Output::Text(text))
}

fn eval_r_silent(code: &str) -> Result<(), magic::MagicError> {
    crate::r_runtime::eval_string_raw_global(code)
        .map_err(|e| magic::MagicError { message: e.to_string() })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// %debug — Control debug mode
// ---------------------------------------------------------------------------
static PDB_ENABLED: AtomicBool = AtomicBool::new(false);

pub struct Debug;

impl MagicHandler for Debug {
    fn name(&self) -> &'static str { "debug" }
    fn description(&self) -> &'static str { "Control debug mode" }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let arg = line.args.trim();
        if arg.is_empty() {
            let status = if PDB_ENABLED.load(Ordering::Relaxed) { "on" } else { "off" };
            Ok(Output::Text(format!("Debug mode is {status}\n")))
        } else {
            Err(magic::MagicError { message: format!("Unknown debug subcommand: {arg}") })
        }
    }
}

// ---------------------------------------------------------------------------
// %pdb — Toggle PDB mode
// ---------------------------------------------------------------------------
pub struct Pdb;

impl MagicHandler for Pdb {
    fn name(&self) -> &'static str { "pdb" }
    fn description(&self) -> &'static str { "Toggle PDB (debug) mode on/off" }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let new = !PDB_ENABLED.load(Ordering::Relaxed);
        PDB_ENABLED.store(new, Ordering::Relaxed);
        let status = if new { "on" } else { "off" };
        Ok(Output::Text(format!("PDB is {status}\n")))
    }
}

// ---------------------------------------------------------------------------
// %where — Show current call stack
// ---------------------------------------------------------------------------
pub struct Where;

impl MagicHandler for Where {
    fn name(&self) -> &'static str { "where" }
    fn description(&self) -> &'static str { "Show the current call stack (debugger context)" }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        eval_r_captured("where")
    }
}

// ---------------------------------------------------------------------------
// %c — Continue execution (debugger)
// ---------------------------------------------------------------------------
pub struct Continue;

impl MagicHandler for Continue {
    fn name(&self) -> &'static str { "c" }
    fn description(&self) -> &'static str { "Continue execution in the debugger" }
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
    fn name(&self) -> &'static str { "tb" }
    fn description(&self) -> &'static str { "Print the last traceback" }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        eval_r_captured("traceback()")
    }
}
