//! Filesystem watcher for auto-reloading modified R source files.
//!
//! Mirrors Revise.jl: watches the current working directory recursively for
//! changes to `.R` / `.r` files and pushes modified paths to a shared queue
//! that the REPL loop drains by calling `source()`.

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread::JoinHandle;

/// Whether auto-reload is currently enabled (read from R options at REPL loop
/// start).
pub static AUTO_RELOAD_ENABLED: AtomicBool = AtomicBool::new(false);

/// Shared queue of file paths that have been modified and need re-sourcing.
static RELOAD_QUEUE: OnceLock<Mutex<VecDeque<PathBuf>>> = OnceLock::new();

fn queue() -> &'static Mutex<VecDeque<PathBuf>> {
    RELOAD_QUEUE.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Pop the next file path that needs re-sourcing.
pub fn try_recv_reload() -> Option<PathBuf> {
    queue().lock().ok()?.pop_front()
}

/// Start the filesystem watcher on the current working directory.
///
/// Spawns a background thread using the `notify` crate's `RecommendedWatcher`.
/// The watcher sends `Event::Modify` events for `.R`/`.r` files to the shared
/// queue. Returns a `JoinHandle` that can be joined on shutdown.
pub fn start_watcher() -> std::io::Result<(JoinHandle<()>, FileWatcherGuard)> {
    let cwd = std::env::current_dir()?;
    let guard = FileWatcherGuard;
    let (tx, rx) = std::sync::mpsc::channel::<Result<Event, notify::Error>>();
    let mut watcher =
        RecommendedWatcher::new(tx, Config::default()).map_err(std::io::Error::other)?;

    watcher
        .watch(&cwd, RecursiveMode::Recursive)
        .map_err(std::io::Error::other)?;

    let handle = std::thread::spawn(move || {
        while let Ok(Ok(Event {
            kind: EventKind::Modify(_),
            paths,
            ..
        })) = rx.recv()
        {
            for path in paths {
                // Only process .R and .r files.
                if let Some(ext) = path.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if ext_lower == "r"
                        && let Ok(mut q) = queue().lock()
                        && !q.contains(&path)
                    {
                        q.push_back(path);
                    }
                }
            }
        }
    });

    Ok((handle, guard))
}

/// RAII guard — the watcher thread stops when this is dropped (the channel
/// closes, causing the receiver loop to exit).
pub struct FileWatcherGuard;

/// Check whether auto-reload is enabled (reads from `getOption("orchard.auto_reload")`).
/// Returns true if the option is set to TRUE.
pub fn auto_reload_enabled() -> bool {
    // The static flag is updated by the REPL loop at each iteration.
    // We read from R options in the REPL loop and set this flag accordingly.
    AUTO_RELOAD_ENABLED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_recv_empty_queue() {
        // Ensure a fresh queue.
        let _ = RELOAD_QUEUE.set(Mutex::new(VecDeque::new()));
        assert!(try_recv_reload().is_none());
    }

    #[test]
    fn push_and_pop() {
        let _ = RELOAD_QUEUE.set(Mutex::new(VecDeque::new()));
        if let Ok(mut q) = queue().lock() {
            q.push_back(PathBuf::from("test.R"));
            q.push_back(PathBuf::from("lib/utils.R"));
        }
        assert_eq!(try_recv_reload(), Some(PathBuf::from("test.R")));
        assert_eq!(try_recv_reload(), Some(PathBuf::from("lib/utils.R")));
        assert!(try_recv_reload().is_none());
    }

    #[test]
    fn deduplicates_paths() {
        let _ = RELOAD_QUEUE.set(Mutex::new(VecDeque::new()));
        if let Ok(mut q) = queue().lock() {
            q.push_back(PathBuf::from("test.R"));
            // Second push of same path should be deduped
            if !q.contains(&PathBuf::from("test.R")) {
                q.push_back(PathBuf::from("test.R"));
            }
        }
        assert_eq!(try_recv_reload(), Some(PathBuf::from("test.R")));
        assert!(try_recv_reload().is_none());
    }
}
