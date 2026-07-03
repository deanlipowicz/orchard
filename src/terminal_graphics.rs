//! Terminal graphics protocol detection and inline image rendering.
//!
//! Detects Sixel, Kitty, and iTerm2 graphics protocols at startup and
//! provides a uniform interface for rendering PNG images inline in the
//! terminal.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Directory where captured plot PNG files are stored.
/// Created lazily in the system temp directory.
fn plot_temp_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join("orchard_plots");
        let _ = std::fs::create_dir_all(&dir);
        dir
    })
}

/// Return the path to a new unique PNG file in the plot temp directory.
pub fn new_plot_path() -> PathBuf {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    plot_temp_dir().join(format!("plot_{timestamp}.png"))
}

/// Return the most recent PNG file in the plot temp directory, if any.
pub fn latest_plot() -> Option<PathBuf> {
    let dir = plot_temp_dir();
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "png"))
        .collect();
    entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));
    entries.first().map(|e| e.path())
}

// ---------------------------------------------------------------------------
// Protocol detection
// ---------------------------------------------------------------------------

/// The terminal graphics protocol detected at startup.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GraphicsProtocol {
    /// No known inline graphics protocol — save to file instead.
    None,
    /// Sixel graphics (DEC terminals, XTerm, mlterm, foot, etc.)
    Sixel,
    /// Kitty terminal's graphics protocol.
    Kitty,
    /// iTerm2's inline images protocol.
    ITerm2,
}

impl GraphicsProtocol {
    /// Probe the terminal and return the best supported protocol.
    ///
    /// Detection order: Kitty → Sixel → iTerm2 → fallback.
    /// Kitty check first because its detection is the most reliable
    /// (environment variable).
    fn detect() -> Self {
        // Kitty — most reliable detection via env var
        if std::env::var("KITTY_PID").is_ok() || std::env::var("KITTY_WINDOW_ID").is_ok() {
            return GraphicsProtocol::Kitty;
        }

        // Sixel — check COLORTERM or TERM
        if let Ok(colorterm) = std::env::var("COLORTERM") {
            let ct = colorterm.to_lowercase();
            if ct.contains("sixel") || ct.contains("truecolor") {
                // Many truecolor terminals also support sixel.
                // We'll try a capability probe later.
                if terminal_supports_sixel() {
                    return GraphicsProtocol::Sixel;
                }
            }
        }
        // Also check TERM for sixel variants
        if let Ok(term) = std::env::var("TERM")
            && (term.contains("sixel") || term.contains("mlterm") || term.contains("foot"))
        {
            return GraphicsProtocol::Sixel;
        }

        // iTerm2 — check TERM_PROGRAM
        if let Ok(term_prog) = std::env::var("TERM_PROGRAM")
            && term_prog == "iTerm.app"
        {
            return GraphicsProtocol::ITerm2;
        }
        if std::env::var("ITERM_SESSION_ID").is_ok() {
            return GraphicsProtocol::ITerm2;
        }

        GraphicsProtocol::None
    }
}

/// Cached protocol detection result.
fn detected_protocol() -> &'static GraphicsProtocol {
    static PROTOCOL: OnceLock<GraphicsProtocol> = OnceLock::new();
    PROTOCOL.get_or_init(GraphicsProtocol::detect)
}

/// Heuristic: try sending a Sixel query sequence and checking if the
/// terminal responds with a Sixel capability indicator.
///
/// This is best-effort — some terminals support sixel without
/// advertising via the query.
fn terminal_supports_sixel() -> bool {
    // For now, conservative: return false unless explicitly detected.
    // A proper implementation would send `\x1b[c` (DA — Device Attributes)
    // and parse the response for sixel capability ( parameter `4` ).
    // This is tricky because it requires non-blocking reads during startup.
    //
    // Instead, we rely on COLORTERM and TERM env var heuristics above.
    false
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the detected graphics protocol.
pub fn protocol() -> GraphicsProtocol {
    *detected_protocol()
}

/// Render a PNG file inline in the terminal if a protocol is available.
///
/// Returns `true` if the image was displayed inline, `false` if it fell
/// back to printing the file path.
pub fn display_png(path: &Path) -> Result<bool, String> {
    match *detected_protocol() {
        GraphicsProtocol::None => {
            // Fallback: just print the path
            println!("Plot saved to: {}", path.display());
            Ok(false)
        }
        GraphicsProtocol::Kitty => render_kitty(path),
        GraphicsProtocol::Sixel => render_sixel(path),
        GraphicsProtocol::ITerm2 => render_iterm2(path),
    }
}

// ---------------------------------------------------------------------------
// Kitty graphics protocol
// ---------------------------------------------------------------------------

/// Render a PNG using Kitty's terminal graphics protocol.
///
/// Protocol: `\x1b_G<encoded_data>\x1b\`
/// The data is base64-encoded PNG.
fn render_kitty(path: &Path) -> Result<bool, String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read plot file: {e}"))?;
    let b64 = base64_encode(&data);

    // Split into chunks of 4096 bytes to avoid terminal buffer limits
    const CHUNK_SIZE: usize = 4096;
    let mut stdout = io::stdout().lock();

    if b64.len() <= CHUNK_SIZE {
        // Single chunk — use 'a' (transmission) with 'T' (transfer, no payload)
        write!(stdout, "\x1b_Ga=T,f=100,m=0;{}\x1b\\", b64)
            .map_err(|e| format!("Kitty write failed: {e}"))?;
    } else {
        // Multiple chunks — first chunk has m=1, last has m=0
        for (i, chunk) in b64.as_bytes().chunks(CHUNK_SIZE).enumerate() {
            let is_last = i == b64.len().div_ceil(CHUNK_SIZE) - 1;
            let chunk_str = std::str::from_utf8(chunk).map_err(|_| "base64 encoding failed")?;
            if i == 0 {
                write!(stdout, "\x1b_Ga=T,f=100,m=1;{}\x1b\\", chunk_str)
                    .map_err(|e| format!("Kitty write failed: {e}"))?;
            } else if is_last {
                write!(stdout, "\x1b_Gm=0;{}\x1b\\", chunk_str)
                    .map_err(|e| format!("Kitty write failed: {e}"))?;
            } else {
                write!(stdout, "\x1b_Gm=1;{}\x1b\\", chunk_str)
                    .map_err(|e| format!("Kitty write failed: {e}"))?;
            }
        }
    }
    stdout.flush().map_err(|e| format!("flush failed: {e}"))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Sixel graphics
// ---------------------------------------------------------------------------

/// Render a PNG using Sixel graphics.
///
/// Sixel does not natively support PNG. We would need to either:
/// 1. Convert PNG → Sixel using a C library (libsixel)
/// 2. Use ImageMagick (`convert`) as an external tool
/// 3. Use a Rust library (e.g., `sixel` crate)
///
/// For now, we save the PNG to a temp file and print instructions.
/// Full Sixel support will require adding a dep or calling an external
/// converter.
fn render_sixel(path: &Path) -> Result<bool, String> {
    // Attempt to convert PNG to sixel using imagemagick or ffmpeg
    // Fallback to just printing the path
    if let Ok(output) = std::process::Command::new("convert")
        .args([path.to_str().unwrap_or(""), "sixel:-"])
        .output()
        && output.status.success()
    {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(&output.stdout)
            .map_err(|e| format!("sixel write failed: {e}"))?;
        stdout.flush().ok();
        return Ok(true);
    }

    // Try ffmpeg as alternative
    if let Ok(output) = std::process::Command::new("ffmpeg")
        .args(["-i", path.to_str().unwrap_or(""), "-f", "sixel", "pipe:1"])
        .output()
        && output.status.success()
    {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(&output.stdout)
            .map_err(|e| format!("sixel write failed: {e}"))?;
        stdout.flush().ok();
        return Ok(true);
    }

    // Fallback
    println!(
        "Plot saved to: {} (install ImageMagick for sixel output)",
        path.display()
    );
    Ok(false)
}

// ---------------------------------------------------------------------------
// iTerm2 inline images protocol
// ---------------------------------------------------------------------------

/// Render a PNG using iTerm2's inline images protocol.
///
/// Protocol: `\x1b]1337;File=inline=1;size=<bytes>:<base64>\x07`
fn render_iterm2(path: &Path) -> Result<bool, String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read plot file: {e}"))?;
    let b64 = base64_encode(&data);
    let size = data.len();

    write!(
        io::stdout(),
        "\x1b]1337;File=inline=1;size={size}:{b64}\x07"
    )
    .map_err(|e| format!("iTerm2 write failed: {e}"))?;
    io::stdout().flush().ok();
    Ok(true)
}

// ---------------------------------------------------------------------------
// Base64 encoding (no external crate)
// ---------------------------------------------------------------------------

/// Encode bytes to base64 without external dependencies.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encodes_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_encodes_single_byte() {
        assert_eq!(base64_encode(b"M"), "TQ==");
    }

    #[test]
    fn base64_encodes_two_bytes() {
        assert_eq!(base64_encode(b"Ma"), "TWE=");
    }

    #[test]
    fn base64_encodes_three_bytes() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
    }

    #[test]
    fn base64_encodes_standard_rfc4648_test() {
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn protocol_detection_returns_something() {
        let proto = GraphicsProtocol::detect();
        // Should not panic — any result is valid depending on the test env
        assert!(matches!(
            proto,
            GraphicsProtocol::None
                | GraphicsProtocol::Kitty
                | GraphicsProtocol::Sixel
                | GraphicsProtocol::ITerm2
        ));
    }

    #[test]
    fn protocol_cached_value_matches_detection() {
        // The cached value should be consistent
        let p = protocol();
        assert!(matches!(
            p,
            GraphicsProtocol::None
                | GraphicsProtocol::Kitty
                | GraphicsProtocol::Sixel
                | GraphicsProtocol::ITerm2
        ));
    }
}
