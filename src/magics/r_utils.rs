//! Shared R evaluation utilities for magic handlers.
//!
//! All magic modules should use these functions instead of defining private
//! copies. `eval_r_captured` wraps code in `capture.output()` to route R
//! output through the `Output::Text` dispatch path consistently.

use crate::magic::{self, Output};

/// Evaluate R code and capture its output via `capture.output()`.
///
/// The code is wrapped in `capture.output({code})` so that R's printed
/// output is captured as a string and returned as `Output::Text`.
/// This ensures output always flows through the magic dispatch path
/// rather than directly through R's stdout callback.
pub fn eval_r_captured(code: &str) -> Result<Output, magic::MagicError> {
    let wrapped = format!("capture.output({code})");
    let text =
        crate::r_runtime::eval_string_raw_global(&wrapped).map_err(|e| magic::MagicError {
            message: e.to_string(),
        })?;
    Ok(Output::Text(text))
}

/// Evaluate R code silently, discarding any output.
///
/// The code is passed directly to R; any printed output goes to R's
/// stdout callback and is discarded. Returns `Ok(())` on success.
pub fn eval_r_silent(code: &str) -> Result<(), magic::MagicError> {
    crate::r_runtime::eval_string_raw_global(code).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    Ok(())
}

/// Check that a package is available, then evaluate R code via `eval_r_captured`.
///
/// Uses `requireNamespace(pkg, quietly=TRUE)` to verify the package is
/// installed before running the code. Returns the captured output on success,
/// or an error if the package is missing.
pub fn eval_with_pkg_check(code: &str, pkg: &str) -> Result<Output, magic::MagicError> {
    let check = format!(
        "if (!requireNamespace('{pkg}', quietly=TRUE)) stop('package {pkg} is not installed')"
    );
    crate::r_runtime::eval_string_raw_global(&check).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    eval_r_captured(code)
}
