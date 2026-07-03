//! %env — List, get, or set environment variables.

use crate::magic::{self, MagicHandler, MagicLine, Output};

pub struct Env;

impl MagicHandler for Env {
    fn name(&self) -> &'static str {
        "env"
    }

    fn description(&self) -> &'static str {
        "List/set/get environment variables"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            let mut vars: Vec<_> = std::env::vars().collect();
            vars.sort_by(|a, b| a.0.cmp(&b.0));
            let mut out = String::new();
            for (k, v) in vars {
                out.push_str(&format!("{}={}\n", k, v));
            }
            Ok(Output::Text(out))
        } else if let Some((key, val)) = args.split_once('=') {
            let _guard = crate::shell::env_lock();
            unsafe {
                std::env::set_var(key.trim(), val.trim());
            }
            Ok(Output::Silent)
        } else {
            match std::env::var(args) {
                Ok(v) => Ok(Output::Text(format!("{}={}\n", args, v))),
                Err(_) => Ok(Output::Text(format!("{}: (not set)\n", args))),
            }
        }
    }
}
