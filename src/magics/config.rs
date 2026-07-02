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
    fn name(&self) -> &'static str { "alias" }
    fn description(&self) -> &'static str { "Define or list command aliases" }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            let map = alias_map().lock().unwrap();
            let mut out = String::new();
            for (k, v) in map.iter() {
                out.push_str(&format!("{} -> {}\n", k, v));
            }
            if out.is_empty() { out = "(no aliases)\n".into(); }
            Ok(Output::Text(out))
        } else if let Some((name, value)) = args.split_once('=') {
            alias_map().lock().unwrap().insert(name.trim().to_string(), value.trim().to_string());
            Ok(Output::Text(format!("Alias: {} -> {}\n", name.trim(), value.trim())))
        } else {
            Err(magic::MagicError { message: "Usage: %alias <name>=<value>".into() })
        }
    }
}

pub struct Unalias;

impl MagicHandler for Unalias {
    fn name(&self) -> &'static str { "unalias" }
    fn description(&self) -> &'static str { "Remove a command alias" }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let name = line.args.trim();
        if name.is_empty() {
            return Err(magic::MagicError { message: "Usage: %unalias <name>".into() });
        }
        alias_map().lock().unwrap().remove(name);
        Ok(Output::Text(format!("Removed alias: {name}\n")))
    }
}

pub struct Config;

impl MagicHandler for Config {
    fn name(&self) -> &'static str { "config" }
    fn description(&self) -> &'static str { "Show or set configuration options" }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            // List known config options with their current values
            let options = [
                "prompt", "browse_prompt", "shell_prompt",
                "auto_match", "auto_indentation", "tab_size",
                "completion_prefix_length", "completion_timeout",
                "auto_suggest", "editing_mode",
            ];
            let mut out = String::from("Configuration:\n");
            for opt in &options {
                let query = format!(
                    r#"cat("{opt} = ", deparse(getOption("radian.{opt}", "unset")), "\n")"#
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
            let set_cmd = format!(r#"options(radian.{name} = {value})"#);
            crate::r_runtime::eval_string_raw_global(&set_cmd)
                .map_err(|e| magic::MagicError { message: e.to_string() })?;
            Ok(Output::Text(format!("Set radian.{name} = {value}\n")))
        } else {
            let name = args.trim();
            let query = format!(
                r#"cat(deparse(getOption("radian.{name}", "<unset>")), "\n")"#
            );
            let val = crate::r_runtime::eval_string_raw_global(&query)
                .map_err(|e| magic::MagicError { message: e.to_string() })?;
            if val.trim() == "\"<unset>\"" || val.trim() == "<unset>" {
                Ok(Output::Text(format!("radian.{name} is not set\n")))
            } else {
                Ok(Output::Text(format!("radian.{name} = {val}")))
            }
        }
    }
}

pub struct Colors;

impl MagicHandler for Colors {
    fn name(&self) -> &'static str { "colors" }
    fn description(&self) -> &'static str { "Show or set color scheme" }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            // Show available schemes
            let schemes = ["light", "dark", "monokai", "solarized-light", "solarized-dark"];
            let current = crate::r_runtime::eval_string_raw_global(
                r#"cat(getOption("radian.color_scheme", "dark"))"#
            ).unwrap_or_default();
            Ok(Output::Text(format!(
                "Current color scheme: {}\nAvailable: {}\n",
                current.trim(),
                schemes.join(", ")
            )))
        } else {
            let scheme = args;
            let set_cmd = format!(r#"options(radian.color_scheme = "{scheme}")"#);
            crate::r_runtime::eval_string_raw_global(&set_cmd)
                .map_err(|e| magic::MagicError { message: e.to_string() })?;
            Ok(Output::Text(format!("Color scheme set to {scheme}\n")))
        }
    }
}
