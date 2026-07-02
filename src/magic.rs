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

    // P9 — EDA handlers
    registry.register(Arc::new(crate::magics::eda::Summary));
    registry.register(Arc::new(crate::magics::eda::Glimpse));
    registry.register(Arc::new(crate::magics::eda::Describe));
    registry.register(Arc::new(crate::magics::eda::Missing));
    registry.register(Arc::new(crate::magics::eda::Corr));
    registry.register(Arc::new(crate::magics::eda::Freq));
    registry.register(Arc::new(crate::magics::eda::Compare));
    registry.register(Arc::new(crate::magics::eda::SessionInfo));
    registry.register(Arc::new(crate::magics::inspect::Inspect));

    // P10 — Debug/Config utilities
    registry.register(Arc::new(crate::magics::debug::Xmode));
    registry.register(Arc::new(crate::magics::config::Automagic));
    registry.register(Arc::new(crate::magics::history_magics::Save));

    // P11 — History Replay (v0.4)
    registry.register(Arc::new(crate::magics::history_magics::Rerun));
    registry.register(Arc::new(crate::magics::history_magics::Recall));

    // P12 — Workspace management (v0.4)
    registry.register(Arc::new(crate::magics::workspace::Store));
    registry.register(Arc::new(crate::magics::workspace::Reset));
    registry.register(Arc::new(crate::magics::workspace::Xdel));

    // P13 — Session logging (v0.4)
    registry.register(Arc::new(crate::magics::logging::LogStart));
    registry.register(Arc::new(crate::magics::logging::LogStop));
    registry.register(Arc::new(crate::magics::logging::LogStateCmd));
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Split "name arg1 arg2" into ("name", "arg1 arg2").
fn split_name_args(input: &str) -> (&str, &str) {
    let trimmed = input.trim_start();
    let end = trimmed
        .find(|c: char| c.is_whitespace())
        .unwrap_or(trimmed.len());
    let name = &trimmed[..end];
    let args = trimmed[end..].trim_start();
    (name, args)
}

/// Try to parse a magic command from the input line.
///
/// Returns `None` if the line is not a magic command. When `automagic` is true,
/// lines starting with a registered magic name (not followed by `(`) are also
/// treated as magic commands.
pub fn parse_magic(text: &str, automagic: bool) -> Option<MagicLine> {
    let trimmed = text.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    // Check for `%` or `%%` prefix
    if let Some(rest) = trimmed.strip_prefix("%%") {
        let (name, args) = split_name_args(rest);
        return Some(MagicLine {
            name: name.to_string(),
            args: args.to_string(),
            is_cell: true,
        });
    }
    if let Some(rest) = trimmed.strip_prefix('%') {
        let (name, args) = split_name_args(rest);
        return Some(MagicLine {
            name: name.to_string(),
            args: args.to_string(),
            is_cell: false,
        });
    }

    // Automagic: no `%` prefix, but line starts with a registered magic name
    // and is not an R function call (i.e. not followed by `(`).
    if automagic {
        let (candidate, _rest) = split_name_args(trimmed);
        if !candidate.is_empty() && is_magic_name(candidate) {
            let after_name = &trimmed[candidate.len()..].trim_start();
            if !after_name.starts_with('(') {
                let (name, args) = split_name_args(trimmed);
                return Some(MagicLine {
                    name: name.to_string(),
                    args: args.to_string(),
                    is_cell: false,
                });
            }
        }
    }

    None
}

/// Check if a magic name is registered.
pub fn is_magic_name(name: &str) -> bool {
    let reg = magic_registry().lock().unwrap();
    reg.get(name).is_some()
}

/// Dispatch a magic command to its registered handler.
pub fn dispatch(cmd: &MagicLine) -> Result<Output, MagicError> {
    // Clone the handler Arc out of the registry to release the lock before
    // calling handler.run(), because handlers may also lock the registry
    // (e.g. %lsmagic reads the handler list).
    let handler = {
        let reg = magic_registry().lock().unwrap();
        reg.get(&cmd.name)
            .ok_or_else(|| MagicError {
                message: format!("Unknown magic: {}", cmd.name),
            })?
            .clone()
    };
    handler.run(cmd)
}

/// Register a magic handler by name.
pub fn register_magic(handler: Arc<dyn MagicHandler>) {
    let mut reg = magic_registry().lock().unwrap();
    reg.register(handler);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_percent_prefix_magic() {
        let cmd = parse_magic("%lsmagic", false).unwrap();
        assert_eq!(cmd.name, "lsmagic");
        assert!(cmd.args.is_empty());
        assert!(!cmd.is_cell);
    }

    #[test]
    fn parse_percent_magic_with_args() {
        let cmd = parse_magic("%who data.frame", false).unwrap();
        assert_eq!(cmd.name, "who");
        assert_eq!(cmd.args, "data.frame");
    }

    #[test]
    fn parse_non_magic_returns_none() {
        assert!(parse_magic("1 + 1", false).is_none());
        assert!(parse_magic("ls()", false).is_none());
        assert!(parse_magic("", false).is_none());
    }

    #[test]
    fn parse_magic_with_leading_whitespace() {
        let cmd = parse_magic("  %lsmagic", false).unwrap();
        assert_eq!(cmd.name, "lsmagic");
    }

    #[test]
    fn automagic_enables_prefixless_magic() {
        // The name must be registered for automagic to work; lsmagic is registered by default
        let cmd = parse_magic("lsmagic", true).unwrap();
        assert_eq!(cmd.name, "lsmagic");
        assert!(cmd.args.is_empty());
    }

    #[test]
    fn automagic_does_not_consume_r_function_calls() {
        // If a name is registered but the input looks like an R call (has `(`), skip
        assert!(parse_magic("lsmagic()", true).is_none());
        assert!(parse_magic("lsmagic(x)", true).is_none());
    }

    #[test]
    fn parse_cell_magic() {
        let cmd = parse_magic("%%timeit", false).unwrap();
        assert_eq!(cmd.name, "timeit");
        assert!(cmd.is_cell);
    }

    #[test]
    fn lsmagic_lists_registered_magics() {
        let reg = magic_registry().lock().unwrap();
        let names = reg.list_all();
        assert!(names.contains(&"lsmagic"));
        assert!(names.contains(&"magic"));
    }

    #[test]
    fn dispatch_known_magic_succeeds() {
        let cmd = MagicLine {
            name: "lsmagic".to_string(),
            args: String::new(),
            is_cell: false,
        };
        let result = dispatch(&cmd).unwrap();
        match result {
            Output::Text(_) => {} // lsmagic returns Text listing available magics
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn dispatch_unknown_magic_fails() {
        let cmd = MagicLine {
            name: "nonexistent".to_string(),
            args: String::new(),
            is_cell: false,
        };
        assert!(dispatch(&cmd).is_err());
    }

    #[test]
    fn is_magic_name_works() {
        assert!(is_magic_name("lsmagic"));
        assert!(!is_magic_name("nonexistent_magic_name_12345"));
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
        let err = MagicError {
            message: "something went wrong".into(),
        };
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
