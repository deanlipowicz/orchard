use crate::magic::{self, MagicHandler, MagicLine, Output};

// ---------------------------------------------------------------------------
// %time — Time a single R expression
// ---------------------------------------------------------------------------

pub struct Time;

impl MagicHandler for Time {
    fn name(&self) -> &'static str {
        "time"
    }

    fn description(&self) -> &'static str {
        "Time a single R expression"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %time <R expression>".into(),
            });
        }

        // Use capture.output to get the printed timing from system.time()
        let code = format!(
            "cat(capture.output(system.time({{ {} }}))[2L], sep = '\n')",
            expr
        );

        let result = crate::r_runtime::eval_string_raw_global(&code)
            .map_err(|e| magic::MagicError {
                message: format!("R evaluation failed: {e}"),
            })?;

        Ok(Output::Text(result))
    }
}

// ---------------------------------------------------------------------------
// %timeit — Time an expression N times (default 7)
// ---------------------------------------------------------------------------

pub struct TimeIt;

impl MagicHandler for TimeIt {
    fn name(&self) -> &'static str {
        "timeit"
    }

    fn description(&self) -> &'static str {
        "Time an R expression multiple times (default 7), report min/mean/max"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args = line.args.trim();

        let (n, expr) = if let Some(rest) = args.strip_prefix("-n ") {
            let rest = rest.trim();
            let space = rest.find(' ').unwrap_or(rest.len());
            let n_str = &rest[..space];
            let n: usize = n_str.parse().map_err(|_| magic::MagicError {
                message: format!("Invalid iteration count: {n_str}"),
            })?;
            let expr = rest[space..].trim();
            (n, expr)
        } else {
            (7, args)
        };

        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %timeit [-n <count>] <R expression>".into(),
            });
        }

        let mut times: Vec<f64> = Vec::with_capacity(n);

        for i in 0..n {
            // Run system.time and extract just the elapsed seconds
            let code = format!(
                "cat({{ t <- system.time({{ {} }}); sprintf('%.6f', t[3]) }})",
                expr
            );
            let result = crate::r_runtime::eval_string_raw_global(&code)
                .map_err(|e| magic::MagicError {
                    message: format!("R evaluation failed (iteration {i}): {e}"),
                })?;
            if let Ok(t) = result.trim().parse::<f64>() {
                times.push(t);
            }
        }

        if times.is_empty() {
            return Err(magic::MagicError {
                message: "No timing data collected".into(),
            });
        }

        let min = times.iter().cloned().fold(f64::MAX, f64::min);
        let max = times.iter().cloned().fold(f64::MIN, f64::max);
        let mean = times.iter().sum::<f64>() / times.len() as f64;

        Ok(Output::Text(format!(
            "{} loops: min: {:.4}s  mean: {:.4}s  max: {:.4}s\n",
            n, min, mean, max
        )))
    }
}

// ---------------------------------------------------------------------------
// %prun — Profile an R expression
// ---------------------------------------------------------------------------

pub struct Prun;

impl MagicHandler for Prun {
    fn name(&self) -> &'static str {
        "prun"
    }

    fn description(&self) -> &'static str {
        "Profile an R expression using Rprof"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %prun <R expression>".into(),
            });
        }

        // Start profiler
        let start_code = r#"Rprof(tmp <- tempfile(fileext = ".Rprof"))"#;
        crate::r_runtime::eval_string_raw_global(start_code)
            .map_err(|e| magic::MagicError {
                message: format!("Failed to start profiler: {e}"),
            })?;

        // Run the expression
        let eval_code = format!("{{ {} }}", expr);
        let eval_result = crate::r_runtime::eval_string_raw_global(&eval_code);

        // Stop profiler
        let stop_code = "Rprof(NULL)";
        let _ = crate::r_runtime::eval_string_raw_global(stop_code);

        // Propagate eval error if any
        if let Err(e) = eval_result {
            return Err(magic::MagicError {
                message: format!("Expression failed: {e}"),
            });
        }

        // Get profiling summary
        let summary_code = r#"
            cat(paste0(capture.output(summaryRprof(tmp)), collapse = "\n"))
            unlink(tmp)
        "#;
        let summary = crate::r_runtime::eval_string_raw_global(summary_code)
            .map_err(|e| magic::MagicError {
                message: format!("Failed to get profiling summary: {e}"),
            })?;

        Ok(Output::Text(summary))
    }
}
