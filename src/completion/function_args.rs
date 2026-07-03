//! Argument-name completion for R function calls.
//!
//! Detects cursor position inside a function call `fname(...)` and
//! generates argument-name completions from `formals()`.

use super::{is_name_char, rank_completions, Completion};
use crate::r_runtime;
use std::collections::HashMap;

/// Detect if the cursor is inside a function call `fname(...)`.
///
/// Returns `(function_expression, span_start)` where `span_start` is the
/// byte position right after the opening `(`. The function expression
/// includes the namespace prefix if present (e.g. `stats::lm`).
pub fn function_call_context(line: &str, cursor: usize) -> Option<(String, usize)> {
    let text = &line[..cursor.min(line.len())];
    let bytes = text.as_bytes();

    let mut depth = 0i32;
    for i in (0..bytes.len()).rev() {
        match bytes[i] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    let before_paren = &text[..i];
                    let fn_start = before_paren
                        .rfind(|c: char| !is_name_char(c) && c != ':')
                        .map_or(0, |i| i + 1);
                    let fn_expr = &text[fn_start..i];
                    if fn_expr.is_empty() {
                        return None;
                    }
                    let first = fn_expr.chars().next()?;
                    if !first.is_ascii_alphabetic() && first != '.' {
                        return None;
                    }
                    return Some((fn_expr.to_string(), i + 1));
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    None
}

/// Generate argument-name completions for a function call.
///
/// Calls R `formals()` to get argument names and default values for the
/// function at the cursor position. Returns `(completions, span_start)` when
/// inside a function call, or `None` otherwise.
pub fn function_arg_completions(line: &str, cursor: usize) -> Option<(Vec<Completion>, usize)> {
    let (fn_expr, paren_start) = function_call_context(line, cursor)?;

    let after_paren = &line[paren_start..cursor.min(line.len())];
    let prefix = after_paren.trim();

    let r_code = format!(
        concat!(
            "local({{ fmls <- tryCatch(formals({}), error = function(e) NULL);",
            " if (is.null(fmls) || length(fmls) == 0) return('');",
            " nms <- names(fmls);",
            " lines <- vapply(seq_along(fmls), function(i) {{",
            "   nm <- nms[i];",
            "   def <- fmls[[i]];",
            "   default_str <- if (is.symbol(def) && as.character(def) == '') ''",
            "     else paste0(' = ', deparse(def, width.cutoff = 60L)[1]);",
            "   paste0(nm, default_str)",
            " }}, character(1));",
            " paste(lines, collapse = '\\n') }})"
        ),
        fn_expr
    );

    let result = r_runtime::with_suppressed_stderr(|| {
        r_runtime::eval_string_raw_global(&r_code)
    })
    .unwrap_or_default();

    let mut arg_map: HashMap<String, Option<String>> = HashMap::new();
    for raw_line in result.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let (arg_name, default_str) = line.split_once(" = ").unwrap_or((line, ""));
        arg_map.insert(
            arg_name.to_string(),
            if default_str.is_empty() {
                None
            } else {
                Some(default_str.to_string())
            },
        );
    }

    let arg_names: Vec<String> = arg_map.keys().cloned().collect();
    let ranked = rank_completions(&arg_names, prefix);

    let items: Vec<Completion> = ranked
        .into_iter()
        .filter_map(|c| {
            let default_str = arg_map.get(&c.replacement)?;
            let display = match default_str {
                Some(d) => format!("{} = {}", c.replacement, d),
                None => c.replacement.clone(),
            };
            Some(Completion {
                replacement: format!("{} = ", c.replacement),
                display,
            })
        })
        .collect();

    if items.is_empty() {
        return None;
    }

    Some((items, paren_start))
}
