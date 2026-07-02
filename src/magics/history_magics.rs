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
