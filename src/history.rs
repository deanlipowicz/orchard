use crate::{cli::Cli, r_runtime::PromptMode, settings::Settings};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use reedline::{
    CommandLineSearch, HistoryItem, HistoryItemId, HistorySessionId, Result, SearchQuery,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub mode: String,
    pub text: String,
}

#[derive(Debug)]
pub struct History {
    entries: Vec<Entry>,
    path: Option<PathBuf>,
    max_size: usize,
    ignore_browser: bool,
}

impl History {
    pub fn new(cli: &Cli, settings: &Settings) -> anyhow::Result<Self> {
        if cli.no_history {
            return Ok(Self::memory(settings));
        }

        let local = PathBuf::from(&settings.local_history_file);
        let path = if cli.local_history || (!cli.global_history && local.exists()) {
            local
        } else {
            PathBuf::from(crate::util::expand_tilde(&settings.global_history_file))
        };

        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).ok();
            }
        }

        let entries = load_file(&path)?;
        Ok(Self {
            entries,
            path: Some(path),
            max_size: settings.history_size.max(10) as usize,
            ignore_browser: settings.history_ignore_browser_commands,
        })
    }

    pub fn memory(settings: &Settings) -> Self {
        Self {
            entries: Vec::new(),
            path: None,
            max_size: settings.history_size.max(10) as usize,
            ignore_browser: settings.history_ignore_browser_commands,
        }
    }

    pub fn append(&mut self, mode: &str, text: &str) -> anyhow::Result<()> {
        let text = text.trim_end_matches('\n');
        if text.is_empty()
            || self
                .entries
                .last()
                .is_some_and(|e| e.mode == mode && e.text == text)
            || (self.ignore_browser && mode == "browse" && is_browser_command(text))
        {
            return Ok(());
        }

        let entry = Entry {
            mode: mode.to_string(),
            text: text.to_string(),
        };
        if let Some(path) = &self.path {
            append_file(path, &entry)?;
        }
        self.entries.push(entry);
        self.trim()?;
        Ok(())
    }

    pub fn search(
        &self,
        mode: &str,
        query: &str,
        ignore_case: bool,
        no_duplicates: bool,
    ) -> Vec<&Entry> {
        let needle = if ignore_case {
            query.to_lowercase()
        } else {
            query.to_string()
        };
        let mut seen = Vec::<String>::new();
        self.entries
            .iter()
            .rev()
            .filter(|e| compatible(mode, &e.mode))
            .filter(|e| {
                let hay = if ignore_case {
                    e.text.to_lowercase()
                } else {
                    e.text.clone()
                };
                hay.contains(&needle)
            })
            .filter(|e| {
                if !no_duplicates {
                    return true;
                }
                if seen.contains(&e.text) {
                    false
                } else {
                    seen.push(e.text.clone());
                    true
                }
            })
            .collect()
    }

    fn trim(&mut self) -> anyhow::Result<()> {
        if self.entries.len() <= self.max_size {
            return Ok(());
        }
        let keep = ((self.max_size as f64) * 0.9).round() as usize;
        self.entries = self
            .entries
            .split_off(self.entries.len().saturating_sub(keep));
        if let Some(path) = &self.path {
            rewrite_file(path, &self.entries)?;
        }
        Ok(())
    }

    /// Returns all loaded entries for seeding the reedline history backend.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
}

// ---------------------------------------------------------------------------
// OrchardHistoryBackend — reedline History trait wrapper for loaded history
// ---------------------------------------------------------------------------

/// Implements reedline's `History` trait backed by orchard's `History`.
/// Provides mode-aware search and delegates file writes.
pub struct OrchardHistoryBackend {
    /// Searchable in-memory entries.  Index in vec = HistoryItemId.
    items: Vec<HistoryItem>,
    /// Mode label for each entry, parallel to `items`.
    modes: Vec<String>,
    /// Current prompt mode, shared with PromptSession.
    mode: Arc<Mutex<PromptMode>>,
    /// Next ID to assign in save().
    next_id: usize,
}

impl OrchardHistoryBackend {
    /// Create a new backend seeded from orchard's loaded history entries.
    /// The entries are copied at construction time — new entries added via
    /// `save()` extend the in-memory index only; file persistence is
    /// handled separately by `append_history()`.
    pub fn new(entries: &[Entry], mode: Arc<Mutex<PromptMode>>) -> Self {
        let (items, modes): (Vec<_>, Vec<_>) = entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let mut item = HistoryItem::from_command_line(entry.text.clone());
                item.id = Some(HistoryItemId(i as i64));
                (item, entry.mode.clone())
            })
            .unzip();
        let next_id = items.len();
        Self {
            items,
            modes,
            mode,
            next_id,
        }
    }
}

impl reedline::History for OrchardHistoryBackend {
    fn save(&mut self, mut item: HistoryItem) -> Result<HistoryItem> {
        let mode_string = self.mode.lock().unwrap().mode_string().to_string();
        item.id = Some(HistoryItemId(self.next_id as i64));
        self.items.push(item.clone());
        self.modes.push(mode_string);
        self.next_id += 1;
        Ok(item)
    }

    fn load(&self, id: HistoryItemId) -> Result<HistoryItem> {
        let i = id.0 as usize;
        if i < self.items.len() {
            Ok(self.items[i].clone())
        } else {
            Ok(HistoryItem::from_command_line(""))
        }
    }

    fn search(&self, query: SearchQuery) -> Result<Vec<HistoryItem>> {
        let current_mode = self.mode.lock().unwrap().mode_string().to_string();

        // Mode filter: only entries compatible with current mode
        let mut results: Vec<&HistoryItem> = self
            .items
            .iter()
            .enumerate()
            .filter(|(i, _)| compatible(&current_mode, &self.modes[*i]))
            .map(|(_, item)| item)
            .collect();

        // Command-line filter
        if let Some(cl_search) = &query.filter.command_line {
            let (search_str, search_type) = match cl_search {
                CommandLineSearch::Prefix(s) => (s.as_str(), 0),
                CommandLineSearch::Substring(s) => (s.as_str(), 1),
                CommandLineSearch::Exact(s) => (s.as_str(), 2),
            };
            results.retain(|item| match search_type {
                0 => item.command_line.starts_with(search_str),
                1 => item.command_line.contains(search_str),
                _ => item.command_line == search_str,
            });
        }

        // Sort by ID descending (most recent first)
        results.sort_by_key(|b| std::cmp::Reverse(b.id));

        // Apply limit
        let items: Vec<HistoryItem> = if let Some(limit) = query.limit {
            results.into_iter().take(limit as usize).cloned().collect()
        } else {
            results.into_iter().cloned().collect()
        };

        Ok(items)
    }

    fn count(&self, query: SearchQuery) -> Result<i64> {
        self.search(query).map(|v| v.len() as i64)
    }

    fn update(
        &mut self,
        id: HistoryItemId,
        updater: &dyn Fn(HistoryItem) -> HistoryItem,
    ) -> Result<()> {
        let i = id.0 as usize;
        if i < self.items.len() {
            self.items[i] = updater(self.items[i].clone());
        }
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        self.items.clear();
        self.modes.clear();
        self.next_id = 0;
        Ok(())
    }

    fn delete(&mut self, id: HistoryItemId) -> Result<()> {
        let i = id.0 as usize;
        if i < self.items.len() {
            self.items[i].command_line = String::new();
            self.modes[i] = String::new();
        }
        Ok(())
    }

    fn sync(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn session(&self) -> Option<HistorySessionId> {
        None
    }
}

pub fn compatible(current: &str, candidate: &str) -> bool {
    current == candidate || history_book(current) == history_book(candidate)
}

fn history_book(mode: &str) -> &str {
    match mode {
        "r" | "browse" => "r",
        other => other,
    }
}

fn is_browser_command(text: &str) -> bool {
    matches!(
        text.trim(),
        "n" | "s" | "f" | "c" | "cont" | "Q" | "where" | "help"
    )
}

fn load_file(path: &Path) -> anyhow::Result<Vec<Entry>> {
    let mut bytes = Vec::new();
    match fs::File::open(path) {
        Ok(mut f) => {
            f.read_to_end(&mut bytes)?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    }
    Ok(parse(&String::from_utf8_lossy(&bytes)))
}

fn parse(input: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut mode = String::new();
    let mut lines = Vec::new();

    for line in input.lines() {
        if let Some(rest) = line.strip_prefix("# mode: ") {
            mode = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix('+') {
            lines.push(rest.to_string());
        } else if !lines.is_empty() {
            out.push(Entry {
                mode: mode.clone(),
                text: lines.join("\n"),
            });
            lines.clear();
        }
    }
    if !lines.is_empty() {
        out.push(Entry {
            mode,
            text: lines.join("\n"),
        });
    }
    out
}

fn append_file(path: &Path, entry: &Entry) -> anyhow::Result<()> {
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    write_entry(&mut f, entry)
}

fn rewrite_file(path: &Path, entries: &[Entry]) -> anyhow::Result<()> {
    let mut f = fs::File::create(path)?;
    for entry in entries {
        write_entry(&mut f, entry)?;
    }
    Ok(())
}

fn write_entry(mut out: impl Write, entry: &Entry) -> anyhow::Result<()> {
    writeln!(out)?;
    writeln!(out, "# time: {} UTC", utc_now())?;
    writeln!(out, "# mode: {}", entry.mode)?;
    for line in entry.text.split('\n') {
        writeln!(out, "+{line}")?;
    }
    Ok(())
}

fn utc_now() -> String {
    chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reedline::{CommandLineSearch, History as _, SearchQuery};

    #[test]
    fn loads_multiline_history() {
        let entries = parse("\n# time: x UTC\n# mode: r\n+x <- 1\n+x + 1\n");
        assert_eq!(
            entries,
            vec![Entry {
                mode: "r".into(),
                text: "x <- 1\nx + 1".into()
            }]
        );
    }

    #[test]
    fn filters_search_by_history_book() {
        let mut h = History::memory(&Settings::default());
        h.append("r", "alpha").unwrap();
        h.append("browse", "alpha browse").unwrap();
        h.append("shell", "alpha shell").unwrap();
        let found = h.search("browse", "alpha", false, false);
        assert_eq!(found.len(), 2);
        assert!(found.iter().all(|e| e.mode != "shell"));
    }

    #[test]
    fn skips_empty_duplicates_and_browser_commands() {
        let mut h = History::memory(&Settings::default());
        h.append("r", "").unwrap();
        h.append("r", "x").unwrap();
        h.append("r", "x").unwrap();
        h.append("browse", "n").unwrap();
        assert_eq!(
            h.entries,
            vec![Entry {
                mode: "r".into(),
                text: "x".into()
            }]
        );
    }

    #[test]
    fn appends_stores_correct_mode_string() {
        let mut h = History::memory(&Settings::default());
        h.append("r", "x <- 1").unwrap();
        h.append("browse", "ls()").unwrap();
        h.append("shell", "echo hello").unwrap();
        assert_eq!(h.entries[0].mode, "r");
        assert_eq!(h.entries[1].mode, "browse");
        assert_eq!(h.entries[2].mode, "shell");
    }

    #[test]
    fn filters_browse_commands_when_ignore_browser_enabled() {
        let mut settings = Settings::default();
        settings.history_ignore_browser_commands = true;
        let mut h = History::memory(&settings);
        h.append("browse", "n").unwrap(); // browser command → skip
        h.append("browse", "x").unwrap(); // not a browser command → keep
        h.append("browse", "s").unwrap(); // browser command → skip
        h.append("browse", "cont").unwrap(); // browser command → skip
        assert_eq!(h.entries.len(), 1);
        assert_eq!(h.entries[0].text, "x");
    }

    #[test]
    fn case_insensitive_search_matches_different_case() {
        let mut h = History::memory(&Settings::default());
        h.append("r", "Mean(x)").unwrap();
        h.append("r", "median(y)").unwrap();
        let found = h.search("r", "mean", true, false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].text, "Mean(x)");
    }

    #[test]
    fn round_trip_write_then_parse() {
        let dir = std::env::temp_dir().join(format!(
            "orchard-test-history-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".orchard_history");

        let mut settings = Settings::default();
        settings.history_ignore_browser_commands = false;
        let mut h = History::memory(&settings);
        h.append("r", "x <- 1").unwrap();
        h.append("browse", "ls()").unwrap();
        h.append("shell", "pwd").unwrap();

        // Write entries to file
        for entry in &h.entries {
            let mut f = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap();
            write_entry(&mut f, entry).unwrap();
        }

        // Parse back
        let loaded = load_file(&path).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].mode, "r");
        assert_eq!(loaded[0].text, "x <- 1");
        assert_eq!(loaded[1].mode, "browse");
        assert_eq!(loaded[1].text, "ls()");
        assert_eq!(loaded[2].mode, "shell");
        assert_eq!(loaded[2].text, "pwd");

        // Clean up
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn trims_when_exceeding_max_size() {
        let mut settings = Settings::default();
        settings.history_size = 10;
        let mut h = History::memory(&settings);
        for i in 0..20 {
            h.append("r", &format!("x{i}")).unwrap();
        }
        // Trim keeps 90% of max_size = 9 entries
        assert!(h.entries.len() <= 10);
        // Most recent entries should be kept
        assert_eq!(h.entries.last().unwrap().text, "x19");
    }

    // --- Malformed-input recovery tests for parse() ---

    #[test]
    fn parse_empty_input_returns_no_entries() {
        assert!(parse("").is_empty());
    }

    #[test]
    fn parse_only_whitespace_returns_no_entries() {
        assert!(parse("\n\n\n").is_empty());
    }

    #[test]
    fn parse_only_headers_no_content_returns_no_entries() {
        let input = "\n# time: 2024-01-01 00:00:00 UTC\n# mode: r\n";
        assert!(parse(input).is_empty());
    }

    #[test]
    fn parse_content_without_mode_header_gives_empty_mode() {
        let input = "+x <- 1\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mode, "");
        assert_eq!(entries[0].text, "x <- 1");
    }

    #[test]
    fn parse_truncated_mode_header_still_parses_content() {
        // "# mode:" without space after colon — strip_prefix("# mode: ") won't match
        let input = "# mode:r\n+x <- 1\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mode, "");
        assert_eq!(entries[0].text, "x <- 1");
    }

    #[test]
    fn parse_truncated_time_header_ignored() {
        // "# time:" without full prefix — line is not a content line, not a mode line,
        // and lines is empty, so it's silently dropped.
        let input = "# time: 2024-01-01\n# mode: r\n+x <- 1\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mode, "r");
        assert_eq!(entries[0].text, "x <- 1");
    }

    #[test]
    fn parse_garbage_line_between_entries_triggers_flush() {
        let input = "\n# mode: r\n+x <- 1\ngarbage line\n# mode: shell\n+ls\n";
        let entries = parse(input);
        // First entry: "x <- 1" flushed by the garbage line
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].mode, "r");
        assert_eq!(entries[0].text, "x <- 1");
        assert_eq!(entries[1].mode, "shell");
        assert_eq!(entries[1].text, "ls");
    }

    #[test]
    fn parse_garbage_before_any_content_is_dropped() {
        let input = "garbage\n# mode: r\n+x <- 1\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mode, "r");
        assert_eq!(entries[0].text, "x <- 1");
    }

    #[test]
    fn parse_truncated_entry_at_eof_is_emitted() {
        // No trailing flush line — the final `if !lines.is_empty()` block emits it.
        let input = "# mode: r\n+x <- 1\n+y <- 2";
        let entries = parse(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mode, "r");
        assert_eq!(entries[0].text, "x <- 1\ny <- 2");
    }

    #[test]
    fn parse_single_line_entry_at_eof() {
        let input = "# mode: shell\n+pwd";
        let entries = parse(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mode, "shell");
        assert_eq!(entries[0].text, "pwd");
    }

    #[test]
    fn parse_mode_persists_across_entries() {
        let input = "\n# mode: r\n+x <- 1\n\n+x <- 2\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].mode, "r");
        assert_eq!(entries[0].text, "x <- 1");
        assert_eq!(entries[1].mode, "r");
        assert_eq!(entries[1].text, "x <- 2");
    }

    #[test]
    fn parse_mode_change_between_entries() {
        let input = "\n# mode: r\n+x <- 1\n\n# mode: shell\n+ls\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].mode, "r");
        assert_eq!(entries[0].text, "x <- 1");
        assert_eq!(entries[1].mode, "shell");
        assert_eq!(entries[1].text, "ls");
    }

    #[test]
    fn parse_multiline_entry_joined_with_newline() {
        let input = "# mode: r\n+x <- 1\n+y <- 2\n+z <- 3\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "x <- 1\ny <- 2\nz <- 3");
    }

    #[test]
    fn parse_empty_content_line_preserved_in_entry() {
        // A "+" with nothing after it is an empty content line
        let input = "# mode: r\n+x <- 1\n+\n+y <- 2\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text, "x <- 1\n\ny <- 2");
    }

    #[test]
    fn parse_plus_prefix_stripped_from_content() {
        let input = "# mode: r\n+print(x)\n";
        let entries = parse(input);
        assert_eq!(entries[0].text, "print(x)");
    }

    #[test]
    fn parse_multiple_entries_with_blank_line_separators() {
        let input = "\n# mode: r\n+x\n\n# mode: shell\n+ls\n\n# mode: browse\n+n\n";
        let entries = parse(input);
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].mode, "r");
        assert_eq!(entries[0].text, "x");
        assert_eq!(entries[1].mode, "shell");
        assert_eq!(entries[1].text, "ls");
        assert_eq!(entries[2].mode, "browse");
        assert_eq!(entries[2].text, "n");
    }

    #[test]
    fn parse_mode_line_with_trailing_whitespace_trimmed() {
        let input = "# mode: r   \n+x <- 1\n";
        let entries = parse(input);
        assert_eq!(entries[0].mode, "r");
    }

    #[test]
    fn parse_completely_garbage_input_returns_no_entries() {
        let input = "this is not valid\nhistory format\nat all\n";
        // No "+" lines, no mode lines — nothing to emit
        assert!(parse(input).is_empty());
    }

    #[test]
    fn parse_blank_line_flushes_current_entry() {
        let input = "# mode: r\n+x <- 1\n\n+y <- 2\n";
        let entries = parse(input);
        // The blank line between the two "+" lines triggers a flush of "x <- 1",
        // then "y <- 2" is emitted at EOF.
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text, "x <- 1");
        assert_eq!(entries[1].text, "y <- 2");
    }

    #[test]
    fn parse_round_trip_multiple_entries() {
        let original = vec![
            Entry {
                mode: "r".into(),
                text: "x <- 1".into(),
            },
            Entry {
                mode: "shell".into(),
                text: "ls -la".into(),
            },
            Entry {
                mode: "browse".into(),
                text: "n".into(),
            },
        ];
        let mut buf = Vec::new();
        for entry in &original {
            write_entry(&mut buf, entry).unwrap();
        }
        let parsed = parse(&String::from_utf8_lossy(&buf));
        assert_eq!(parsed.len(), original.len());
        for (got, want) in parsed.iter().zip(original.iter()) {
            assert_eq!(got.mode, want.mode);
            assert_eq!(got.text, want.text);
        }
    }

    #[test]
    fn parse_round_trip_multiline_entry() {
        let original = vec![Entry {
            mode: "r".into(),
            text: "x <- 1\ny <- 2\nz <- 3".into(),
        }];
        let mut buf = Vec::new();
        for entry in &original {
            write_entry(&mut buf, entry).unwrap();
        }
        let parsed = parse(&String::from_utf8_lossy(&buf));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].mode, "r");
        assert_eq!(parsed[0].text, "x <- 1\ny <- 2\nz <- 3");
    }

    // --- parse property tests ---

    use proptest::prelude::*;

    fn arb_entry() -> impl Strategy<Value = Entry> {
        (
            prop::sample::select(vec!["r", "shell", "browse", ""]),
            "[a-zA-Z0-9 \n.<\\-+*/()]+",
        )
            .prop_map(|(mode, text)| Entry {
                mode: mode.into(),
                text,
            })
    }

    proptest! {
        #[test]
        fn prop_round_trip_single_entry(entry in arb_entry()) {
            let mut buf = Vec::new();
            write_entry(&mut buf, &entry).unwrap();
            let parsed = parse(&String::from_utf8_lossy(&buf));
            prop_assert_eq!(parsed.len(), 1);
            prop_assert_eq!(&parsed[0].mode, &entry.mode);
            prop_assert_eq!(&parsed[0].text, &entry.text);
        }

        #[test]
        fn prop_round_trip_multiple_entries(entries in prop::collection::vec(arb_entry(), 1..10)) {
            let mut buf = Vec::new();
            for entry in &entries {
                write_entry(&mut buf, entry).unwrap();
            }
            let parsed = parse(&String::from_utf8_lossy(&buf));
            prop_assert_eq!(parsed.len(), entries.len());
            for (got, want) in parsed.iter().zip(entries.iter()) {
                prop_assert_eq!(&got.mode, &want.mode);
                prop_assert_eq!(&got.text, &want.text);
            }
        }

        #[test]
        fn prop_parse_never_panics(input in ".*") {
            // parse should never panic on arbitrary input
            let _ = parse(&input);
        }
    }

    // --- OrchardHistoryBackend tests ---

    #[test]
    fn backend_seeded_from_entries() {
        let entries = [
            Entry {
                mode: "r".into(),
                text: "mean(x)".into(),
            },
            Entry {
                mode: "r".into(),
                text: "plot(y)".into(),
            },
            Entry {
                mode: "shell".into(),
                text: "ls -la".into(),
            },
        ];
        let mode = Arc::new(Mutex::new(PromptMode::R));
        let backend = OrchardHistoryBackend::new(&entries, mode);
        assert_eq!(backend.items.len(), 3);
        assert_eq!(backend.items[0].command_line, "mean(x)");
        assert_eq!(backend.items[1].command_line, "plot(y)");
        assert_eq!(backend.items[2].command_line, "ls -la");
    }

    #[test]
    fn save_appends_to_index() {
        let mode = Arc::new(Mutex::new(PromptMode::R));
        let mut backend = OrchardHistoryBackend::new(&[], mode);
        backend.save(HistoryItem::from_command_line("1 + 1")).ok();
        assert_eq!(backend.items.len(), 1);
        assert_eq!(backend.items[0].command_line, "1 + 1");
        assert_eq!(backend.modes[0], "r");
    }

    #[test]
    fn search_filters_by_current_mode() {
        let mode = Arc::new(Mutex::new(PromptMode::R));
        let entries = [
            Entry {
                mode: "r".into(),
                text: "lm(y ~ x)".into(),
            },
            Entry {
                mode: "shell".into(),
                text: "ls".into(),
            },
            Entry {
                mode: "browse".into(),
                text: "n".into(),
            },
        ];
        let backend = OrchardHistoryBackend::new(&entries, mode.clone());

        // In R mode, should find "r" and "browse" (same history book)
        let query = SearchQuery {
            direction: reedline::SearchDirection::Backward,
            start_time: None,
            end_time: None,
            start_id: None,
            end_id: None,
            limit: None,
            filter: reedline::SearchFilter::from_text_search(
                CommandLineSearch::Substring(String::new()),
                None,
            ),
        };
        let results = backend.search(query).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|i| i.command_line == "lm(y ~ x)"));
        assert!(results.iter().any(|i| i.command_line == "n"));

        // Switch to shell mode
        *mode.lock().unwrap() = PromptMode::Shell;
        let query = SearchQuery {
            direction: reedline::SearchDirection::Backward,
            start_time: None,
            end_time: None,
            start_id: None,
            end_id: None,
            limit: None,
            filter: reedline::SearchFilter::from_text_search(
                CommandLineSearch::Substring(String::new()),
                None,
            ),
        };
        let results = backend.search(query).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command_line, "ls");
    }

    #[test]
    fn search_filters_by_substring() {
        let mode = Arc::new(Mutex::new(PromptMode::R));
        let entries = [
            Entry {
                mode: "r".into(),
                text: "mean(x)".into(),
            },
            Entry {
                mode: "r".into(),
                text: "plot(mean)".into(),
            },
            Entry {
                mode: "r".into(),
                text: "lm(y)".into(),
            },
        ];
        let backend = OrchardHistoryBackend::new(&entries, mode);

        let query = SearchQuery {
            direction: reedline::SearchDirection::Backward,
            start_time: None,
            end_time: None,
            start_id: None,
            end_id: None,
            limit: None,
            filter: reedline::SearchFilter::from_text_search(
                CommandLineSearch::Substring("mean".into()),
                None,
            ),
        };
        let results = backend.search(query).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|i| i.command_line == "mean(x)"));
        assert!(results.iter().any(|i| i.command_line == "plot(mean)"));
    }
}

// Accessor for magics that need a snapshot of the current history
pub fn get_history_snapshot() -> Vec<String> {
    crate::r_runtime::history_text_snapshot()
}
