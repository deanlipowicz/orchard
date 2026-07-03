//! Schema-aware (`$`, `@`, `[[`) completions via R FFI and static data.

use super::{
    extract_bracket_context, extract_dollar_at_context, rank_completions, schema_cache,
    static_dataset_columns, Completion, SchemaEntry, SCHEMA_CACHE_TTL,
};
use crate::r_runtime;
use crate::util::r_string;
use std::time::Instant;

/// Resolve column or slot names for an R object by calling R.
fn resolve_schema(obj_name: &str, op: char) -> Vec<String> {
    let cache_key = format!("{}:{}", obj_name, op);

    {
        let cache = schema_cache().lock().unwrap();
        if let Some(entry) = cache.get(&cache_key)
            && entry.fetched_at.elapsed() < SCHEMA_CACHE_TTL
        {
            return entry.names.clone();
        }
    }

    if op == '$'
        && let Some(cols) = static_dataset_columns(obj_name)
    {
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

    let r_code = if op == '@' {
        format!(
            concat!(
                "local({{ obj <- tryCatch(get({}, envir = .GlobalEnv), error = function(e) NULL);",
                " if (is.null(obj)) return('');",
                " nms <- tryCatch(methods::slotNames(obj), error = function(e) NULL);",
                " if (is.null(nms) || length(nms) == 0) return('');",
                " paste(nms, collapse = '\\n') }})"
            ),
            r_string(obj_name)
        )
    } else {
        format!(
            concat!(
                "local({{ obj <- tryCatch(get({}, envir = .GlobalEnv), error = function(e) NULL);",
                " if (is.null(obj)) return('');",
                " if (inherits(obj, 'R6')) {{",
                "   nms <- tryCatch(ls(envir = obj), error = function(e) NULL);",
                "   if (!is.null(nms)) nms <- nms[!grepl('^\\.__', nms)];",
                " }} else if (methods::is(obj, 'refClass')) {{",
                "   nms <- tryCatch(names(obj), error = function(e) NULL);",
                " }} else {{",
                "   nms <- tryCatch(names(obj), error = function(e) NULL);",
                " }};",
                " if (is.null(nms) || length(nms) == 0) return('');",
                " paste(nms, collapse = '\\n') }})"
            ),
            r_string(obj_name)
        )
    };

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

/// Generate completions for schema-aware contexts (`$`, `@`, `[[`).
///
/// Returns `(completions, span_start)` when a schema context is detected
/// and completions are available, or `None` otherwise.
pub fn schema_completions(line: &str, cursor: usize) -> Option<(Vec<Completion>, usize)> {
    if let Some((obj_name, op, span_start)) = extract_dollar_at_context(line, cursor) {
        let prefix = &line[span_start..cursor.min(line.len())];
        let names = resolve_schema(&obj_name, op);
        let items = rank_completions(&names, prefix);
        if !items.is_empty() {
            return Some((items, span_start));
        }
    }

    if let Some((obj_name, span_start)) = extract_bracket_context(line, cursor) {
        let prefix = &line[span_start..cursor.min(line.len())];
        let names = resolve_schema(&obj_name, '$');
        let items = rank_completions(&names, prefix);
        if !items.is_empty() {
            return Some((items, span_start));
        }
    }

    None
}
