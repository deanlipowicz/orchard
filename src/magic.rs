#![deny(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// A parsed magic command from a line of REPL input.
#[derive(Debug, Clone)]
pub struct MagicLine {
    pub name: String,
    pub args: String,
    pub is_cell: bool,
}

/// The result of executing a magic command.
#[derive(Debug)]
pub enum Output {
    Text(String),
    Eval(String),
    DisplayAndEval(String),
    Silent,
}

/// An error that occurred while running a magic command.
#[derive(Debug)]
pub struct MagicError {
    pub message: String,
}

impl std::fmt::Display for MagicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "magic error: {}", self.message)
    }
}

impl std::error::Error for MagicError {}

/// A registered magic command handler.
pub trait MagicHandler: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn run(&self, line: &MagicLine) -> Result<Output, MagicError>;
}

/// Registry of all magic commands.
pub struct MagicRegistry {
    handlers: HashMap<String, Arc<dyn MagicHandler>>,
}

impl Default for MagicRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MagicRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, handler: Arc<dyn MagicHandler>) {
        self.handlers.insert(handler.name().to_string(), handler);
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn MagicHandler>> {
        self.handlers.get(name)
    }

    pub fn list_all(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.handlers.values().map(|h| h.name()).collect();
        names.sort();
        names
    }
}

static MAGIC_REGISTRY: OnceLock<Mutex<MagicRegistry>> = OnceLock::new();

pub fn magic_registry() -> &'static Mutex<MagicRegistry> {
    MAGIC_REGISTRY.get_or_init(|| {
        let mut reg = MagicRegistry::new();
        register_all(&mut reg);
        Mutex::new(reg)
    })
}

pub fn register_all(registry: &mut MagicRegistry) {
    // P0 — Framework built-ins
    registry.register(Arc::new(crate::magics::lsmagic::Lsmagic));
    registry.register(Arc::new(crate::magics::magic_help::MagicHelp));

    // P1 — Shell magics
    registry.register(Arc::new(crate::magics::shell::Pwd));
    registry.register(Arc::new(crate::magics::shell::Env));
    registry.register(Arc::new(crate::magics::shell::Bookmark));
    registry.register(Arc::new(crate::magics::shell::Cd));
    registry.register(Arc::new(crate::magics::shell::Ls));
    registry.register(Arc::new(crate::magics::shell::Sx));
    registry.register(Arc::new(crate::magics::shell::Pushd));
    registry.register(Arc::new(crate::magics::shell::Popd));
    registry.register(Arc::new(crate::magics::shell::Dhist));

    // P2 — Object inspection
    registry.register(Arc::new(crate::magics::inspect::Objects));
    registry.register(Arc::new(crate::magics::inspect::Who));
    registry.register(Arc::new(crate::magics::inspect::Whos));
    registry.register(Arc::new(crate::magics::inspect::WhoLs));
    registry.register(Arc::new(crate::magics::inspect::Rm));
    registry.register(Arc::new(crate::magics::inspect::Clear));
    registry.register(Arc::new(crate::magics::inspect::Str));
    registry.register(Arc::new(crate::magics::inspect::Head));
    registry.register(Arc::new(crate::magics::inspect::Skim));
    registry.register(Arc::new(crate::magics::inspect::Dim));
    registry.register(Arc::new(crate::magics::inspect::Names));
    registry.register(Arc::new(crate::magics::inspect::Plot));
    registry.register(Arc::new(crate::magics::inspect::Tidy));
    registry.register(Arc::new(crate::magics::inspect::View));
    registry.register(Arc::new(crate::magics::inspect::Pdoc));
    registry.register(Arc::new(crate::magics::inspect::Pdef));
    registry.register(Arc::new(crate::magics::inspect::Psource));
    registry.register(Arc::new(crate::magics::inspect::Pfile));

    // P3 — Debugging and timing
    registry.register(Arc::new(crate::magics::debug::Debug));
    registry.register(Arc::new(crate::magics::debug::Pdb));
    registry.register(Arc::new(crate::magics::debug::Traceback));
    registry.register(Arc::new(crate::magics::debug::Where));
    registry.register(Arc::new(crate::magics::debug::Continue));
    registry.register(Arc::new(crate::magics::timing::Time));
    registry.register(Arc::new(crate::magics::timing::TimeIt));
    registry.register(Arc::new(crate::magics::timing::Prun));

    // P4 — History magics
    registry.register(Arc::new(crate::magics::history_magics::Hist));
    registry.register(Arc::new(crate::magics::history_magics::HistN));

    // P5 — Configuration
    registry.register(Arc::new(crate::magics::config::Config));
    registry.register(Arc::new(crate::magics::config::Colors));
    registry.register(Arc::new(crate::magics::config::Alias));
    registry.register(Arc::new(crate::magics::config::Unalias));

    // P6 — Workspace
    registry.register(Arc::new(crate::magics::workspace::Pinfo));
    registry.register(Arc::new(crate::magics::workspace::Pinfo2));

    // P7 — Edit magic
    registry.register(Arc::new(crate::magics::edit_magic::Macro));
    registry.register(Arc::new(crate::magics::edit_magic::Edit));

    // P8 — File execution
    registry.register(Arc::new(crate::magics::file_magics::Run));
    registry.register(Arc::new(crate::magics::file_magics::Load));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_magic_line_basic() {
        let line = MagicLine {
            name: "test".into(),
            args: "".into(),
            is_cell: false,
        };
        assert_eq!(line.name, "test");
    }

    #[test]
    fn test_registry_roundtrip() {
        let mut reg = MagicRegistry::new();
        reg.register(Arc::new(TestHandler));
        assert!(reg.get("test").is_some());
        let names = reg.list_all();
        assert!(names.contains(&"test"));
    }

    #[test]
    fn test_magic_error_display() {
        let err = MagicError { message: "something went wrong".into() };
        assert_eq!(err.to_string(), "magic error: something went wrong");
    }
}

#[cfg(test)]
struct TestHandler;

#[cfg(test)]
impl MagicHandler for TestHandler {
    fn name(&self) -> &'static str {
        "test"
    }
    fn description(&self) -> &'static str {
        "test handler for unit tests"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, MagicError> {
        Ok(Output::Text("test ok".into()))
    }
}
