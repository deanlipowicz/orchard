//! Formula completion for modeling functions (lm, glm, aov, etc.).
//!
//! Detects cursor position inside a formula expression, resolves the
//! `data =` argument, and returns column-name completions from the
//! referenced data frame.

use super::{is_name_char, rank_completions, schema_cache, static_dataset_columns, Completion,
            SCHEMA_CACHE_TTL, SchemaEntry};
use crate::r_runtime;
use crate::util::r_string;
use std::time::Instant;

/// Modeling functions whose first positional argument is a formula
/// accepting a `data = ` argument for column name resolution.
const MODEL_FNS: &[&str] = &["lm", "glm", "aov", "anova", "manova", "nls", "loess", "rlm"];

/// Check if a function name is a known modeling function.
pub(crate) fn is_modeling_fn(name: &str) -> bool {
    let base = name.split("::").last().unwrap_or(name);
    MODEL_FNS.contains(&base)
}

/// Detect if the cursor is inside a formula expression in a modeling
/// function call (lm, glm, aov, etc.).
///
/// Returns `(function_name, span_start)` where `span_start` is the
/// byte position of the current word boundary inside the call.
pub fn formula_context(line: &str, cursor: usize) -> Option<(String, usize)> {
    let text = &line[..cursor.min(line.len())];
    let bytes = text.as_bytes();

    let mut depth = 0i32;
    let mut paren_pos = None;
    for i in (0..bytes.len()).rev() {
        match bytes[i] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    paren_pos = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let paren_pos = paren_pos?;

    let inside_call = &text[paren_pos..];
    if !inside_call.contains('~') {
        return None;
    }

    let before_paren = &text[..paren_pos];
    let fn_start = before_paren
        .rfind(|c: char| !is_name_char(c) && c != ':')
        .map_or(0, |i| i + 1);
    let fn_name = &before_paren[fn_start..];
    if fn_name.is_empty() || !is_modeling_fn(fn_name) {
        return None;
    }
    let first = fn_name.chars().next()?;
    if !first.is_ascii_alphabetic() && first != '.' {
        return None;
    }

    let span_start = text
        .rfind(|c: char| !is_name_char(c) && c != '.' && c != '+' && c != '~' && c != ' ')
        .map_or(0, |i| i + 1);

    Some((fn_name.to_string(), span_start))
}

/// Extract the `data = <expr>` argument from a function call string.
pub(crate) fn extract_data_arg(call_text: &str) -> Option<String> {
    let re =
        regex::Regex::new(r#"data\s*=\s*(?:([[:alpha:].][[:alnum:]._]*)|['\"]([^'\"]+)['\"])"#)
            .ok()?;
    let caps = re.captures(call_text)?;
    caps.get(1)
        .or_else(|| caps.get(2))
        .map(|m| m.as_str().to_string())
}

/// Resolve column names from a data expression for formula completion.
///
/// Checks the static dataset TSV first, then falls through to R FFI.
/// Results are cached in the shared schema cache.
fn resolve_formula_columns(data_expr: &str) -> Vec<String> {
    let cache_key = format!("formula:{}", data_expr);

    {
        let cache = schema_cache().lock().unwrap();
        if let Some(entry) = cache.get(&cache_key)
            && entry.fetched_at.elapsed() < SCHEMA_CACHE_TTL
        {
            return entry.names.clone();
        }
    }

    if let Some(cols) = static_dataset_columns(data_expr) {
        let mut cache = schema_cache().lock().unwrap();
        cache.insert(
            cache_key,
            SchemaEntry {
                names: cols.clone(),
                fetched_at: Instant::now(),
            },
        );
        return cols;
    }

    let r_code = format!(
        concat!(
            "local({{ obj <- tryCatch(get({}, envir = .GlobalEnv), error = function(e) NULL);",
            " if (is.null(obj)) return('');",
            " nms <- tryCatch(names(obj), error = function(e) NULL);",
            " if (is.null(nms) || length(nms) == 0) return('');",
            " paste(nms, collapse = '\\n') }})"
        ),
        r_string(data_expr)
    );

    let result = r_runtime::with_suppressed_stderr(|| {
        r_runtime::eval_string_raw_global(&r_code)
    })
    .unwrap_or_default();

    let names: Vec<String> = result
        .lines()
        .map(String::from)
        .filter(|s| !s.is_empty())
        .collect();

    let mut cache = schema_cache().lock().unwrap();
    cache.insert(
        cache_key,
        SchemaEntry {
            names: names.clone(),
            fetched_at: Instant::now(),
        },
    );
    names
}

/// Generate column-name completions for a formula context.
pub fn formula_completions(line: &str, cursor: usize) -> Option<(Vec<Completion>, usize)> {
    let (_fn_name, span_start) = formula_context(line, cursor)?;

    let text = &line[..cursor.min(line.len())];

    let call_start = text
        .rfind('(')
        .and_then(|i| {
            let before = &text[..i];
            let fn_s = before.rfind(|c: char| !is_name_char(c) && c != ':')?;
            Some(fn_s + 1)
        })
        .unwrap_or(0);
    let call_text = &text[call_start..];

    let data_expr = extract_data_arg(call_text)?;
    let names = resolve_formula_columns(&data_expr);

    if names.is_empty() {
        return None;
    }

    let prefix = &text[span_start..cursor.min(line.len())];
    let items = rank_completions(&names, prefix);

    if items.is_empty() {
        return None;
    }
    Some((items, span_start))
}
