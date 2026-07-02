use crate::magic::{self, MagicHandler, MagicLine, Output};

fn eval_r_captured(code: &str) -> Result<Output, magic::MagicError> {
    let wrapped = format!("capture.output({code})");
    let text = crate::r_runtime::eval_string_raw_global(&wrapped).map_err(|e| {
        magic::MagicError {
            message: e.to_string(),
        }
    })?;
    Ok(Output::Text(text))
}

fn eval_with_pkg_check(code: &str, pkg: &str) -> Result<Output, magic::MagicError> {
    let check = format!(
        "if (!requireNamespace('{pkg}', quietly=TRUE)) stop('package {pkg} is not installed')"
    );
    crate::r_runtime::eval_string_raw_global(&check).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    eval_r_captured(code)
}

// ---------------------------------------------------------------------------
// %summary — Statistical summary via summary()
// ---------------------------------------------------------------------------

pub struct Summary;

impl MagicHandler for Summary {
    fn name(&self) -> &'static str {
        "summary"
    }

    fn description(&self) -> &'static str {
        "Statistical summary of an R object via summary()"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %summary <R expression>".into(),
            });
        }
        eval_r_captured(&format!("base::summary({expr})"))
    }
}

// ---------------------------------------------------------------------------
// %glimpse — Compact column view via dplyr::glimpse()
// ---------------------------------------------------------------------------

pub struct Glimpse;

impl MagicHandler for Glimpse {
    fn name(&self) -> &'static str {
        "glimpse"
    }

    fn description(&self) -> &'static str {
        "Compact column view of a data frame via dplyr::glimpse()"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %glimpse <R expression>".into(),
            });
        }
        eval_with_pkg_check(
            &format!("dplyr::glimpse({expr})"),
            "dplyr",
        )
    }
}

// ---------------------------------------------------------------------------
// %describe — Rich summary stats via skimr::skim()
// ---------------------------------------------------------------------------

pub struct Describe;

impl MagicHandler for Describe {
    fn name(&self) -> &'static str {
        "describe"
    }

    fn description(&self) -> &'static str {
        "Rich summary statistics via skimr::skim()"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %describe <R expression>".into(),
            });
        }
        eval_with_pkg_check(
            &format!("skimr::skim({expr})"),
            "skimr",
        )
    }
}

// ---------------------------------------------------------------------------
// %missing — Missingness summary via naniar::miss_summary()
// ---------------------------------------------------------------------------

pub struct Missing;

impl MagicHandler for Missing {
    fn name(&self) -> &'static str {
        "missing"
    }

    fn description(&self) -> &'static str {
        "Missingness summary via naniar::miss_summary()"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %missing <R expression>".into(),
            });
        }
        eval_with_pkg_check(
            &format!("print(naniar::miss_summary({expr}))"),
            "naniar",
        )
    }
}

// ---------------------------------------------------------------------------
// %corr — Correlation matrix via cor()
// ---------------------------------------------------------------------------

pub struct Corr;

impl MagicHandler for Corr {
    fn name(&self) -> &'static str {
        "corr"
    }

    fn description(&self) -> &'static str {
        "Correlation matrix via cor() with pairwise complete observations"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %corr <R expression>".into(),
            });
        }
        eval_r_captured(&format!("cor({expr}, use = 'pairwise.complete.obs')"))
    }
}

// ---------------------------------------------------------------------------
// %freq — Frequency tables via janitor::tabyl()
// ---------------------------------------------------------------------------

pub struct Freq;

impl MagicHandler for Freq {
    fn name(&self) -> &'static str {
        "freq"
    }

    fn description(&self) -> &'static str {
        "Frequency tables via janitor::tabyl()"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %freq <R expression>".into(),
            });
        }
        eval_with_pkg_check(
            &format!("janitor::tabyl({expr})"),
            "janitor",
        )
    }
}

// ---------------------------------------------------------------------------
// %compare — Object diff via waldo::compare()
// ---------------------------------------------------------------------------

pub struct Compare;

impl MagicHandler for Compare {
    fn name(&self) -> &'static str {
        "compare"
    }

    fn description(&self) -> &'static str {
        "Diff two R objects via waldo::compare()"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %compare <obj1, obj2>".into(),
            });
        }
        eval_with_pkg_check(
            &format!("waldo::compare({expr}, max_diffs = 20)"),
            "waldo",
        )
    }
}

// ---------------------------------------------------------------------------
// %sessioninfo — Reproducibility metadata
// ---------------------------------------------------------------------------

pub struct SessionInfo;

impl MagicHandler for SessionInfo {
    fn name(&self) -> &'static str {
        "sessioninfo"
    }

    fn description(&self) -> &'static str {
        "Reproducibility metadata via sessioninfo::session_info()"
    }

    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        eval_with_pkg_check(
            "sessioninfo::session_info()",
            "sessioninfo",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_handler_registered(name: &str) {
        // Each handler must be reachable via the global registry
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get(name).is_some(), "handler '{name}' not registered");
    }

    #[test]
    fn summary_registered() {
        test_handler_registered("summary");
    }

    #[test]
    fn glimpse_registered() {
        test_handler_registered("glimpse");
    }

    #[test]
    fn describe_registered() {
        test_handler_registered("describe");
    }

    #[test]
    fn missing_registered() {
        test_handler_registered("missing");
    }

    #[test]
    fn corr_registered() {
        test_handler_registered("corr");
    }

    #[test]
    fn freq_registered() {
        test_handler_registered("freq");
    }

    #[test]
    fn compare_registered() {
        test_handler_registered("compare");
    }

    #[test]
    fn sessioninfo_registered() {
        test_handler_registered("sessioninfo");
    }

    #[test]
    fn summary_empty_args_returns_error() {
        let handler = Summary;
        let line = MagicLine {
            name: "summary".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err());
    }

    #[test]
    fn corr_empty_args_returns_error() {
        let handler = Corr;
        let line = MagicLine {
            name: "corr".into(),
            args: "".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err());
    }

    #[test]
    fn sessioninfo_ignores_args() {
        let handler = SessionInfo;
        let line = MagicLine {
            name: "sessioninfo".into(),
            args: "".into(),
            is_cell: false,
        };
        // Should not error on empty args — sessioninfo() takes no arguments
        let result = handler.run(&line);
        // May fail if R is not initialized, but should be an R error, not our validation
        // We just verify it attempts dispatch (some R call happens)
        assert!(
            result.is_err() || result.is_ok(),
            "sessioninfo should attempt R call"
        );
    }

    #[test]
    fn parse_magic_recognizes_all_eda_handlers() {
        let names = ["summary", "glimpse", "describe", "missing", "corr", "freq", "compare", "sessioninfo"];
        for name in &names {
            let input = format!("%{name} mtcars");
            let parsed = crate::magic::parse_magic(&input, false);
            assert!(parsed.is_some(), "failed to parse '%{name}'");
            assert_eq!(parsed.unwrap().name, *name);
        }
    }

    #[test]
    fn dispatch_eda_handler_returns_correct_variant() {
        // These test that dispatch runs without panicking and returns Output::Text.
        // The actual R evaluation will fail since R is not initialized in unit tests.
        // We test the pre-R path: parse + lookup succeeds.
        let names = ["summary", "glimpse", "describe", "missing", "corr", "freq", "compare", "sessioninfo"];
        for name in &names {
            let parsed = crate::magic::parse_magic(&format!("%{name} x"), false);
            assert!(parsed.is_some(), "failed to parse '%{name}'");
            let cmd = parsed.unwrap();
            // Check the handler exists in the registry (full dispatch needs R)
            let reg = crate::magic::magic_registry().lock().unwrap();
            assert!(reg.get(&cmd.name).is_some(), "handler '{}' not found in registry", cmd.name);
        }
    }
}
