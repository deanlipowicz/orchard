use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

/// Global logging state.
struct LogState {
    active: bool,
    path: Option<String>,
    file: Option<File>,
}

fn log_state() -> &'static Mutex<LogState> {
    static STATE: OnceLock<Mutex<LogState>> = OnceLock::new();
    STATE.get_or_init(|| {
        Mutex::new(LogState {
            active: false,
            path: None,
            file: None,
        })
    })
}

/// Called by the REPL loop to log a command if logging is active.
pub fn log_command(text: &str) {
    let state = log_state().lock().unwrap();
    if state.active
        && let Some(ref file) = state.file
    {
        let mut f = file;
        let _ = writeln!(f, "{}", text);
        let _ = f.flush();
    }
}

// ---------------------------------------------------------------------------
// %logstart — Start logging session to a file
// ---------------------------------------------------------------------------

pub struct LogStart;

impl MagicHandler for LogStart {
    fn name(&self) -> &'static str {
        "logstart"
    }
    fn description(&self) -> &'static str {
        "Start logging session to a file: %logstart <filename>"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let path = line.args.trim();
        if path.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %logstart <filename>".into(),
            });
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| magic::MagicError {
                message: format!("Cannot open {path}: {e}"),
            })?;

        let mut state = log_state().lock().unwrap();
        state.active = true;
        state.path = Some(path.to_string());
        state.file = Some(file);

        Ok(Output::Text(format!("Logging started to {path}\n")))
    }
}

// ---------------------------------------------------------------------------
// %logstop — Stop logging session
// ---------------------------------------------------------------------------

pub struct LogStop;

impl MagicHandler for LogStop {
    fn name(&self) -> &'static str {
        "logstop"
    }
    fn description(&self) -> &'static str {
        "Stop logging session"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let mut state = log_state().lock().unwrap();
        if !state.active {
            return Ok(Output::Text("(not currently logging)\n".into()));
        }
        let path = state.path.clone().unwrap_or_default();
        state.active = false;
        state.path = None;
        state.file = None;
        Ok(Output::Text(format!("Logging stopped. Saved to {path}\n")))
    }
}

// ---------------------------------------------------------------------------
// %logstate — Show current logging status
// ---------------------------------------------------------------------------

pub struct LogStateCmd;

impl MagicHandler for LogStateCmd {
    fn name(&self) -> &'static str {
        "logstate"
    }
    fn description(&self) -> &'static str {
        "Show current logging status"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let state = log_state().lock().unwrap();
        if state.active {
            let path = state.path.as_deref().unwrap_or("?");
            Ok(Output::Text(format!("Logging is ON → {path}\n")))
        } else {
            Ok(Output::Text("Logging is OFF\n".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logstart_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("logstart").is_some());
    }

    #[test]
    fn logstop_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("logstop").is_some());
    }

    #[test]
    fn logoff_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("logstop").is_some());
    }

    #[test]
    fn logstate_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("logstate").is_some());
    }

    #[test]
    fn logstate_reports_off_when_inactive() {
        let result = LogStateCmd.run(&MagicLine {
            name: "logstate".into(),
            args: "".into(),
            is_cell: false,
        });
        assert!(result.is_ok());
        if let Ok(Output::Text(msg)) = result {
            assert!(msg.contains("OFF"), "should report OFF: {msg}");
        }
    }

    #[test]
    fn logstop_noop_when_not_logging() {
        let result = LogStop.run(&MagicLine {
            name: "logstop".into(),
            args: "".into(),
            is_cell: false,
        });
        assert!(result.is_ok());
    }
}
