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
        let n = entries.len().saturating_sub(1);
        if n == 0 {
            return Err(magic::MagicError {
                message: "History is empty".into(),
            });
        }
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
        // Range: %edit N-M
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
    } else if args.starts_with('-') {
        // Negative: %edit -N
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
