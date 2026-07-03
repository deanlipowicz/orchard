//! Namespace-style (`pkg::fun`) completions from static package symbol data.

use super::{is_name_char, rank_completions, Completion, PACKAGE_SYMBOLS};
use std::{collections::HashMap, sync::OnceLock};

/// Look up exported function names + argument signatures for a package from static TSV.
fn static_package_fn_map() -> &'static HashMap<&'static str, Vec<(&'static str, &'static str)>> {
    static CACHE: OnceLock<HashMap<&'static str, Vec<(&'static str, &'static str)>>> =
        OnceLock::new();
    CACHE.get_or_init(|| {
        let mut m: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
        for line in PACKAGE_SYMBOLS.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(3, '\t');
            if let (Some(pkg), Some(fn_name), Some(args)) =
                (parts.next(), parts.next(), parts.next())
            {
                m.entry(pkg).or_default().push((fn_name, args));
            }
        }
        m
    })
}

/// Detect if the cursor is in a `pkg::fun` completion context.
///
/// Returns `(package_name, span_start)` where `span_start` is the byte
/// position right after `::` where the function-name completion should begin.
pub fn namespace_context(line: &str, cursor: usize) -> Option<(String, usize)> {
    let text = &line[..cursor.min(line.len())];

    if let Some(pos) = text.rfind("::") {
        if pos >= 2 && text.as_bytes()[pos - 1] == b':' {
            return None;
        }
        if pos == 0 || !is_name_char(text.as_bytes()[pos - 1] as char) {
            return None;
        }
        let before = &text[..pos];
        let pkg_start = before
            .rfind(|c: char| !is_name_char(c))
            .map_or(0, |i| i + 1);
        let pkg_name = &before[pkg_start..];
        if pkg_name.is_empty() {
            return None;
        }
        let first = pkg_name.chars().next()?;
        if !first.is_ascii_alphabetic() && first != '.' {
            return None;
        }
        Some((pkg_name.to_string(), pos + 2))
    } else {
        None
    }
}

/// Generate function-name completions for a `pkg::` namespace context.
pub fn namespace_completions(line: &str, cursor: usize) -> Option<(Vec<Completion>, usize)> {
    let (pkg_name, span_start) = namespace_context(line, cursor)?;

    let prefix = &line[span_start..cursor.min(line.len())];
    let fn_map = static_package_fn_map();
    let fns = fn_map.get(pkg_name.as_str())?;

    let names: Vec<String> = fns.iter().map(|(name, _)| name.to_string()).collect();
    let ranked = rank_completions(&names, prefix);

    let fn_args: HashMap<&str, &str> = fns.iter().map(|(n, a)| (*n, *a)).collect();
    let items: Vec<Completion> = ranked
        .into_iter()
        .map(|c| {
            let args = fn_args.get(c.replacement.as_str()).unwrap_or(&"");
            Completion {
                replacement: c.replacement,
                display: if args.is_empty() {
                    c.display
                } else {
                    format!("{}({})", c.display, args)
                },
            }
        })
        .collect();

    if items.is_empty() {
        return None;
    }
    Some((items, span_start))
}
