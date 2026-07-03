use crate::magic::{self, MagicHandler, MagicLine, Output};

/// Handler for `%magic` — print help about the magic system.
pub struct MagicHelp;

impl MagicHandler for MagicHelp {
    fn name(&self) -> &'static str {
        "magic"
    }
    fn description(&self) -> &'static str {
        "Print help about the magic command system"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            Ok(Output::Text(
                concat!(
                    "Magic command system\n",
                    "  %<name>       Line magic — executes on the current line\n",
                    "  %%<name>      Cell magic — executes on following lines\n\n",
                    "Automagic:\n",
                    "  %automagic on|off  Toggle automatic magic detection\n",
                    "  When enabled, magics work without the % prefix\n",
                    "  when the name does not conflict with an R function.\n\n",
                    "System magics:\n",
                    "  %lsmagic  List all registered magics\n",
                    "  %magic    This help message\n\n",
                    "Use %magic <name> for help on a specific magic command.",
                )
                .to_string(),
            ))
        } else {
            // Show help for a specific magic
            let registry = crate::magic::magic_registry();
            let reg = registry.lock().unwrap();
            match reg.get(&line.args) {
                Some(handler) => Ok(Output::Text(format!(
                    "Magic: %{}\n  {}",
                    handler.name(),
                    handler.description()
                ))),
                None => Err(magic::MagicError {
                    message: format!(
                        "No magic command '{}' found. Use %lsmagic to list all.",
                        line.args
                    ),
                }),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magic_help_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("magic").is_some());
    }

    #[test]
    fn magic_help_empty_args() {
        let line = MagicLine {
            name: "magic".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = MagicHelp.run(&line);
        assert!(result.is_ok());
        if let Ok(Output::Text(msg)) = result {
            assert!(
                msg.contains("Magic command system"),
                "should show help: {msg}"
            );
        }
    }

    #[test]
    fn magic_help_unknown_command_errors() {
        let line = MagicLine {
            name: "magic".into(),
            args: "nonexistent_magic_××××".into(),
            is_cell: false,
        };
        let result = MagicHelp.run(&line);
        assert!(result.is_err());
    }
}
