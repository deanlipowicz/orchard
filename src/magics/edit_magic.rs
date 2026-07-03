use crate::magic::{self, MagicHandler, MagicLine, Output};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

static MACROS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

fn macros() -> &'static Mutex<HashMap<String, String>> {
    MACROS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn join_entries(entries: &[crate::history::Entry]) -> String {
    entries
        .iter()
        .map(|e| e.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn create_temp_file(code: &str) -> Result<PathBuf, magic::MagicError> {
    let path = std::env::temp_dir().join(format!("orchard-edit-{}.R", std::process::id()));
    match std::fs::File::create(&path) {
        Ok(mut f) => {
            use std::io::Write;
            write!(f, "{}", code).map_err(|e| magic::MagicError {
                message: e.to_string(),
            })?;
            Ok(path)
        }
        Err(e) => Err(magic::MagicError {
            message: e.to_string(),
        }),
    }
}

fn resolve_edit_target(args: &str) -> Result<(PathBuf, Option<String>), magic::MagicError> {
    if args.is_empty() {
        // %edit with no args → edit last entry
        let entries = crate::magics::history_magics::get_history_snapshot();
        if entries.is_empty() {
            return Err(magic::MagicError {
                message: "History is empty".into(),
            });
        }
        let n = entries.len().saturating_sub(1);
        let code = join_entries(&entries[n..]);
        let path = create_temp_file(&code)?;
        Ok((path, None))
    } else if let Some(rest) = args.strip_prefix('$') {
        // %edit $N — edit entry by absolute index (1-based)
        let n: usize = rest.parse().map_err(|_| magic::MagicError {
            message: format!("Invalid history index: {rest}"),
        })?;
        let entries = crate::magics::history_magics::get_history_snapshot();
        let idx = n.checked_sub(1).ok_or_else(|| magic::MagicError {
            message: "Index must be ≥ 1".into(),
        })?;
        let selected = entries.get(idx).ok_or_else(|| magic::MagicError {
            message: format!("No entry {n} (max {})", entries.len()),
        })?;
        let code = selected.text.clone();
        let path = create_temp_file(&code)?;
        Ok((path, None))
    } else if let Ok(n) = args.parse::<usize>() {
        // %edit N — edit entry N from end (1 = most recent)
        let entries = crate::magics::history_magics::get_history_snapshot();
        if n == 0 || n > entries.len() {
            return Err(magic::MagicError {
                message: format!("Invalid entry: {n} (max {})", entries.len()),
            });
        }
        let code = join_entries(&entries[entries.len().saturating_sub(n)..]);
        let path = create_temp_file(&code)?;
        Ok((path, None))
    } else if args.contains('-') {
        // Range: %edit N-M or -N
        let entries = crate::magics::history_magics::get_history_snapshot();
        let selected =
            crate::magics::history_magics::resolve_range(args, &entries).ok_or_else(|| {
                magic::MagicError {
                    message: format!("Invalid range: '{args}' (max {})", entries.len()),
                }
            })?;
        let code = join_entries(&selected);
        let path = create_temp_file(&code)?;
        Ok((path, None))
    } else {
        // %edit <filename>
        let path = PathBuf::from(args);
        if path.exists() {
            let code = std::fs::read_to_string(&path).unwrap_or_default();
            Ok((path, Some(code)))
        } else {
            Err(magic::MagicError {
                message: format!("File not found: {args}"),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// %macro — Store/edit named history snippet
// ---------------------------------------------------------------------------
pub struct Macro;

impl MagicHandler for Macro {
    fn name(&self) -> &'static str {
        "macro"
    }
    fn description(&self) -> &'static str {
        "Store a named history snippet: %macro <name> <- <code>, %macro <name> to recall"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        if args.is_empty() {
            // List macros
            let map = macros().lock().unwrap();
            if map.is_empty() {
                return Ok(Output::Text("(no macros defined)\n".into()));
            }
            let mut out = String::from("Macros:\n");
            let mut names: Vec<_> = map.keys().collect();
            names.sort();
            for name in names {
                if let Some(code) = map.get(name) {
                    let preview = if code.len() > 60 {
                        format!("{}...", &code[..57])
                    } else {
                        code.clone()
                    };
                    out.push_str(&format!("  {}: {}\n", name, preview));
                }
            }
            Ok(Output::Text(out))
        } else if let Some((name, code)) = args.split_once("<-") {
            // Store: %macro name <- code
            let name = name.trim();
            let code = code.trim();
            if name.is_empty() || code.is_empty() {
                return Err(magic::MagicError {
                    message: "Usage: %macro <name> <- <code>".into(),
                });
            }
            macros()
                .lock()
                .unwrap()
                .insert(name.to_string(), code.to_string());
            Ok(Output::Text(format!(
                "Stored macro '{name}' ({} chars)\n",
                code.len()
            )))
        } else {
            // Recall: %macro name — evaluate and return
            let map = macros().lock().unwrap();
            match map.get(args) {
                Some(code) => {
                    // Evaluate the stored code in R
                    crate::r_runtime::eval_string_raw_global(code).map_err(|e| {
                        magic::MagicError {
                            message: e.to_string(),
                        }
                    })?;
                    Ok(Output::Text(format!("[macro {}]\n", args)))
                }
                None => Err(magic::MagicError {
                    message: format!("No macro '{args}'. Use %macro to list."),
                }),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// %edit — Open R code in external editor
// ---------------------------------------------------------------------------
pub struct Edit;

impl MagicHandler for Edit {
    fn name(&self) -> &'static str {
        "edit"
    }
    fn description(&self) -> &'static str {
        "Open R code in external editor (vim by default)"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();
        let (path, backup) = resolve_edit_target(args)?;
        let editor = std::env::var("R_EDITOR")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vim".to_string());

        // Spawn the editor and wait for it to close
        let status = Command::new(&editor)
            .arg(&path)
            .status()
            .map_err(|e| magic::MagicError {
                message: format!("Failed to launch editor '{editor}': {e}"),
            })?;

        if !status.success() {
            return Err(magic::MagicError {
                message: format!("Editor '{editor}' exited with error"),
            });
        }

        // Read the edited file and evaluate it in R
        let edited = std::fs::read_to_string(&path).unwrap_or_default();
        if !edited.trim().is_empty() {
            // If we had a backup code (editing a filename directly), compare
            if let Some(ref original) = backup
                && edited.trim() == original.trim()
            {
                return Ok(Output::Text("(no changes)\n".into()));
            }
            crate::r_runtime::eval_string_raw_global(&edited).map_err(|e| magic::MagicError {
                message: e.to_string(),
            })?;
            Ok(Output::Text(format!("Sourced edited file ({editor})\n")))
        } else {
            Ok(Output::Text("(empty file, nothing sourced)\n".into()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- join_entries tests ---

    #[test]
    fn join_entries_single() {
        let entries = vec![crate::history::Entry {
            text: "1 + 1".into(),
            mode: String::new(),
            cwd: None,
        }];
        assert_eq!(join_entries(&entries), "1 + 1");
    }

    #[test]
    fn join_entries_multiple() {
        let entries = vec![
            crate::history::Entry {
                text: "a".into(),
                mode: String::new(),
                cwd: None,
            },
            crate::history::Entry {
                text: "b".into(),
                mode: String::new(),
                cwd: None,
            },
        ];
        assert_eq!(join_entries(&entries), "a\nb");
    }

    #[test]
    fn join_entries_empty() {
        assert_eq!(join_entries(&[]), "");
    }

    // --- resolve_edit_target tests ---

    #[test]
    fn resolve_empty_args_without_history() {
        // With no history entries, empty args should return error.
        // Note: get_history_snapshot() returns the actual history,
        // which is empty in unit tests → tests the P0.2 fix path.
        let result = resolve_edit_target("");
        assert!(result.is_err(), "empty history should error");
        let msg = result.unwrap_err().message;
        assert!(msg.contains("empty"), "expected 'empty' in error: {msg}");
    }

    #[test]
    fn resolve_file_not_found() {
        let result = resolve_edit_target("/tmp/orchard_nonexistent_testfile.R");
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("not found"), "expected 'not found': {msg}");
    }

    #[test]
    fn resolve_dollar_absolute_index_parsing() {
        let result = resolve_edit_target("$abc");
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("Invalid"), "expected parse error: {msg}");
    }

    #[test]
    fn resolve_dollar_zero_index() {
        let result = resolve_edit_target("$0");
        assert!(result.is_err());
        let msg = result.unwrap_err().message;
        assert!(msg.contains("≥ 1"), "expected ≥1 error: {msg}");
    }

    #[test]
    fn resolve_range_contains_dash() {
        // Verify contains('-') path is reached (P0.1 regression)
        let result = resolve_edit_target("1-3");
        assert!(result.is_err(), "range should fail without history");
    }

    #[test]
    fn resolve_negative_range() {
        // P0.1 regression: -N should be handled by the range branch
        let result = resolve_edit_target("-5");
        assert!(result.is_err(), "negative range should not panic");
    }

    #[test]
    fn resolve_numeric_zero() {
        let result = resolve_edit_target("0");
        assert!(result.is_err());
    }

    #[test]
    fn resolve_numeric_out_of_bounds() {
        let result = resolve_edit_target("999");
        assert!(result.is_err());
    }

    // --- Macro handler tests ---

    #[test]
    fn macro_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("macro").is_some());
    }

    #[test]
    fn macro_list_empty() {
        // Clear macros first
        macros().lock().unwrap().clear();
        let line = MagicLine {
            name: "macro".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = Macro.run(&line);
        assert!(result.is_ok());
        if let Ok(Output::Text(msg)) = result {
            assert!(msg.contains("(no macros"), "expected empty message: {msg}");
        }
    }

    #[test]
    fn macro_store_and_list() {
        macros().lock().unwrap().clear();
        let line = MagicLine {
            name: "macro".into(),
            args: "foo <- print(42)".into(),
            is_cell: false,
        };
        let result = Macro.run(&line);
        assert!(result.is_ok());
        if let Ok(Output::Text(msg)) = result {
            assert!(msg.contains("Stored macro 'foo'"));
        }

        let list = Macro.run(&MagicLine {
            name: "macro".into(),
            args: "".into(),
            is_cell: false,
        });
        assert!(list.is_ok());
        if let Ok(Output::Text(msg)) = list {
            assert!(msg.contains("foo"), "list should contain macro: {msg}");
        }
    }

    #[test]
    fn macro_unknown_returns_error() {
        macros().lock().unwrap().clear();
        let line = MagicLine {
            name: "macro".into(),
            args: "nonexistent".into(),
            is_cell: false,
        };
        let result = Macro.run(&line);
        assert!(result.is_err());
    }

    // --- Edit handler tests ---

    #[test]
    fn edit_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("edit").is_some());
    }

    #[test]
    fn edit_file_not_found_errors() {
        let line = MagicLine {
            name: "edit".into(),
            args: "/tmp/orchard_nonexistent_testfile.R".into(),
            is_cell: false,
        };
        let result = Edit.run(&line);
        assert!(result.is_err());
    }

    #[test]
    #[ignore = "requires R initialization (eval_string_raw_global)"]
    fn edit_empty_args_with_history() {
        // Would test %edit with no args when history has exactly 1 entry (P0.2 fix)
        let line = MagicLine {
            name: "edit".into(),
            args: "".into(),
            is_cell: false,
        };
        let _result = Edit.run(&line);
    }
}
