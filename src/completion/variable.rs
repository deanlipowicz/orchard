//! Variable-selector completions from the global R environment.

use super::{Completion, rank_completions};
use crate::r_runtime;
use std::collections::HashMap;

/// Generate variable-selector completions from the global R environment.
///
/// Returns all variables with their class and size metadata, filtered by
/// the optional prefix.
pub fn variable_selector_completions(prefix: &str) -> Vec<Completion> {
    let r_code = r#"
        local({
            vars <- ls(envir = .GlobalEnv)
            if (length(vars) == 0) return("")
            lines <- vapply(vars, function(v) {
                obj <- tryCatch(get(v, envir = .GlobalEnv), error = function(e) NULL)
                if (is.null(obj)) return(paste(v, "NULL", "0 B", sep = "\t"))
                cls <- paste(class(obj), collapse = "/")
                sz <- tryCatch(format(utils::object.size(obj), units = "auto"), error = function(e) "?")
                paste(v, cls, sz, sep = "\t")
            }, character(1))
            paste(lines, collapse = "\n")
        })
    "#;

    let result = r_runtime::with_suppressed_stderr(|| r_runtime::eval_string_raw_global(r_code))
        .unwrap_or_default();

    let mut raw_map: HashMap<String, (String, String)> = HashMap::new();
    for line in result.lines() {
        let mut parts = line.splitn(3, '\t');
        if let Some(name) = parts.next() {
            let cls = parts.next().unwrap_or("").to_string();
            let sz = parts.next().unwrap_or("").to_string();
            raw_map.insert(name.to_string(), (cls, sz));
        }
    }

    let names: Vec<String> = raw_map.keys().cloned().collect();
    let ranked = rank_completions(&names, prefix);

    ranked
        .into_iter()
        .filter_map(|c| {
            let (cls, sz) = raw_map.get(&c.replacement)?;
            Some(Completion {
                replacement: c.replacement,
                display: format!("{}  ({}, {})", c.display, cls, sz),
            })
        })
        .collect()
}
