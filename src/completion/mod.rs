use crate::lexer::cursor_in_string;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub replacement: String,
    pub display: String,
}

const LATEX_SYMBOLS: &str = include_str!("../data/latex_symbols.tsv");
const DATASET_SCHEMAS: &str = include_str!("../data/dataset_schemas.tsv");
pub(crate) const PACKAGE_SYMBOLS: &str = include_str!("../data/package_symbols.tsv");

pub fn latex_completions(prefix: &str) -> Vec<Completion> {
    let symbols = latex_symbols();
    let exact = symbols
        .iter()
        .filter(|(name, _)| *name == prefix)
        .map(|(name, value)| Completion {
            replacement: value.clone(),
            display: name.clone(),
        });
    let prefix_matches = symbols
        .iter()
        .filter(|(name, _)| name.starts_with(prefix) && *name != prefix)
        .map(|(name, value)| Completion {
            replacement: value.clone(),
            display: name.clone(),
        });
    exact.chain(prefix_matches).collect()
}

pub fn package_context(text: &str, cursor: usize) -> bool {
    let before = remove_nested_parens(&text[..cursor.min(text.len())]);
    let before = before.trim_end();
    [
        "library(",
        "library(\"",
        "library('",
        "require(",
        "require(\"",
        "require('",
    ]
    .iter()
    .any(|call| package_call_tail(before, call))
        || ["requireNamespace(\"", "requireNamespace('"]
            .iter()
            .any(|call| package_call_tail(before, call))
}

pub fn package_prefix(text: &str, cursor: usize) -> &str {
    let (start, _) = package_span(text, cursor);
    &text[start..cursor.min(text.len())]
}

pub fn package_span(text: &str, cursor: usize) -> (usize, usize) {
    let before = &text[..cursor.min(text.len())];
    let start = before
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_'))
        .map_or(0, |i| i + 1);
    (start, cursor.min(text.len()))
}

pub fn package_completions(text: &str, cursor: usize, packages: &[String]) -> Vec<Completion> {
    let prefix = package_prefix(text, cursor);
    let in_package_context = package_context(text, cursor) || cursor_in_string(text, cursor);
    rank_completions(packages, prefix)
        .into_iter()
        .map(|c| Completion {
            replacement: if in_package_context {
                c.replacement
            } else {
                format!("{}::", c.replacement)
            },
            display: c.display,
        })
        .collect()
}

pub fn shell_path_completions(command: &str) -> Vec<Completion> {
    let dirs_only = command.trim_start().starts_with("cd ");
    let (dir, prefix, quoted) = split_path_word(command);
    let expanded = PathBuf::from(crate::util::expand_vars(&crate::util::expand_tilde(&dir)));
    let read_dir = if expanded.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        expanded
    };
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&read_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(&prefix) {
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if dirs_only && !is_dir {
                continue;
            }
            let mut replacement = name;
            if is_dir {
                replacement.push('/');
            }
            if !quoted {
                replacement = replacement.replace(' ', "\\ ");
            }
            out.push(Completion {
                display: replacement.clone(),
                replacement,
            });
        }
    }
    out.sort_by(|a, b| a.display.cmp(&b.display));
    out
}

pub fn r_completion_code(line: &str, pos: usize, timeout: Option<f64>) -> String {
    let complete = if let Some(timeout) = timeout.filter(|value| value.is_finite() && *value > 0.0)
    {
        format!(
            concat!(
                "local({{ ",
                "setTimeLimit(elapsed={}, transient=TRUE); ",
                "on.exit(setTimeLimit(cpu=Inf, elapsed=Inf, transient=FALSE), add=TRUE); ",
                "utils:::.completeToken()",
                " }}); "
            ),
            timeout
        )
    } else {
        "utils:::.completeToken(); ".to_string()
    };
    format!(
        concat!(
            "utils:::.assignLinebuffer({}); ",
            "utils:::.assignEnd({}); ",
            "invisible(utils:::.guessTokenFromLine()); ",
            "{}",
            "paste(utils:::.retrieveCompletions(), collapse='\\n')"
        ),
        crate::util::r_string(line),
        pos,
        complete
    )
}

pub fn namespace_completion(text: &str, cursor: usize) -> bool {
    let before = &text[..cursor.min(text.len())];
    before
        .rfind("::")
        .is_some_and(|namespace| namespace >= package_span(text, cursor).0.saturating_sub(2))
}

pub fn completions_from_raw(raw: &str, spaces_around_equals: bool) -> Vec<Completion> {
    raw.lines()
        .filter(|s| !s.ends_with("::"))
        .map(|s| Completion {
            replacement: if spaces_around_equals && *s == *"=" {
                " = ".to_string()
            } else {
                s.to_string()
            },
            display: s.to_string(),
        })
        .collect()
}

pub(crate) fn split_path_word(command: &str) -> (String, String, bool) {
    let word = command.split_whitespace().last().unwrap_or("");
    let quoted = word.starts_with('"') || word.starts_with('\'');
    let word = word.trim_matches(['"', '\'']);
    let path = Path::new(word);
    let dir = path.parent().map_or("", |p| p.to_str().unwrap_or(""));
    let prefix = path.file_name().map_or("", |p| p.to_str().unwrap_or(""));
    (dir.to_string(), prefix.to_string(), quoted)
}

fn latex_symbols() -> &'static [(String, String)] {
    static SYMBOLS: OnceLock<Vec<(String, String)>> = OnceLock::new();
    SYMBOLS.get_or_init(parse_latex_symbols)
}

fn parse_latex_symbols() -> Vec<(String, String)> {
    LATEX_SYMBOLS
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let (command, value) = line.split_once('\t')?;
            Some((command.to_string(), value.to_string()))
        })
        .collect()
}

fn package_call_tail(text: &str, call: &str) -> bool {
    let Some(start) = text.rfind(call) else {
        return false;
    };
    if start > 0
        && text[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return false;
    }
    text[start + call.len()..]
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

/// Remove outermost parentheses when the enclosed content has no nested parens.
///
/// Single-pass using a stack of frames: `(position_in_output, has_inner_paren)`.
/// Parentheses are not written until the matching `)` is seen, so we know
/// whether the pair is necessary.
fn remove_nested_parens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    // Stack of (index_of_deferred_'('_in_out, has_inner_paren_flag)
    let mut stack: Vec<(usize, bool)> = Vec::new();

    for ch in text.chars() {
        match ch {
            '(' => {
                // Defer writing the '(' — it may be dropped if this pair
                // has no inner parens.  Mark the position and a flag
                // indicating no inner parens (yet).
                stack.push((out.len(), false));
            }
            ')' => {
                if let Some((open_pos, has_inner)) = stack.pop() {
                    if has_inner {
                        // Encloses nested parens — keep the pair.
                        // Insert '(' at the deferred position, write ')'
                        // at the current position.
                        out.insert(open_pos, '(');
                        out.push(')');
                    }
                    // else: drop both parens — content already in `out`
                    // without the unnecessary wrapping.
                } else {
                    out.push(')'); // unmatched — preserve as-is
                }
                // Mark outer level (if any) that content with parens
                // was seen inside it.
                if let Some((_, flag)) = stack.last_mut() {
                    *flag = true;
                }
            }
            _ => {
                out.push(ch);
                // Mark all enclosing levels as having inner content
                // (which implies they need their parens if they have
                // any nested paren structure).
                if let Some((_, flag)) = stack.last_mut() {
                    *flag = true;
                }
            }
        }
    }

    // If parens are still open at EOF, insert the deferred '(' chars
    // for every level still on the stack.
    for (open_pos, _) in stack.iter() {
        out.insert(*open_pos, '(');
    }

    out
}

// ── Schema-Aware Autocomplete ──────────────────────────────────────────────

/// TTL for cached schema lookups (column names of R objects).
pub(crate) const SCHEMA_CACHE_TTL: Duration = Duration::from_secs(5);

pub(crate) struct SchemaEntry {
    names: Vec<String>,
    fetched_at: Instant,
}

pub(crate) fn schema_cache() -> &'static Mutex<HashMap<String, SchemaEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, SchemaEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '.' || c == '_'
}

/// Case-insensitive fuzzy subsequence match.
///
/// Returns `true` if all characters of `query` appear in `name` in order
/// (not necessarily consecutively). An empty query always matches.
pub fn fuzzy_match(name: &str, query: &str) -> bool {
    let name = name.to_lowercase();
    let query = query.to_lowercase();
    if query.is_empty() {
        return true;
    }
    let mut ni = name.chars().peekable();
    for qc in query.chars() {
        loop {
            match ni.next() {
                Some(nc) if nc == qc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

/// A global fuzzy matcher instance (skim backend).
fn fuzzy_matcher() -> &'static SkimMatcherV2 {
    static MATCHER: OnceLock<SkimMatcherV2> = OnceLock::new();
    MATCHER.get_or_init(SkimMatcherV2::default)
}

/// Rank and score a set of candidate names against a prefix.
///
/// Uses `fuzzy-matcher` (skim backend) for scored fuzzy matching and
/// adds a frequency boost from prior completion history. Returns
/// `Completion` items sorted by descending score (best first).
pub fn rank_completions(names: &[String], prefix: &str) -> Vec<Completion> {
    if names.is_empty() || prefix.is_empty() {
        return names
            .iter()
            .map(|n| Completion {
                replacement: n.clone(),
                display: n.clone(),
            })
            .collect();
    }

    let matcher = fuzzy_matcher();
    let mut scored: Vec<(f64, &String)> = names
        .iter()
        .filter_map(|n| {
            matcher
                .fuzzy_match(n, prefix)
                .map(|score| (score as f64 + crate::frequency::frequency_boost(n), n))
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    scored
        .into_iter()
        .map(|(_, n)| Completion {
            replacement: n.clone(),
            display: n.clone(),
        })
        .collect()
}

/// Detect a `$` or `@` accessor context before the cursor.
///
/// Returns `(object_name, operator, span_start)` where `span_start` is the
/// byte position right after the operator where column-name completion
/// should start replacing.
pub fn extract_dollar_at_context(line: &str, cursor: usize) -> Option<(String, char, usize)> {
    let text = &line[..cursor.min(line.len())];

    // Find the last $ or @ in the text before cursor
    let op_pos = text.rfind(['$', '@'])?;
    let op = text.as_bytes()[op_pos] as char;

    // The operator must be preceded by a valid name character
    if op_pos == 0 || !is_name_char(text.as_bytes()[op_pos - 1] as char) {
        return None;
    }

    // Find the start of the object name before the operator
    let before_op = &text[..op_pos];
    let obj_start = before_op
        .rfind(|c: char| !is_name_char(c))
        .map_or(0, |i| i + 1);

    let obj_name = &before_op[obj_start..];
    if obj_name.is_empty() {
        return None;
    }

    // R identifiers must start with a letter or dot, and contain only
    // alphanumerics, dots, and underscores.
    let first = obj_name.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '.' {
        return None;
    }

    Some((obj_name.to_string(), op, op_pos + 1))
}

/// Detect a `[[` bracket completion context.
///
/// Returns `(object_name, span_start)` where `span_start` is the byte
/// position right after `[[`.
pub fn extract_bracket_context(line: &str, cursor: usize) -> Option<(String, usize)> {
    let text = &line[..cursor.min(line.len())];
    let bytes = text.as_bytes();
    let len = bytes.len();

    if len < 2 {
        return None;
    }

    // Find the last occurrence of [[ in the text before cursor
    // We search for "[[", but must handle the case where cursor is inside
    // a quoted string like df[["partial or df[[partial

    // Find last "[["
    let bracket_start = text.rfind("[[");

    // Also try just "[" — if there's a single [ with no preceding second [,
    // it's not our context
    let bracket_end = bracket_start?;
    if bracket_end + 2 > len {
        return None;
    }

    // Extract the object name before [[
    let before_bracket = &text[..bracket_end];
    let obj_start = before_bracket
        .rfind(|c: char| !is_name_char(c))
        .map_or(0, |i| i + 1);
    let obj_name = &before_bracket[obj_start..];

    // R identifiers must start with a letter or dot
    if obj_name.is_empty() {
        return None;
    }
    let first = obj_name.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '.' {
        return None;
    }

    // Determine what comes after [[ and where the completion span starts.
    // Cases:
    //   df[[        — cursor/trailing whitespace right after [[
    //   df[["col    — opening double-quote + partial column name
    //   df[['col    — opening single-quote + partial column name
    //   df[[42      — numeric (still complete after [[ for edge cases)
    let after_bracket = &text[bracket_end + 2..];

    if after_bracket.is_empty() || after_bracket.trim().is_empty() {
        // Nothing after [[ (or just trailing whitespace) — span starts right after [[
        return Some((obj_name.to_string(), bracket_end + 2));
    }

    // Check for opening quote
    let trimmed = after_bracket.trim_start();
    let quote_offset = after_bracket.len() - trimmed.len();
    let first_content = trimmed.chars().next();

    match first_content {
        Some('"') | Some('\'') => {
            // Quoted column name — span starts after the opening quote
            let span_start = bracket_end + 2 + quote_offset + 1;
            Some((obj_name.to_string(), span_start))
        }
        _ => {
            // Unquoted content after [[ (e.g. numeric index or unquoted name)
            // Still usable — span starts right after [[
            Some((obj_name.to_string(), bracket_end + 2))
        }
    }
}

/// Look up column names for a known dataset from static TSV (no R call).
pub(crate) fn static_dataset_columns(obj_name: &str) -> Option<Vec<String>> {
    static CACHE: OnceLock<HashMap<&'static str, Vec<&'static str>>> = OnceLock::new();
    let map = CACHE.get_or_init(|| {
        let mut m: HashMap<&str, Vec<&str>> = HashMap::new();
        for line in DATASET_SCHEMAS.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.splitn(3, '\t');
            if let (Some(dataset), Some(col), _) = (parts.next(), parts.next(), parts.next()) {
                m.entry(dataset).or_default().push(col);
            }
        }
        m
    });
    map.get(obj_name)
        .map(|cols| cols.iter().map(|s| s.to_string()).collect())
}

mod namespace;
pub use namespace::*;

mod schema;
pub use schema::*;

mod pipe;
pub use pipe::*;

mod variable;
pub use variable::*;

mod magic;
#[cfg(test)]
pub(crate) use magic::magic_arg_kind;
pub use magic::*;

mod spellcheck;
pub use spellcheck::*;

mod function_args;
pub use function_args::*;

mod formula;
pub use formula::*;
#[cfg(test)]
pub(crate) use formula::{extract_data_arg, is_modeling_fn};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn detects_package_contexts() {
        assert!(package_context("library(ba", 10));
        assert!(package_context("requireNamespace(\"ba", 20));
        assert!(!package_context("print(ba", 8));
    }

    #[test]
    fn completes_package_names_or_namespace() {
        let pkgs = vec!["base".to_string(), "boot".to_string()];
        assert_eq!(
            package_completions("library(ba", 10, &pkgs)[0].replacement,
            "base"
        );
        assert_eq!(package_completions("ba", 2, &pkgs)[0].replacement, "base::");
    }

    #[test]
    fn completes_latex_symbols() {
        assert_eq!(latex_completions("\\alp")[0].replacement, "α");
    }

    #[test]
    fn latex_table_has_full_upstream_count() {
        let symbols = latex_symbols();
        // Upstream file has 1983 entries; the parser must load all of them
        assert!(
            symbols.len() >= 1980,
            "Expected ~1983 LaTeX symbols, got {}",
            symbols.len()
        );
    }

    #[test]
    fn latex_completions_work_for_common_symbols() {
        // Verify that common LaTeX symbols are available via prefix match
        assert_eq!(latex_completions("\\alpha")[0].replacement, "α");
        assert_eq!(latex_completions("\\beta")[0].replacement, "β");
        assert_eq!(latex_completions("\\gamma")[0].replacement, "γ");
        assert_eq!(latex_completions("\\pi")[0].replacement, "π");
        assert_eq!(latex_completions("\\sum")[0].replacement, "∑");
        assert_eq!(latex_completions("\\int")[0].replacement, "∫");
        assert_eq!(latex_completions("\\infty")[0].replacement, "∞");
        assert_eq!(latex_completions("\\ne")[0].replacement, "≠");
        assert_eq!(latex_completions("\\pm")[0].replacement, "±");
        assert_eq!(latex_completions("\\partial")[0].replacement, "∂");
    }

    #[test]
    fn r_completion_code_uses_full_line_and_cursor() {
        let code = r_completion_code("mean(x)", 4, None);
        assert!(code.contains("utils:::.assignLinebuffer(\"mean(x)\")"));
        assert!(code.contains("utils:::.assignEnd(4)"));
        assert!(code.contains("utils:::.guessTokenFromLine()"));
    }

    #[test]
    fn raw_r_completions_filter_namespace_and_space_equals() {
        assert_eq!(
            completions_from_raw("mean\nbase::\n=\n", true),
            vec![
                Completion {
                    replacement: "mean".into(),
                    display: "mean".into(),
                },
                Completion {
                    replacement: " = ".into(),
                    display: "=".into(),
                },
            ]
        );
    }

    #[test]
    fn completes_shell_directories_only_for_cd() {
        let root = std::env::temp_dir().join(format!(
            "orchard-complete-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("alpha dir")).unwrap();
        fs::write(root.join("alpha.txt"), "").unwrap();
        let got = shell_path_completions(&format!("cd {}/alp", root.display()));
        assert_eq!(
            got,
            vec![Completion {
                replacement: "alpha\\ dir/".into(),
                display: "alpha\\ dir/".into()
            }]
        );
    }

    // ── Schema-aware context detection tests ──────────────────────────────

    #[test]
    fn detects_dollar_context_simple() {
        let (name, op, span) = extract_dollar_at_context("df$", 3).unwrap();
        assert_eq!(name, "df");
        assert_eq!(op, '$');
        assert_eq!(span, 3);
    }

    #[test]
    fn detects_dollar_context_with_partial_column() {
        let (name, op, span) = extract_dollar_at_context("df$col", 6).unwrap();
        assert_eq!(name, "df");
        assert_eq!(op, '$');
        assert_eq!(span, 3);
    }

    #[test]
    fn detects_dollar_context_with_dotted_name() {
        let (name, op, span) = extract_dollar_at_context("my.data$", 8).unwrap();
        assert_eq!(name, "my.data");
        assert_eq!(op, '$');
        assert_eq!(span, 8);
    }

    #[test]
    fn detects_at_context() {
        let (name, op, span) = extract_dollar_at_context("myobj@", 6).unwrap();
        assert_eq!(name, "myobj");
        assert_eq!(op, '@');
        assert_eq!(span, 6);
    }

    #[test]
    fn rejects_dollar_without_preceding_name() {
        assert!(extract_dollar_at_context("$", 1).is_none());
        assert!(extract_dollar_at_context("5$", 2).is_none());
    }

    #[test]
    fn detects_bracket_context() {
        let (name, span) = extract_bracket_context("df[[", 4).unwrap();
        assert_eq!(name, "df");
        assert_eq!(span, 4);
    }

    #[test]
    fn rejects_single_bracket() {
        assert!(extract_bracket_context("df[", 3).is_none());
    }

    #[test]
    fn detects_bracket_with_double_quote() {
        let (name, span) = extract_bracket_context("df[[\"col", 8).unwrap();
        assert_eq!(name, "df");
        assert_eq!(span, 5); // after [["
    }

    #[test]
    fn detects_bracket_with_single_quote() {
        let (name, span) = extract_bracket_context("df[['col", 8).unwrap();
        assert_eq!(name, "df");
        assert_eq!(span, 5); // after [['
    }

    #[test]
    fn detects_bracket_with_content_after() {
        // Unquoted content after [[ (numeric index, etc.)
        let (name, span) = extract_bracket_context("df[[42", 6).unwrap();
        assert_eq!(name, "df");
        assert_eq!(span, 4); // right after [[
    }

    #[test]
    fn detects_bracket_with_trailing_whitespace() {
        let (name, span) = extract_bracket_context("df[[ ", 5).unwrap();
        assert_eq!(name, "df");
        assert_eq!(span, 4); // right after [[
    }

    #[test]
    fn test_pipe_context_simple() {
        let expr = extract_pipe_context("df %>% ", 7).unwrap();
        assert_eq!(expr, "df");
    }

    #[test]
    fn detects_pipe_context_with_filter() {
        let expr = extract_pipe_context("df %>% filter(x > 1) %>% ", 26).unwrap();
        assert_eq!(expr, "df %>% filter(x > 1)");
    }

    #[test]
    fn rejects_empty_pipe_context() {
        assert!(extract_pipe_context("%>% ", 4).is_none());
    }

    #[test]
    fn is_name_char_accepts_valid_chars() {
        assert!(is_name_char('a'));
        assert!(is_name_char('Z'));
        assert!(is_name_char('0'));
        assert!(is_name_char('.'));
        assert!(is_name_char('_'));
    }

    #[test]
    fn is_name_char_rejects_invalid_chars() {
        assert!(!is_name_char('$'));
        assert!(!is_name_char('@'));
        assert!(!is_name_char(' '));
        assert!(!is_name_char('-'));
    }

    #[test]
    fn schema_completions_no_context() {
        assert!(schema_completions("mean(x)", 7).is_none());
        assert!(schema_completions("library(dplyr)", 15).is_none());
    }

    // ── Fuzzy matching tests ──────────────────────────────────────────────

    #[test]
    fn fuzzy_match_exact() {
        assert!(fuzzy_match("select", "select"));
    }

    #[test]
    fn fuzzy_match_case_insensitive() {
        assert!(fuzzy_match("SELECT", "select"));
        assert!(fuzzy_match("select", "SELECT"));
    }

    #[test]
    fn fuzzy_match_substring() {
        assert!(fuzzy_match("select", "sel"));
        assert!(fuzzy_match("select", "ect"));
    }

    #[test]
    fn fuzzy_match_skip_chars() {
        // "sl" matches "select" — s...l
        assert!(fuzzy_match("select", "sl"));
        // "slt" matches "select" — s...l...ect
        assert!(fuzzy_match("select", "slt"));
    }

    #[test]
    fn fuzzy_match_no_match() {
        assert!(!fuzzy_match("select", "xyz"));
        assert!(!fuzzy_match("select", "sx"));
    }

    #[test]
    fn fuzzy_match_empty_query() {
        assert!(fuzzy_match("anything", ""));
    }

    #[test]
    fn fuzzy_match_underscore_and_dots() {
        assert!(fuzzy_match("my_column_name", "mcn"));
        assert!(fuzzy_match("my.column.name", "mcn"));
    }

    // ── Magic context tests ──────────────────────────────────────────────

    #[test]
    fn detects_magic_context_run() {
        let (name, start, kind) = magic_context("%run /path/to/file", 18).unwrap();
        assert_eq!(name, "run");
        assert_eq!(start, 5);
        assert_eq!(kind, MagicArgKind::File);
    }

    #[test]
    fn detects_magic_context_cd() {
        let (name, start, kind) = magic_context("%cd mydir", 9).unwrap();
        assert_eq!(name, "cd");
        assert_eq!(start, 4);
        assert_eq!(kind, MagicArgKind::Dir);
    }

    #[test]
    fn detects_magic_context_rm() {
        let (name, start, kind) = magic_context("%rm mtcars", 10).unwrap();
        assert_eq!(name, "rm");
        assert_eq!(start, 4);
        assert_eq!(kind, MagicArgKind::Variable);
    }

    #[test]
    fn detects_magic_context_with_leading_space() {
        let (name, start, kind) = magic_context("  %run file.R", 15).unwrap();
        assert_eq!(name, "run");
        assert_eq!(start, 7);
        assert_eq!(kind, MagicArgKind::File);
    }

    #[test]
    fn rejects_magic_without_args() {
        // Cursor still in the magic name (no space yet)
        assert!(magic_context("%run", 4).is_none());
    }

    #[test]
    fn rejects_unknown_magic() {
        assert!(magic_context("%nonexistent arg", 16).is_none());
    }

    #[test]
    fn rejects_non_magic_line() {
        assert!(magic_context("mean(x)", 7).is_none());
    }

    #[test]
    fn magic_arg_kind_covers_all_common_magics() {
        for name in &["run", "load", "edit", "save", "pfile"] {
            assert_eq!(magic_arg_kind(name), Some(MagicArgKind::File));
        }
        for name in &["cd", "pushd", "popd"] {
            assert_eq!(magic_arg_kind(name), Some(MagicArgKind::Dir));
        }
        for name in &[
            "rm", "clear", "who", "str", "head", "summary", "glimpse", "dim", "names", "inspect",
        ] {
            assert_eq!(magic_arg_kind(name), Some(MagicArgKind::Variable));
        }
    }

    // ── Function call context tests ───────────────────────────────────────

    #[test]
    fn detects_function_call_simple() {
        let (name, start) = function_call_context("mean(", 5).unwrap();
        assert_eq!(name, "mean");
        assert_eq!(start, 5);
    }

    #[test]
    fn detects_function_call_with_arg() {
        let (name, start) = function_call_context("mean(x, ", 8).unwrap();
        assert_eq!(name, "mean");
        assert_eq!(start, 5);
    }

    #[test]
    fn detects_function_call_namespaced() {
        // Cursor inside the parens (after a comma and space)
        let (name, start) = function_call_context("stats::lm(y ~ x, ", 18).unwrap();
        assert_eq!(name, "stats::lm");
        assert_eq!(start, 10);
    }

    #[test]
    fn detects_function_call_nested_inner() {
        // Cursor right after the inner call's "("
        let (name, start) = function_call_context("mean(x, sd(", 11).unwrap();
        assert_eq!(name, "sd");
        assert_eq!(start, 11);
    }

    #[test]
    fn detects_function_call_nested_outer() {
        // Cursor after the outer call's second comma
        let (name, start) = function_call_context("mean(x, sd(y), ", 15).unwrap();
        assert_eq!(name, "mean");
        assert_eq!(start, 5);
    }

    #[test]
    fn rejects_non_function_context() {
        assert!(function_call_context("x + 1", 5).is_none());
        assert!(function_call_context("", 0).is_none());
    }

    #[test]
    fn rejects_anonymous_function() {
        assert!(function_call_context("(function(x) x)(5)", 18).is_none());
    }

    // ── Levenshtein distance tests ────────────────────────────────────────

    #[test]
    fn levenshtein_exact_match() {
        assert_eq!(levenshtein_distance("mean", "mean"), 0);
    }

    #[test]
    fn levenshtein_case_insensitive() {
        assert_eq!(levenshtein_distance("Mean", "mean"), 0);
        assert_eq!(levenshtein_distance("MEAN", "mean"), 0);
    }

    #[test]
    fn levenshtein_one_edit() {
        assert_eq!(levenshtein_distance("mean", "meen"), 1); // substitution
        assert_eq!(levenshtein_distance("mean", "meann"), 1); // insertion
        assert_eq!(levenshtein_distance("mean", "mea"), 1); // deletion
    }

    #[test]
    fn levenshtein_two_edits() {
        assert_eq!(levenshtein_distance("mean", "miin"), 2);
    }

    #[test]
    fn levenshtein_empty_strings() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
    }

    #[test]
    fn levenshtein_completely_different() {
        assert_eq!(levenshtein_distance("abc", "xyz"), 3);
    }

    // ── Formula completion tests ──────────────────────────────────────────

    #[test]
    fn detects_formula_in_lm() {
        // Cursor after the "+ " inside a formula
        let (name, _) = formula_context("lm(mpg ~ cyl + , data = mtcars)", 22).unwrap();
        assert_eq!(name, "lm");
    }

    #[test]
    fn detects_formula_after_tilde() {
        // Cursor at the 'd' of data= — inside the call, after the tilde.
        // "lm(y ~ , data = df)"
        //           ^ cursor=9
        let (name, _) = formula_context("lm(y ~ , data = df)", 9).unwrap();
        assert_eq!(name, "lm");
    }

    #[test]
    fn rejects_non_modeling_fn() {
        assert!(formula_context("mean(x, na.rm = TRUE)", 22).is_none());
    }

    #[test]
    fn rejects_without_tilde() {
        assert!(formula_context("lm(mpg, data = mtcars)", 23).is_none());
    }

    #[test]
    fn rejects_without_parens() {
        assert!(formula_context("lm ", 3).is_none());
    }

    #[test]
    fn extracts_data_arg_unquoted() {
        let result = extract_data_arg("lm(mpg ~ cyl, data = mtcars)");
        assert_eq!(result, Some("mtcars".to_string()));
    }

    #[test]
    fn extracts_data_arg_quoted() {
        let result = extract_data_arg("lm(mpg ~ ., data = \"mtcars\")");
        assert_eq!(result, Some("mtcars".to_string()));
    }

    #[test]
    fn extracts_data_arg_single_quoted() {
        let result = extract_data_arg("lm(mpg ~ ., data = 'mtcars')");
        assert_eq!(result, Some("mtcars".to_string()));
    }

    #[test]
    fn extract_data_arg_fails_when_missing() {
        assert!(extract_data_arg("lm(mpg ~ wt)").is_none());
    }

    #[test]
    fn is_modeling_fn_recognizes_known_fns() {
        assert!(is_modeling_fn("lm"));
        assert!(is_modeling_fn("glm"));
        assert!(is_modeling_fn("aov"));
        assert!(is_modeling_fn("stats::lm"));
        assert!(!is_modeling_fn("mean"));
        assert!(!is_modeling_fn("print"));
    }
}
