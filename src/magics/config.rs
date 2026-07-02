use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static ALIAS_MAP: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub fn alias_map() -> &'static Mutex<HashMap<String, String>> {
    ALIAS_MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn expand_aliases(text: &str) -> String {
    let trimmed = text.trim_start();
    if let Some(first_word) = trimmed.split_whitespace().next()
        && let Ok(map) = alias_map().lock()
        && let Some(replacement) = map.get(first_word)
    {
        let lead_len = text.len() - trimmed.len();
        let after_alias = &trimmed[first_word.len()..];
        return format!("{}{}{}", &text[..lead_len], replacement, after_alias);
    }
    text.to_string()
}

pub struct Alias;

impl MagicHandler for Alias {
    fn name(&self) -> &'static str {
        "alias"
    }
    fn description(&self) -> &'static str {
        "Define or list command aliases"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            let map = alias_map().lock().unwrap();
            let mut out = String::new();
            for (k, v) in map.iter() {
                out.push_str(&format!("{} -> {}\n", k, v));
            }
            if out.is_empty() {
                out = "(no aliases)\n".into();
            }
            Ok(Output::Text(out))
        } else if let Some((name, value)) = args.split_once('=') {
            alias_map()
                .lock()
                .unwrap()
                .insert(name.trim().to_string(), value.trim().to_string());
            Ok(Output::Text(format!(
                "Alias: {} -> {}\n",
                name.trim(),
                value.trim()
            )))
        } else {
            Err(magic::MagicError {
                message: "Usage: %alias <name>=<value>".into(),
            })
        }
    }
}

pub struct Unalias;

impl MagicHandler for Unalias {
    fn name(&self) -> &'static str {
        "unalias"
    }
    fn description(&self) -> &'static str {
        "Remove a command alias"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let name = line.args.trim();
        if name.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %unalias <name>".into(),
            });
        }
        alias_map().lock().unwrap().remove(name);
        Ok(Output::Text(format!("Removed alias: {name}\n")))
    }
}

pub struct Config;

impl MagicHandler for Config {
    fn name(&self) -> &'static str {
        "config"
    }
    fn description(&self) -> &'static str {
        "Show or set configuration options"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            // List known config options with their current values
            let options = [
                "prompt",
                "browse_prompt",
                "shell_prompt",
                "auto_match",
                "auto_indentation",
                "tab_size",
                "completion_prefix_length",
                "completion_timeout",
                "auto_suggest",
                "editing_mode",
            ];
            let mut out = String::from("Configuration:\n");
            for opt in &options {
                let query = format!(
                    r#"cat("{opt} = ", deparse(getOption("orchard.{opt}", "unset")), "\n")"#
                );
                match crate::r_runtime::eval_string_raw_global(&query) {
                    Ok(val) => out.push_str(&val),
                    Err(_) => out.push_str(&format!("{opt} = (error)\n")),
                }
            }
            Ok(Output::Text(out))
        } else if let Some((name, value)) = args.split_once('=') {
            let name = name.trim();
            let value = value.trim();
            let set_cmd = format!(r#"options(orchard.{name} = {value})"#);
            crate::r_runtime::eval_string_raw_global(&set_cmd).map_err(|e| magic::MagicError {
                message: e.to_string(),
            })?;
            Ok(Output::Text(format!("Set orchard.{name} = {value}\n")))
        } else {
            let name = args.trim();
            let query = format!(r#"cat(deparse(getOption("orchard.{name}", "<unset>")), "\n")"#);
            let val = crate::r_runtime::eval_string_raw_global(&query).map_err(|e| {
                magic::MagicError {
                    message: e.to_string(),
                }
            })?;
            if val.trim() == "\"<unset>\"" || val.trim() == "<unset>" {
                Ok(Output::Text(format!("orchard.{name} is not set\n")))
            } else {
                Ok(Output::Text(format!("orchard.{name} = {val}")))
            }
        }
    }
}

pub struct Colors;

impl MagicHandler for Colors {
    fn name(&self) -> &'static str {
        "colors"
    }
    fn description(&self) -> &'static str {
        "Show or set color scheme"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            // Show available schemes
            let schemes = [
                "light",
                "dark",
                "monokai",
                "solarized-light",
                "solarized-dark",
            ];
            let current = crate::r_runtime::eval_string_raw_global(
                r#"cat(getOption("orchard.color_scheme", "dark"))"#,
            )
            .unwrap_or_default();
            Ok(Output::Text(format!(
                "Current color scheme: {}\nAvailable: {}\n",
                current.trim(),
                schemes.join(", ")
            )))
        } else {
            let scheme = args;
            let set_cmd = format!(r#"options(orchard.color_scheme = "{scheme}")"#);
            crate::r_runtime::eval_string_raw_global(&set_cmd).map_err(|e| magic::MagicError {
                message: e.to_string(),
            })?;
            Ok(Output::Text(format!("Color scheme set to {scheme}\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Serializes all alias tests so they don't race on the shared `ALIAS_MAP`.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// RAII guard that acquires `TEST_LOCK`, clears the shared `ALIAS_MAP` on
    /// construction, and restores it on drop. Tests hold this guard for their
    /// entire duration so parallel test execution can't interfere.
    struct AliasGuard {
        saved: HashMap<String, String>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl AliasGuard {
        fn new() -> Self {
            let lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let saved = alias_map().lock().unwrap().clone();
            alias_map().lock().unwrap().clear();
            Self { saved, _lock: lock }
        }
    }

    impl Drop for AliasGuard {
        fn drop(&mut self) {
            let mut map = alias_map().lock().unwrap();
            map.clear();
            *map = self.saved.clone();
        }
    }

    fn magic_line(args: &str) -> MagicLine {
        MagicLine {
            name: "alias".into(),
            args: args.into(),
            is_cell: false,
        }
    }

    // --- Alias ---

    #[test]
    fn alias_list_shows_no_aliases_when_empty() {
        let _guard = AliasGuard::new();
        let handler = Alias;
        let result = handler.run(&magic_line(""));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "(no aliases)\n");
        } else {
            panic!("expected Text output");
        }
    }

    #[test]
    fn alias_set_stores_and_reports() {
        let _guard = AliasGuard::new();
        let handler = Alias;
        let result = handler.run(&magic_line("ll=ls -la"));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "Alias: ll -> ls -la\n");
        } else {
            panic!("expected Text output");
        }
        // Verify it was actually stored
        let map = alias_map().lock().unwrap();
        assert_eq!(map.get("ll").map(|s| s.as_str()), Some("ls -la"));
    }

    #[test]
    fn alias_list_shows_existing_aliases() {
        let _guard = AliasGuard::new();
        alias_map()
            .lock()
            .unwrap()
            .insert("g".into(), "git status".into());
        let handler = Alias;
        let result = handler.run(&magic_line(""));
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "g -> git status\n");
        } else {
            panic!("expected Text output");
        }
    }

    #[test]
    fn alias_without_equals_returns_error() {
        let _guard = AliasGuard::new();
        let handler = Alias;
        let result = handler.run(&magic_line("justaname"));
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("Usage"), "error should mention usage: {msg}");
    }

    #[test]
    fn alias_set_trims_whitespace_around_name_and_value() {
        let _guard = AliasGuard::new();
        let handler = Alias;
        let result = handler.run(&magic_line("  ll  =  ls -la  "));
        assert!(result.is_ok());
        let map = alias_map().lock().unwrap();
        assert_eq!(map.get("ll").map(|s| s.as_str()), Some("ls -la"));
    }

    // --- Unalias ---

    #[test]
    fn unalias_empty_returns_usage_error() {
        let _guard = AliasGuard::new();
        let handler = Unalias;
        let line = MagicLine {
            name: "unalias".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("Usage"), "error should mention usage: {msg}");
    }

    #[test]
    fn unalias_removes_existing_alias() {
        let _guard = AliasGuard::new();
        alias_map()
            .lock()
            .unwrap()
            .insert("ll".into(), "ls -la".into());
        let handler = Unalias;
        let line = MagicLine {
            name: "unalias".into(),
            args: "ll".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "Removed alias: ll\n");
        } else {
            panic!("expected Text output");
        }
        // Verify it was actually removed
        assert!(alias_map().lock().unwrap().get("ll").is_none());
    }

    #[test]
    fn unalias_nonexistent_still_reports_removed() {
        let _guard = AliasGuard::new();
        let handler = Unalias;
        let line = MagicLine {
            name: "unalias".into(),
            args: "nonexistent".into(),
            is_cell: false,
        };
        // Unalias uses .remove() which returns Option; the handler always
        // reports "Removed alias" regardless of whether it existed.
        let result = handler.run(&line);
        assert!(result.is_ok());
        if let Ok(Output::Text(text)) = result {
            assert_eq!(text, "Removed alias: nonexistent\n");
        } else {
            panic!("expected Text output");
        }
    }

    // --- expand_aliases ---

    #[test]
    fn expand_aliases_replaces_first_word() {
        let _guard = AliasGuard::new();
        alias_map()
            .lock()
            .unwrap()
            .insert("ll".into(), "ls -la".into());
        assert_eq!(expand_aliases("ll /tmp"), "ls -la /tmp");
    }

    #[test]
    fn expand_aliases_preserves_leading_whitespace() {
        let _guard = AliasGuard::new();
        alias_map()
            .lock()
            .unwrap()
            .insert("ll".into(), "ls -la".into());
        assert_eq!(expand_aliases("  ll /tmp"), "  ls -la /tmp");
    }

    #[test]
    fn expand_aliases_passes_through_unknown() {
        let _guard = AliasGuard::new();
        assert_eq!(expand_aliases("unknown_cmd arg"), "unknown_cmd arg");
    }

    #[test]
    fn expand_aliases_passes_through_empty() {
        let _guard = AliasGuard::new();
        assert_eq!(expand_aliases(""), "");
    }
}
