//! Argument completions for magic commands (%run, %load, %cd, etc.).

use super::{rank_completions, split_path_word, Completion};
use crate::r_runtime;
use std::{fs, path::PathBuf};

/// The kind of argument a magic command expects for completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagicArgKind {
    /// File paths (optionally filtered to .R/.r).
    File,
    /// Directory paths only.
    Dir,
    /// Variable names from the global environment.
    Variable,
}

/// Map a magic command name to its expected argument completion kind.
pub(crate) fn magic_arg_kind(name: &str) -> Option<MagicArgKind> {
    match name {
        "run" | "load" | "edit" | "save" | "pfile" => Some(MagicArgKind::File),
        "cd" | "pushd" | "popd" | "bookmark" => Some(MagicArgKind::Dir),
        "rm" | "clear" | "who" | "whos" | "who_ls" | "objects" | "str" | "head" | "summary"
        | "glimpse" | "skim" | "dim" | "names" | "plot" | "tidy" | "View" | "pdoc" | "pdef"
        | "psource" | "inspect" => Some(MagicArgKind::Variable),
        _ => None,
    }
}

/// Detect if the cursor is inside a magic command argument position.
///
/// Returns `(magic_name, arg_start, kind)` where `arg_start` is the byte
/// position of the first argument character after the magic name and space.
pub fn magic_context(line: &str, cursor: usize) -> Option<(String, usize, MagicArgKind)> {
    let text = &line[..cursor.min(line.len())];

    let trimmed = text.trim_start();
    if !trimmed.starts_with('%') {
        return None;
    }

    let after_pct = &trimmed[1..];
    let space_pos = after_pct.find(char::is_whitespace)?;
    let magic_name = &after_pct[..space_pos];

    if magic_name.is_empty() {
        return None;
    }

    let kind = magic_arg_kind(magic_name)?;

    let leading_offset = text.len() - trimmed.len();
    let arg_start = leading_offset + 1 + space_pos + 1;

    Some((magic_name.to_string(), arg_start, kind))
}

/// Generate completions for the argument of a magic command.
pub fn magic_completions(line: &str, cursor: usize) -> Option<(Vec<Completion>, usize)> {
    let (_magic_name, arg_start, kind) = magic_context(line, cursor)?;

    let completions = match kind {
        MagicArgKind::File => magic_path_completions(arg_start, line, cursor, false, true),
        MagicArgKind::Dir => magic_path_completions(arg_start, line, cursor, true, false),
        MagicArgKind::Variable => {
            let prefix = &line[arg_start..cursor.min(line.len())];
            variable_name_completions(prefix)
        }
    };

    if completions.is_empty() {
        return None;
    }

    Some((completions, arg_start))
}

fn magic_path_completions(
    arg_start: usize,
    line: &str,
    cursor: usize,
    dirs_only: bool,
    r_only: bool,
) -> Vec<Completion> {
    let arg = &line[arg_start..cursor.min(line.len())];
    let (dir, prefix, quoted) = split_path_word(arg);
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
            if r_only && !is_dir && !name.ends_with(".R") && !name.ends_with(".r") {
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

fn variable_name_completions(prefix: &str) -> Vec<Completion> {
    let r_code = r#"
        local({
            vars <- ls(envir = .GlobalEnv)
            if (length(vars) == 0) return("")
            paste(vars, collapse = "\n")
        })
    "#;

    let result = r_runtime::with_suppressed_stderr(|| {
        r_runtime::eval_string_raw_global(r_code)
    })
    .unwrap_or_default();

    let names: Vec<String> = result.lines().map(String::from).collect();
    rank_completions(&names, prefix)
}
