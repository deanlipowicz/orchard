use crate::{lexer::cursor_in_string, r_runtime::RRuntime};
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub replacement: String,
    pub display: String,
}

const LATEX_SYMBOLS: &str = include_str!("data/latex_symbols.tsv");

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
    packages
        .iter()
        .filter(|p| p.starts_with(prefix))
        .map(|p| Completion {
            replacement: if in_package_context {
                p.clone()
            } else {
                format!("{p}::")
            },
            display: p.clone(),
        })
        .collect()
}

pub fn shell_path_completions(command: &str) -> Vec<Completion> {
    let dirs_only = command.trim_start().starts_with("cd ");
    let (dir, prefix, quoted) = split_path_word(command);
    let expanded = expand_path(&dir);
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

pub fn r_completions(runtime: &mut RRuntime, token: &str) -> anyhow::Result<Vec<Completion>> {
    let code = r_completion_code(token, token.len(), None);
    let raw = runtime.eval_string_raw(&code)?;
    Ok(completions_from_raw(&raw, false))
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
        r_string(line),
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

pub fn installed_packages(runtime: &mut RRuntime) -> anyhow::Result<Vec<String>> {
    let raw = runtime.eval_string_raw("paste(.packages(all.available = TRUE), collapse='\\n')")?;
    Ok(raw.lines().map(ToString::to_string).collect())
}

fn split_path_word(command: &str) -> (String, String, bool) {
    let word = command.split_whitespace().last().unwrap_or("");
    let quoted = word.starts_with('"') || word.starts_with('\'');
    let word = word.trim_matches(['"', '\'']);
    let path = Path::new(word);
    let dir = path.parent().map_or("", |p| p.to_str().unwrap_or(""));
    let prefix = path.file_name().map_or("", |p| p.to_str().unwrap_or(""));
    (dir.to_string(), prefix.to_string(), quoted)
}

fn expand_path(path: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let path = if path == "~" {
        home
    } else if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(path)
    };
    PathBuf::from(expand_vars(&path.to_string_lossy()))
}

fn expand_vars(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            out.push(ch);
            continue;
        }
        let mut name = String::new();
        while chars
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
        {
            name.push(chars.next().unwrap());
        }
        out.push_str(&env::var(name).unwrap_or_default());
    }
    out
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

fn r_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

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
        let root = env::temp_dir().join(format!(
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
}

