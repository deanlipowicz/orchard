//! Frequency-tracked completion scoring.
//!
//! Records how often each completion is accepted and boosts frequent
//! completions in future rankings. Persisted to JSON between sessions.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Frequency bonus weight applied per previous acceptance.
const FREQUENCY_BONUS: f64 = 50.0;

/// Maximum boost from frequency alone (prevents old history from dominating).
const MAX_FREQUENCY_SCORE: f64 = 500.0;

/// A completion-frequency tracker that persists to disk.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct FrequencyData {
    /// Map from completion text → count.
    pub counts: HashMap<String, usize>,
}

impl FrequencyData {
    fn data_path() -> PathBuf {
        let base = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("orchard");
        // Ensure directory exists
        let _ = fs::create_dir_all(&base);
        base.join("completion_freq.json")
    }

    /// Load frequency data from disk, or return empty defaults.
    pub fn load() -> Self {
        let path = Self::data_path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Save frequency data to disk.
    pub fn save(&self) {
        let path = Self::data_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(&path, json);
        }
    }

    /// Record that a completion was accepted.
    pub fn record(&mut self, name: &str) {
        *self.counts.entry(name.to_string()).or_insert(0) += 1;
    }

    /// Get the frequency boost for a completion name.
    ///
    /// Returns a score in [0.0, MAX_FREQUENCY_SCORE] that should be added
    /// to the fuzzy-match score.
    pub fn boost(&self, name: &str) -> f64 {
        self.counts
            .get(name)
            .map(|&c| (c as f64 * FREQUENCY_BONUS).min(MAX_FREQUENCY_SCORE))
            .unwrap_or(0.0)
    }
}

/// Global singleton for frequency data.
fn global_frequency() -> &'static Mutex<FrequencyData> {
    static FREQ: OnceLock<Mutex<FrequencyData>> = OnceLock::new();
    FREQ.get_or_init(|| Mutex::new(FrequencyData::load()))
}

/// Record a completion acceptance and persist.
pub fn record_completion(name: &str) {
    let mut freq = global_frequency().lock().unwrap();
    freq.record(name);
    freq.save();
}

/// Get the frequency boost for a completion name.
pub fn frequency_boost(name: &str) -> f64 {
    let freq = global_frequency().lock().unwrap();
    freq.boost(name)
}
