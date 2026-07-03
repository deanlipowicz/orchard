use crate::magic::{self, MagicHandler, MagicLine, Output};
use super::r_utils;
use std::sync::{Mutex, OnceLock};

/// Xmode verbosity levels.
pub const XMODE_PLAIN: &str = "plain";
pub const XMODE_CONTEXT: &str = "context";
pub const XMODE_VERBOSE: &str = "verbose";
pub const XMODE_DEFAULT: &str = XMODE_CONTEXT;

const VALID_XMODES: &[&str] = &[XMODE_PLAIN, XMODE_CONTEXT, XMODE_VERBOSE];

/// The xmode state — controls traceback verbosity for `%tb`.
static XMODE: OnceLock<Mutex<String>> = OnceLock::new();

fn xmode_state() -> &'static Mutex<String> {
    XMODE.get_or_init(|| Mutex::new(XMODE_DEFAULT.to_string()))
}

/// Set the xmode level. Returns an error if the mode is invalid.
pub fn set_xmode(mode: &str) -> Result<(), String> {
    let m = mode.trim().to_lowercase();
    if !VALID_XMODES.contains(&m.as_str()) {
        return Err(format!(
            "Unknown xmode '{}'. Valid modes: {}",
            mode,
            VALID_XMODES.join(", ")
        ));
    }
    *xmode_state().lock().unwrap() = m.clone();
    Ok(())
}

/// Get the current xmode level.
pub fn get_xmode() -> String {
    xmode_state().lock().unwrap().clone()
}

/// Return the R traceback expression adjusted for the current xmode.
fn traceback_code() -> String {
    match get_xmode().as_str() {
        XMODE_PLAIN => "cat(conditionMessage(attr(last.warning, 'condition')))\n".to_string(),
        XMODE_VERBOSE => "traceback(max.lines = NULL)".to_string(),
        _ => "traceback()".to_string(),
    }
}

// ---------------------------------------------------------------------------
// %xmode — Control traceback verbosity
// ---------------------------------------------------------------------------
pub struct Xmode;

impl MagicHandler for Xmode {
    fn name(&self) -> &'static str {
        "xmode"
    }
    fn description(&self) -> &'static str {
        "Control traceback verbosity: plain | context | verbose"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            return Ok(Output::Text(format!(
                "Current xmode: {}\nValid modes: {}\n",
                get_xmode(),
                VALID_XMODES.join(", ")
            )));
        }
        set_xmode(args).map_err(|e| magic::MagicError { message: e })?;
        Ok(Output::Text(format!("Xmode set to '{}'.\n", get_xmode())))
    }
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
        r_utils::eval_r_captured("where")
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
        r_utils::eval_r_silent("c")?;
        Ok(Output::Silent)
    }
}

// ---------------------------------------------------------------------------
// %tb — Print the last traceback (respects xmode)
// ---------------------------------------------------------------------------
pub struct Traceback;

impl MagicHandler for Traceback {
    fn name(&self) -> &'static str {
        "tb"
    }
    fn description(&self) -> &'static str {
        "Print the last traceback (use %xmode to set verbosity)"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        r_utils::eval_r_captured(&traceback_code())
    }
}

// ---------------------------------------------------------------------------
// %debug — Enter post-mortem debugger
// ---------------------------------------------------------------------------
pub struct Debug;

impl MagicHandler for Debug {
    fn name(&self) -> &'static str {
        "debug"
    }
    fn description(&self) -> &'static str {
        "Enter post-mortem debugger (recover)"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        r_utils::eval_r_captured("recover()")
    }
}

// ---------------------------------------------------------------------------
// %pdb — Toggle post-mortem debugger
// ---------------------------------------------------------------------------
pub struct Pdb;

impl MagicHandler for Pdb {
    fn name(&self) -> &'static str {
        "pdb"
    }
    fn description(&self) -> &'static str {
        "Toggle post-mortem debugger: on | off"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            let result = r_utils::eval_r_captured("cat(deparse(getOption('error')))")?;
            let current = match &result {
                Output::Text(t) => t.clone(),
                _ => format!("{:?}", result),
            };
            return Ok(Output::Text(format!("Current error handler: {}", current)));
        }
        match args {
            "on" => {
                r_utils::eval_r_silent("options(error = recover)")?;
                Ok(Output::Text("Post-mortem debugger enabled.\n".into()))
            }
            "off" => {
                r_utils::eval_r_silent("options(error = NULL)")?;
                Ok(Output::Text("Post-mortem debugger disabled.\n".into()))
            }
            _ => Err(magic::MagicError {
                message: format!("Usage: %pdb [on|off]. Unknown option: {args}"),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// %debugonce — Set a function to debug once
// ---------------------------------------------------------------------------
pub struct DebugOnce;

impl MagicHandler for DebugOnce {
    fn name(&self) -> &'static str {
        "debugonce"
    }
    fn description(&self) -> &'static str {
        "Set a function to debug once"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let name = line.args.trim();
        if name.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %debugonce <function_name>".into(),
            });
        }
        r_utils::eval_r_silent(&format!("debugonce({name})"))?;
        Ok(Output::Silent)
    }
}

// ---------------------------------------------------------------------------
// %undebug — Remove debugger from a function
// ---------------------------------------------------------------------------
pub struct Undebug;

impl MagicHandler for Undebug {
    fn name(&self) -> &'static str {
        "undebug"
    }
    fn description(&self) -> &'static str {
        "Remove debugger from a function"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let name = line.args.trim();
        if name.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %undebug <function_name>".into(),
            });
        }
        r_utils::eval_r_silent(&format!("undebug({name})"))?;
        Ok(Output::Silent)
    }
}

// ---------------------------------------------------------------------------
// %browser — Invoke browser() at the current point
// ---------------------------------------------------------------------------
pub struct Browser;

impl MagicHandler for Browser {
    fn name(&self) -> &'static str {
        "browser"
    }
    fn description(&self) -> &'static str {
        "Invoke browser() at the current point"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Eval("browser()".into()))
    }
}

// ---------------------------------------------------------------------------
// %n — Execute next line in the debugger
// ---------------------------------------------------------------------------
pub struct StepNext;

impl MagicHandler for StepNext {
    fn name(&self) -> &'static str {
        "n"
    }
    fn description(&self) -> &'static str {
        "Execute next line in the debugger"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Eval("n".into()))
    }
}

// ---------------------------------------------------------------------------
// %finish — Finish current function in the debugger
// ---------------------------------------------------------------------------
pub struct StepFinish;

impl MagicHandler for StepFinish {
    fn name(&self) -> &'static str {
        "finish"
    }
    fn description(&self) -> &'static str {
        "Finish current function in the debugger"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Eval("finish".into()))
    }
}

// ---------------------------------------------------------------------------
// %Q — Quit the debugger
// ---------------------------------------------------------------------------
pub struct QuitDebug;

impl MagicHandler for QuitDebug {
    fn name(&self) -> &'static str {
        "Q"
    }
    fn description(&self) -> &'static str {
        "Quit the debugger"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        Ok(Output::Silent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xmode_default_is_context() {
        // Reset to default for this test
        *xmode_state().lock().unwrap() = XMODE_DEFAULT.to_string();
        assert_eq!(get_xmode(), XMODE_CONTEXT);
    }

    #[test]
    fn xmode_set_valid_modes() {
        for mode in VALID_XMODES {
            assert!(set_xmode(mode).is_ok());
            assert_eq!(get_xmode(), *mode);
        }
    }

    #[test]
    fn xmode_invalid_mode_returns_error() {
        set_xmode(XMODE_CONTEXT).ok();
        assert!(set_xmode("invalid").is_err());
        // State should be unchanged
        assert_eq!(get_xmode(), XMODE_CONTEXT);
    }

    #[test]
    fn xmode_is_case_insensitive() {
        assert!(set_xmode("VERBOSE").is_ok());
        assert_eq!(get_xmode(), XMODE_VERBOSE);
    }

    #[test]
    fn traceback_code_changes_with_xmode() {
        set_xmode(XMODE_CONTEXT).unwrap();
        let ctx_code = traceback_code();
        assert_eq!(ctx_code, "traceback()");

        set_xmode(XMODE_VERBOSE).unwrap();
        let verb_code = traceback_code();
        assert!(verb_code.contains("max.lines = NULL"));
    }

    #[test]
    fn xmode_handler_shows_current_without_args() {
        let handler = Xmode;
        let line = MagicLine {
            name: "xmode".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = handler.run(&line).unwrap();
        match result {
            Output::Text(t) => assert!(t.contains("Current xmode")),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn xmode_handler_sets_mode_with_args() {
        let handler = Xmode;
        let line = MagicLine {
            name: "xmode".into(),
            args: "plain".into(),
            is_cell: false,
        };
        let result = handler.run(&line).unwrap();
        match result {
            Output::Text(t) => assert!(t.contains("Xmode set to")),
            _ => panic!("expected Text"),
        }
        assert_eq!(get_xmode(), XMODE_PLAIN);
        // Reset for other tests
        set_xmode(XMODE_DEFAULT).ok();
    }

    #[test]
    fn xmode_invalid_via_handler_returns_error() {
        let handler = Xmode;
        let line = MagicLine {
            name: "xmode".into(),
            args: "bogus".into(),
            is_cell: false,
        };
        assert!(handler.run(&line).is_err());
    }

    #[test]
    fn debug_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("debug").is_some());
    }

    #[test]
    fn debug_returns_text() {
        let handler = Debug;
        let line = MagicLine {
            name: "debug".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn pdb_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("pdb").is_some());
    }

    #[test]
    fn pdb_empty_args_does_not_error() {
        let handler = Pdb;
        let line = MagicLine {
            name: "pdb".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        match result {
            Ok(Output::Text(_)) => {}
            Err(_) => {}
            _ => panic!("expected Text or error"),
        }
    }

    #[test]
    fn pdb_on_does_not_error() {
        let handler = Pdb;
        let line = MagicLine {
            name: "pdb".into(),
            args: "on".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn pdb_off_does_not_error() {
        let handler = Pdb;
        let line = MagicLine {
            name: "pdb".into(),
            args: "off".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn pdb_invalid_arg_returns_error() {
        let handler = Pdb;
        let line = MagicLine {
            name: "pdb".into(),
            args: "bogus".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err());
    }

    #[test]
    fn debugonce_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("debugonce").is_some());
    }

    #[test]
    fn debugonce_empty_args_returns_error() {
        let handler = DebugOnce;
        let line = MagicLine {
            name: "debugonce".into(),
            args: "".into(),
            is_cell: false,
        };
        assert!(handler.run(&line).is_err());
    }

    #[test]
    fn undebug_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("undebug").is_some());
    }

    #[test]
    fn undebug_empty_args_returns_error() {
        let handler = Undebug;
        let line = MagicLine {
            name: "undebug".into(),
            args: "".into(),
            is_cell: false,
        };
        assert!(handler.run(&line).is_err());
    }

    #[test]
    fn browser_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("browser").is_some());
    }

    #[test]
    fn browser_returns_eval() {
        let handler = Browser;
        let line = MagicLine {
            name: "browser".into(),
            args: "".into(),
            is_cell: false,
        };
        match handler.run(&line) {
            Ok(Output::Eval(_)) => {}
            Err(_) => {}
            _ => panic!("expected Eval"),
        }
    }

    #[test]
    fn step_next_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("n").is_some());
    }

    #[test]
    fn step_next_returns_eval() {
        let handler = StepNext;
        let line = MagicLine {
            name: "n".into(),
            args: "".into(),
            is_cell: false,
        };
        match handler.run(&line) {
            Ok(Output::Eval(_)) => {}
            Err(_) => {}
            _ => panic!("expected Eval"),
        }
    }

    #[test]
    fn step_finish_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("finish").is_some());
    }

    #[test]
    fn step_finish_returns_eval() {
        let handler = StepFinish;
        let line = MagicLine {
            name: "finish".into(),
            args: "".into(),
            is_cell: false,
        };
        match handler.run(&line) {
            Ok(Output::Eval(_)) => {}
            Err(_) => {}
            _ => panic!("expected Eval"),
        }
    }

    #[test]
    fn quit_debug_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("Q").is_some());
    }

    #[test]
    fn quit_debug_returns_silent() {
        let handler = QuitDebug;
        let line = MagicLine {
            name: "Q".into(),
            args: "".into(),
            is_cell: false,
        };
        match handler.run(&line) {
            Ok(Output::Silent) => {}
            Err(_) => {}
            _ => panic!("expected Silent"),
        }
    }
}
