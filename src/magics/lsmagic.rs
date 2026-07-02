use crate::magic::{self, MagicHandler, MagicLine, Output};

/// Handler for `%lsmagic` — list all registered magic commands.
pub struct Lsmagic;

impl MagicHandler for Lsmagic {
    fn name(&self) -> &'static str {
        "lsmagic"
    }
    fn description(&self) -> &'static str {
        "List all registered magic commands"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        let registry = crate::magic::magic_registry();
        let reg = registry.lock().unwrap();
        let names = reg.list_all();
        let mut output = String::from("Available magics:\n");
        for name in names {
            if let Some(handler) = reg.get(name) {
                output.push_str(&format!("  {:<15} {}\n", name, handler.description()));
            }
        }
        output.push_str(&format!("\nTotal: {} handlers", reg.list_all().len()));
        Ok(Output::Text(output))
    }
}
