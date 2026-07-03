//! Did-you-mean spellcheck suggestions via Levenshtein distance.

use super::Completion;
use crate::r_runtime;
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

/// Compute Levenshtein distance between two strings (case-insensitive).
pub fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.to_lowercase().chars().collect();
    let b: Vec<char> = b.to_lowercase().chars().collect();
    let a_len = a.len();
    let b_len = b.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev: Vec<usize> = (0..=b_len).collect();
    let mut curr = vec![0; b_len + 1];

    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] =
                std::cmp::min(std::cmp::min(curr[j] + 1, prev[j + 1] + 1), prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b_len]
}

const SPELLCHECK_CACHE_TTL: Duration = Duration::from_secs(60);

struct SpellcheckEntry {
    names: Vec<String>,
    fetched_at: Instant,
}

fn spellcheck_cache() -> &'static Mutex<HashMap<String, SpellcheckEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<String, SpellcheckEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn r_function_names() -> Vec<String> {
    let cache_key = "r_function_names";

    {
        let cache = spellcheck_cache().lock().unwrap();
        if let Some(entry) = cache.get(cache_key)
            && entry.fetched_at.elapsed() < SPELLCHECK_CACHE_TTL
        {
            return entry.names.clone();
        }
    }

    let r_code = r#"
        local({
            pkgs <- c(".GlobalEnv", "package:base", "package:stats",
                      "package:graphics", "package:grDevices",
                      "package:utils", "package:datasets", "package:methods")
            all <- unique(unlist(lapply(pkgs, function(p) {
                tryCatch(ls(name = p, all.names = FALSE), error = function(e) character(0))
            })))
            paste(all, collapse = "\n")
        })
    "#;

    let result = r_runtime::with_suppressed_stderr(|| {
        r_runtime::eval_string_raw_global(r_code)
    })
    .unwrap_or_default();

    let names: Vec<String> = result
        .lines()
        .map(String::from)
        .filter(|s| !s.is_empty())
        .collect();

    let mut cache = spellcheck_cache().lock().unwrap();
    cache.insert(
        cache_key.to_string(),
        SpellcheckEntry {
            names: names.clone(),
            fetched_at: Instant::now(),
        },
    );

    names
}

/// Generate "did you mean?" spellcheck completions.
///
/// Only activates when `prefix` is at least 3 characters long. Returns the
/// top 3 closest matches by Levenshtein distance (within distance ≤ 3).
/// Results include a hint in the display text (e.g. "mean  (did you mean?)").
pub fn spellcheck_completions(prefix: &str) -> Vec<Completion> {
    if prefix.len() < 3 {
        return vec![];
    }

    let candidates = r_function_names();
    if candidates.is_empty() {
        return vec![];
    }

    let prefix_lower = prefix.to_lowercase();

    let mut scored: Vec<(usize, &str)> = candidates
        .iter()
        .map(|name| (levenshtein_distance(name, &prefix_lower), name.as_str()))
        .filter(|(dist, _)| *dist <= 3)
        .collect();

    scored.sort_by_key(|(dist, _)| *dist);

    scored
        .into_iter()
        .take(3)
        .map(|(_, name)| Completion {
            replacement: name.to_string(),
            display: format!("{}  (did you mean?)", name),
        })
        .collect()
}
