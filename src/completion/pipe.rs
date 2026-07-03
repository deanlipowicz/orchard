//! Pipe chain (`%>%`) context detection and column-name completions.

use super::{Completion, rank_completions};
use crate::r_runtime;
use crate::util::r_string;

/// Detect a `%>%` pipe completion context.
///
/// Returns the R expression before the last `%>%` when the cursor is in
/// a pipe chain position.
pub fn extract_pipe_context(line: &str, cursor: usize) -> Option<String> {
    let text = &line[..cursor.min(line.len())];
    let text = text.trim_end();

    let pipe_pos = text.rfind("%>%")?;
    let before_pipe = &text[..pipe_pos].trim_end();
    if before_pipe.is_empty() {
        return None;
    }

    Some(before_pipe.to_string())
}

/// Generate completions for a pipe chain (`%>%`) context.
///
/// Evaluates the pipe expression before the last `%>%` and returns column
/// names as completions. Returns `(completions, span_start)` when a pipe
/// context is detected, or `None` otherwise.
pub fn pipe_completions(line: &str, cursor: usize) -> Option<(Vec<Completion>, usize)> {
    let expr = extract_pipe_context(line, cursor)?;

    let r_code = format!(
        concat!(
            "local({{ result <- tryCatch(eval(parse(text = {}), envir = .GlobalEnv),",
            " error = function(e) NULL);",
            " if (is.null(result)) return('');",
            " nms <- tryCatch(names(result), error = function(e) NULL);",
            " if (is.null(nms) || length(nms) == 0) return('');",
            " paste(nms, collapse = '\\n') }})"
        ),
        r_string(&expr)
    );

    let result = r_runtime::with_suppressed_stderr(|| r_runtime::eval_string_raw_global(&r_code))
        .unwrap_or_default();

    let names: Vec<String> = result
        .lines()
        .map(String::from)
        .filter(|s| !s.is_empty())
        .collect();
    if names.is_empty() {
        return None;
    }

    let text = &line[..cursor.min(line.len())];
    let pipe_end = text
        .rfind("%>%")
        .map(|p| p + 3)
        .unwrap_or(cursor.min(line.len()));

    let after_pipe = &text[pipe_end..cursor.min(line.len())];
    let prefix = after_pipe.trim_start();
    let span_start = pipe_end + (after_pipe.len() - prefix.len());

    let items = rank_completions(&names, prefix);

    if items.is_empty() {
        return None;
    }

    Some((items, span_start))
}
