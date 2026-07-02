use crate::history::Entry;
use crate::magic::{self};
use std::io::Write;

pub fn get_history_snapshot() -> Vec<Entry> {
    crate::r_runtime::history_entries_snapshot()
}

/// Resolve a range expression against the given entries.
///
/// Supported formats:
/// - `N-M` or `N:M` — inclusive range (1-based, from end)
/// - `-N` — last N entries
/// - `$N` — absolute index (1-based)
pub fn resolve_range(range: &str, entries: &[Entry]) -> Option<Vec<Entry>> {
    let range = range.trim();
    if range.is_empty() {
        return Some(entries.to_vec());
    }

    // $N — absolute index
    if let Some(n_str) = range.strip_prefix('$') {
        let n: usize = n_str.parse().ok()?;
        let idx = n.checked_sub(1)?;
        entries.get(idx).map(|e| vec![e.clone()])
    } else if let Some(rest) = range.strip_prefix('-') {
        // -N — last N entries
        if rest.is_empty() {
            return None;
        }
        let n: usize = rest.parse().ok()?;
        if n == 0 || n > entries.len() {
            return None;
        }
        Some(entries[entries.len().saturating_sub(n)..].to_vec())
    } else if range.contains('-') || range.contains(':') {
        // N-M or N:M — range from end (1 = most recent)
        let sep = if range.contains('-') { '-' } else { ':' };
        let (a_str, b_str) = range.split_once(sep)?;
        let a: usize = a_str.trim().parse().ok()?;
        let b: usize = b_str.trim().parse().ok()?;
        if a == 0 || b == 0 || a > b || a > entries.len() {
            return None;
        }
        let start = entries.len().saturating_sub(b);
        let end = entries.len().saturating_sub(a.saturating_sub(1));
        if start >= end || start >= entries.len() {
            return None;
        }
        Some(entries[start..end.min(entries.len())].to_vec())
    } else if let Ok(n) = range.parse::<usize>() {
        // Bare number — single entry from end
        if n == 0 || n > entries.len() {
            return None;
        }
        let idx = entries.len().saturating_sub(n);
        entries.get(idx).map(|e| vec![e.clone()])
    } else {
        None
    }
}

pub struct SnapshotFilter {
    pub mode_filter: Option<String>,
    pub pattern: Option<String>,
}

/// Return up to `n` most recent entries matching the given filter.
pub fn recent_entries(filter: &SnapshotFilter, n: usize) -> Vec<Entry> {
    let entries = get_history_snapshot();
    let limit = if n > 0 { n } else { 20 };
    entries
        .into_iter()
        .filter(|e| {
            if let Some(ref m) = filter.mode_filter
                && !e.mode.eq_ignore_ascii_case(m)
            {
                return false;
            }
            if let Some(ref p) = filter.pattern
                && !e.text.contains(p.as_str())
            {
                return false;
            }
            true
        })
        .rev()
        .take(limit)
        .collect()
}

pub fn export_history(file_path: &str, filter: &SnapshotFilter) -> Result<(), magic::MagicError> {
    let entries = recent_entries(filter, 0);
    let mut file = std::fs::File::create(file_path).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    for entry in &entries {
        writeln!(file, "|{}| {}", entry.mode.replace('|', "_"), entry.text).map_err(|e| {
            magic::MagicError {
                message: e.to_string(),
            }
        })?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// %hist — Print history entries
// ---------------------------------------------------------------------------

pub struct Hist;

impl magic::MagicHandler for Hist {
    fn name(&self) -> &'static str {
        "hist"
    }
    fn description(&self) -> &'static str {
        "Print history entries (optional: range, pattern, -N)"
    }
    fn run(&self, line: &magic::MagicLine) -> Result<magic::Output, magic::MagicError> {
        let entries = get_history_snapshot();
        if entries.is_empty() {
            return Ok(magic::Output::Text("(history empty)\n".into()));
        }
        let args = line.args.trim();
        let selected = if args.is_empty() {
            // Default: last 20 entries
            let n = entries.len().min(20);
            entries[entries.len().saturating_sub(n)..].to_vec()
        } else if let Some(resolved) = resolve_range(args, &entries) {
            resolved
        } else {
            // Treat as pattern search
            let pattern = args;
            entries
                .iter()
                .filter(|e| e.text.contains(pattern))
                .cloned()
                .collect::<Vec<_>>()
        };

        if selected.is_empty() {
            return Ok(magic::Output::Text("(no matching entries)\n".into()));
        }

        let start_idx = 0.max(entries.len() as isize - selected.len() as isize) as usize;
        let mut output = String::new();
        for (i, entry) in selected.iter().enumerate() {
            let num = start_idx + i + 1;
            // Truncate long lines for display
            let text = if entry.text.len() > 120 {
                format!("{}...", &entry.text[..117])
            } else {
                entry.text.clone()
            };
            output.push_str(&format!("{:>4}: [{}] {}\n", num, entry.mode, text));
        }
        Ok(magic::Output::Text(output))
    }
}

// ---------------------------------------------------------------------------
// %hist_n — Print history with line numbers
// ---------------------------------------------------------------------------

pub struct HistN;

impl magic::MagicHandler for HistN {
    fn name(&self) -> &'static str {
        "hist_n"
    }
    fn description(&self) -> &'static str {
        "Print history entries with absolute line numbers"
    }
    fn run(&self, line: &magic::MagicLine) -> Result<magic::Output, magic::MagicError> {
        let entries = get_history_snapshot();
        if entries.is_empty() {
            return Ok(magic::Output::Text("(history empty)\n".into()));
        }
        let args = line.args.trim();
        let selected: Vec<(&[Entry], usize)> = if args.is_empty() {
            let n = entries.len().min(20);
            let start = entries.len().saturating_sub(n);
            vec![(&entries[start..], start + 1)]
        } else if let Some(resolved) = resolve_range(args, &entries) {
            let start_idx = 0.max(entries.len() as isize - resolved.len() as isize) as usize;
            let num = start_idx + 1;
            // Need to map back; simpler: find the matching range
            vec![(&entries[start_idx..start_idx + resolved.len()], num)]
        } else {
            let pattern = args;
            let matched: Vec<usize> = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.text.contains(pattern))
                .map(|(i, _)| i)
                .collect();
            if matched.is_empty() {
                return Ok(magic::Output::Text("(no matching entries)\n".into()));
            }
            // Show matched entries with their absolute numbers
            let mut output = String::new();
            for &idx in &matched {
                let entry = &entries[idx];
                output.push_str(&format!(
                    "{:>4}: [{}] {}\n",
                    idx + 1,
                    entry.mode,
                    entry.text
                ));
            }
            return Ok(magic::Output::Text(output));
        };

        let mut output = String::new();
        for (i, entry) in selected[0].0.iter().enumerate() {
            let num = selected[0].1 + i;
            output.push_str(&format!("{:>4}: [{}] {}\n", num, entry.mode, entry.text));
        }
        Ok(magic::Output::Text(output))
    }
}

// ---------------------------------------------------------------------------
// %save — Save history to file
// ---------------------------------------------------------------------------

pub struct Save;

impl magic::MagicHandler for Save {
    fn name(&self) -> &'static str {
        "save"
    }
    fn description(&self) -> &'static str {
        "Save history to a file"
    }
    fn run(&self, line: &magic::MagicLine) -> Result<magic::Output, magic::MagicError> {
        let path = line.args.trim();
        if path.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %save <filepath>".into(),
            });
        }
        let filter = SnapshotFilter {
            mode_filter: None,
            pattern: None,
        };
        export_history(path, &filter)?;
        let entries = recent_entries(&filter, 0);
        Ok(magic::Output::Text(format!(
            "Saved {} history entries to {}\n",
            entries.len(),
            path
        )))
    }
}

// ---------------------------------------------------------------------------
// %rerun — Re-run a previous command by history number or pattern
// ---------------------------------------------------------------------------

pub struct Rerun;

impl magic::MagicHandler for Rerun {
    fn name(&self) -> &'static str {
        "rerun"
    }
    fn description(&self) -> &'static str {
        "Re-run a previous command by history number or pattern"
    }
    fn run(&self, line: &magic::MagicLine) -> Result<magic::Output, magic::MagicError> {
        let args = line.args.trim();
        let entries = get_history_snapshot();
        if entries.is_empty() {
            return Ok(magic::Output::Text("(history empty)\n".into()));
        }

        let matched: Vec<&Entry> = if args.is_empty() {
            // Default: re-run the most recent entry
            entries.last().into_iter().collect()
        } else if let Some(resolved) = resolve_range(args, &entries) {
            resolved.iter().map(|e| {
                entries.iter().find(|h| h.text == e.text).unwrap()
            }).collect()
        } else {
            // Pattern search: find most recent match
            entries.iter().rev().filter(|e| e.text.contains(args)).take(1).collect()
        };

        if matched.is_empty() {
            return Err(magic::MagicError {
                message: format!("No history entry matching '{args}'"),
            });
        }

        let entry = matched[0];
        let output = format!("Re-running: {}\n", entry.text);
        crate::r_runtime::eval_string_raw_global(&entry.text).map_err(|e| magic::MagicError {
            message: e.to_string(),
        })?;
        Ok(magic::Output::Text(output))
    }
}

// ---------------------------------------------------------------------------
// %recall — Recall a previous command for editing
// ---------------------------------------------------------------------------

pub struct Recall;

impl magic::MagicHandler for Recall {
    fn name(&self) -> &'static str {
        "recall"
    }
    fn description(&self) -> &'static str {
        "Recall a previous command by number or pattern for editing"
    }
    fn run(&self, line: &magic::MagicLine) -> Result<magic::Output, magic::MagicError> {
        let args = line.args.trim();
        let entries = get_history_snapshot();
        if entries.is_empty() {
            return Ok(magic::Output::Text("(history empty)\n".into()));
        }

        let matched: Vec<&Entry> = if args.is_empty() {
            entries.last().into_iter().collect()
        } else if let Some(resolved) = resolve_range(args, &entries) {
            resolved.iter().map(|e| {
                entries.iter().find(|h| h.text == e.text).unwrap()
            }).collect()
        } else {
            entries.iter().rev().filter(|e| e.text.contains(args)).take(1).collect()
        };

        if matched.is_empty() {
            return Err(magic::MagicError {
                message: format!("No history entry matching '{args}'"),
            });
        }

        Ok(magic::Output::Text(format!(
            "Recalled: {}\n",
            matched[0].text
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str) -> Entry {
        Entry {
            mode: "r".into(),
            text: text.into(),
        }
    }

    fn entries(texts: &[&str]) -> Vec<Entry> {
        texts.iter().map(|t| entry(t)).collect()
    }

    #[test]
    fn resolve_range_empty_returns_all() {
        let e = entries(&["a", "b", "c"]);
        let result = resolve_range("", &e).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn resolve_range_dollar_absolute() {
        let e = entries(&["x0", "x1", "x2", "x3", "x4"]);
        let result = resolve_range("$3", &e).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "x2");
    }

    #[test]
    fn resolve_range_dollar_out_of_range() {
        let e = entries(&["a", "b"]);
        assert!(resolve_range("$5", &e).is_none());
    }

    #[test]
    fn resolve_range_neg_last_n() {
        let e = entries(&["a", "b", "c", "d", "e"]);
        let result = resolve_range("-2", &e).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "d");
        assert_eq!(result[1].text, "e");
    }

    #[test]
    fn resolve_range_neg_more_than_available() {
        let e = entries(&["a", "b"]);
        assert!(resolve_range("-5", &e).is_none());
    }

    #[test]
    fn resolve_range_dash_range() {
        let e = entries(&["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9"]);
        let result = resolve_range("1-3", &e).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].text, "x7");
        assert_eq!(result[1].text, "x8");
        assert_eq!(result[2].text, "x9");
    }

    #[test]
    fn resolve_range_colon_range() {
        let e = entries(&["x0", "x1", "x2", "x3", "x4"]);
        let result = resolve_range("1:2", &e).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "x3");
        assert_eq!(result[1].text, "x4");
    }

    #[test]
    fn resolve_range_single_number() {
        let e = entries(&["a", "b", "c", "d"]);
        let result = resolve_range("1", &e).unwrap();
        assert_eq!(result[0].text, "d");
    }

    #[test]
    fn resolve_range_invalid_formats() {
        let e = entries(&["a"]);
        assert!(resolve_range("garbage", &e).is_none());
        assert!(resolve_range("$0", &e).is_none());
        assert!(resolve_range("-0", &e).is_none());
        assert!(resolve_range("0", &e).is_none());
        assert!(resolve_range("0-2", &e).is_none());
        assert!(resolve_range("2-1", &e).is_none());
    }
}
