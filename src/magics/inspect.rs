use super::r_utils;
use crate::magic::{self, MagicHandler, MagicLine, Output};
use crate::r_runtime;
use comfy_table::{Attribute, Cell, CellAlignment, ContentArrangement, Table, presets::UTF8_FULL};

// ---------------------------------------------------------------------------
// %objects — List R objects (like `ls()`)
// ---------------------------------------------------------------------------

pub struct Objects;

impl MagicHandler for Objects {
    fn name(&self) -> &'static str {
        "objects"
    }
    fn description(&self) -> &'static str {
        "List objects in the global environment (like ls())"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let pattern = if line.args.is_empty() {
            String::new()
        } else {
            format!("pattern=\"{}\"", line.args)
        };
        r_utils::eval_r_captured(&format!("ls({pattern})"))
    }
}

// ---------------------------------------------------------------------------
// %pdoc — Print object documentation
// ---------------------------------------------------------------------------

pub struct Pdoc;

impl MagicHandler for Pdoc {
    fn name(&self) -> &'static str {
        "pdoc"
    }
    fn description(&self) -> &'static str {
        "Print documentation for an object (help)"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %pdoc <topic>".into(),
            });
        }
        r_utils::eval_r_captured(&format!(r#"tools::Rd2txt(utils::help("{}"))"#, line.args))
    }
}

// ---------------------------------------------------------------------------
// %pdef — Print function definition/signature
// ---------------------------------------------------------------------------

pub struct Pdef;

impl MagicHandler for Pdef {
    fn name(&self) -> &'static str {
        "pdef"
    }
    fn description(&self) -> &'static str {
        "Print function signature (formal arguments)"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %pdef <function>".into(),
            });
        }
        r_utils::eval_r_captured(&format!("args({})", line.args))
    }
}

// ---------------------------------------------------------------------------
// %psource — Print source code of a function
// ---------------------------------------------------------------------------

pub struct Psource;

impl MagicHandler for Psource {
    fn name(&self) -> &'static str {
        "psource"
    }
    fn description(&self) -> &'static str {
        "Print source code of a function"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %psource <function>".into(),
            });
        }
        // deparse() gives lines of source; capture.output() joins them
        r_utils::eval_r_captured(&format!("cat(deparse({}), sep=\"\\n\")", line.args))
    }
}

// ---------------------------------------------------------------------------
// %pfile — Print file where an object is defined
// ---------------------------------------------------------------------------

pub struct Pfile;

impl MagicHandler for Pfile {
    fn name(&self) -> &'static str {
        "pfile"
    }
    fn description(&self) -> &'static str {
        "Show the file where an object is defined"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %pfile <object>".into(),
            });
        }
        r_utils::eval_r_captured(&format!(
            r#"cat(attr(attr({}, "srcref"), "srcfile")$filename, "\n")"#,
            line.args
        ))
    }
}

// ---------------------------------------------------------------------------
// %who — Filtered object listing by type
// ---------------------------------------------------------------------------

pub struct Who;

impl MagicHandler for Who {
    fn name(&self) -> &'static str {
        "who"
    }
    fn description(&self) -> &'static str {
        "List objects, optionally filtered by class (e.g. %who data.frame)"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return r_utils::eval_r_captured("ls()");
        }
        r_utils::eval_r_captured(&format!(
            r#"Filter(function(x) inherits(get(x), "{}"), ls())"#,
            line.args
        ))
    }
}

// ---------------------------------------------------------------------------
// %whos — Detailed object listing (Name, Class, Length, Size)
// ---------------------------------------------------------------------------

pub struct Whos;

impl MagicHandler for Whos {
    fn name(&self) -> &'static str {
        "whos"
    }
    fn description(&self) -> &'static str {
        "Detailed object listing: name, class, length, size"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let pattern = if line.args.is_empty() {
            String::new()
        } else {
            format!("pattern=\"{}\"", line.args)
        };
        r_utils::eval_r_captured(&format!(
            r#"
local({{
    objs <- ls({pattern}, envir=.GlobalEnv)
    if (length(objs) == 0L) cat("(empty workspace)\n") else {{
        for (nm in objs) {{
            obj <- get(nm, envir=.GlobalEnv)
            cl <- paste(class(obj), collapse=", ")
            sz <- tryCatch(object.size(obj), error=function(e) NA)
            if (!is.na(sz)) sz <- format(sz, units="auto") else sz <- "?"
            len <- if (is.null(dim(obj))) length(obj) else paste(dim(obj), collapse="x")
            cat(sprintf("%-20s %-15s %-10s %s\n", nm, cl, len, sz))
        }}
    }}
}})
"#,
            pattern = pattern
        ))
    }
}

// ---------------------------------------------------------------------------
// %who_ls — Sorted object names (one per line)
// ---------------------------------------------------------------------------

pub struct WhoLs;

impl MagicHandler for WhoLs {
    fn name(&self) -> &'static str {
        "who_ls"
    }
    fn description(&self) -> &'static str {
        "Return sorted object names (one per line)"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let pattern = if line.args.is_empty() {
            String::new()
        } else {
            format!("pattern=\"{}\"", line.args)
        };
        r_utils::eval_r_captured(&format!("cat(sort(ls({pattern})), sep=\"\\n\")"))
    }
}

// ---------------------------------------------------------------------------
// %rm — Remove specific objects
// ---------------------------------------------------------------------------

pub struct Rm;

impl MagicHandler for Rm {
    fn name(&self) -> &'static str {
        "rm"
    }
    fn description(&self) -> &'static str {
        "Remove objects from the global environment"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %rm <name1> <name2> ...".into(),
            });
        }
        r_utils::eval_r_captured(&format!("rm(list=c({}))", line.args))
    }
}

// ---------------------------------------------------------------------------
// %clear — Remove all objects
// ---------------------------------------------------------------------------

pub struct Clear;

impl MagicHandler for Clear {
    fn name(&self) -> &'static str {
        "clear"
    }
    fn description(&self) -> &'static str {
        "Remove all objects from the global environment"
    }
    fn run(&self, _line: &MagicLine) -> Result<Output, magic::MagicError> {
        r_utils::eval_r_captured("rm(list=ls(envir=.GlobalEnv), envir=.GlobalEnv)")
    }
}

// ---------------------------------------------------------------------------
// %str — Display structure of an object
// ---------------------------------------------------------------------------

pub struct Str;

impl MagicHandler for Str {
    fn name(&self) -> &'static str {
        "str"
    }
    fn description(&self) -> &'static str {
        "Display structure of an R object"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %str <expression>".into(),
            });
        }
        // str() outputs to stderr by default — use str(..., give.attr=FALSE)
        r_utils::eval_r_captured(&format!("utils::str({}, give.attr=FALSE)", line.args))
    }
}

// ---------------------------------------------------------------------------
// %head — Show first few rows
// ---------------------------------------------------------------------------

pub struct Head;

impl MagicHandler for Head {
    fn name(&self) -> &'static str {
        "head"
    }
    fn description(&self) -> &'static str {
        "Show the first few rows of an object"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %head <expression>".into(),
            });
        }
        r_utils::eval_r_captured(&format!("head({})", line.args))
    }
}

// ---------------------------------------------------------------------------
// %skim — skimr::skim
// ---------------------------------------------------------------------------

pub struct Skim;

impl MagicHandler for Skim {
    fn name(&self) -> &'static str {
        "skim"
    }
    fn description(&self) -> &'static str {
        "Summarize a data frame with skimr::skim()"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %skim <data.frame>".into(),
            });
        }
        r_utils::eval_with_pkg_check(&format!("skimr::skim({})", line.args), "skimr")
    }
}

// ---------------------------------------------------------------------------
// %dim — Show dimensions
// ---------------------------------------------------------------------------

pub struct Dim;

impl MagicHandler for Dim {
    fn name(&self) -> &'static str {
        "dim"
    }
    fn description(&self) -> &'static str {
        "Show dimensions of an object"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %dim <expression>".into(),
            });
        }
        r_utils::eval_r_captured(&format!(r#"cat(deparse(dim({})), "\n")"#, line.args))
    }
}

// ---------------------------------------------------------------------------
// %names — Show column/variable names
// ---------------------------------------------------------------------------

pub struct Names;

impl MagicHandler for Names {
    fn name(&self) -> &'static str {
        "names"
    }
    fn description(&self) -> &'static str {
        "Show names (column names) of an object"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %names <expression>".into(),
            });
        }
        r_utils::eval_r_captured(&format!("names({})", line.args))
    }
}

// ---------------------------------------------------------------------------
// %plot — Plot an expression
// ---------------------------------------------------------------------------

pub struct Plot;

impl MagicHandler for Plot {
    fn name(&self) -> &'static str {
        "plot"
    }
    fn description(&self) -> &'static str {
        "Plot an expression (opens graphics device)"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %plot <expression>".into(),
            });
        }
        r_runtime::eval_string_raw_global(&format!("plot({})", line.args)).map_err(|e| {
            magic::MagicError {
                message: e.to_string(),
            }
        })?;
        Ok(Output::Text("Plot sent to graphics device.\n".into()))
    }
}

// ---------------------------------------------------------------------------
// %tidy — broom::tidy
// ---------------------------------------------------------------------------

pub struct Tidy;

impl MagicHandler for Tidy {
    fn name(&self) -> &'static str {
        "tidy"
    }
    fn description(&self) -> &'static str {
        "Tidy model output with broom::tidy()"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %tidy <model>".into(),
            });
        }
        r_utils::eval_with_pkg_check(&format!("broom::tidy({})", line.args), "broom")
    }
}

// ---------------------------------------------------------------------------
// %View — View data in spreadsheet
// ---------------------------------------------------------------------------

pub struct View;

impl MagicHandler for View {
    fn name(&self) -> &'static str {
        "View"
    }
    fn description(&self) -> &'static str {
        "View an object in the spreadsheet viewer (utils::View)"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        if line.args.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %View <expression>".into(),
            });
        }
        r_runtime::eval_string_raw_global(&format!("utils::View({})", line.args)).map_err(|e| {
            magic::MagicError {
                message: e.to_string(),
            }
        })?;
        Ok(Output::Silent)
    }
}

// ---------------------------------------------------------------------------
// %inspect — Render any R object as a formatted text table
// ---------------------------------------------------------------------------

/// Parse tab-separated structured data from R into a comfy-table.
fn render_tabular(data: &str, expr: &str) -> String {
    let lines: Vec<&str> = data.lines().collect();
    if lines.len() < 2 {
        return format!("(empty result for {expr})\n");
    }

    let header_parts: Vec<&str> = lines[0].split('\t').collect();
    if header_parts.is_empty() || header_parts.len() < 3 {
        return format!("(unexpected data format: {data})\n");
    }
    let class_name = header_parts[0];
    let nrow: usize = header_parts[1].parse().unwrap_or(0);
    let ncol: usize = header_parts[2].parse().unwrap_or(0);

    let col_names: Vec<&str> = if lines.len() > 1 {
        lines[1].split('\t').collect()
    } else {
        Vec::new()
    };

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth);

    // Header row
    let header_cells: Vec<Cell> = col_names
        .iter()
        .map(|name| {
            Cell::new(name)
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold)
        })
        .collect();
    table.set_header(header_cells);

    // Data rows
    for line in lines.iter().skip(2) {
        let values: Vec<&str> = line.split('\t').collect();
        let row_cells: Vec<Cell> = values.iter().map(Cell::new).collect();
        table.add_row(row_cells);
    }

    // Footer summary
    let footer = if nrow > ncol {
        format!(
            "\nShowing {} of {} rows, {} columns ({class_name})",
            nrow.min(20),
            nrow,
            ncol
        )
    } else {
        format!("\nShowing {} columns", ncol)
    };

    format!("{table}{footer}\n")
}

pub(crate) fn build_inspect_code(expr: &str) -> String {
    format!(
        r#"local({{
  x <- {expr}
  if (is.data.frame(x) || is.matrix(x)) {{
    h <- utils::head(x, 20)
    nr <- NROW(x)
    nc <- NCOL(x)
    cls <- class(x)[1]
    cn <- colnames(x)
    if (is.null(cn)) cn <- paste0("V", seq_len(nc))
    data_lines <- apply(h, 1, function(r) paste(ifelse(is.na(r), "NA", as.character(r)), collapse = "\t"))
    paste(c(
      paste(cls, nr, nc, sep = "\t"),
      paste(cn, collapse = "\t"),
      data_lines
    ), collapse = "\n")
  }} else {{
    paste("no-table", paste(capture.output(str(x, give.attr = FALSE)), collapse = "\n"), sep = "\t")
  }}
}})"#,
        expr = expr
    )
}

pub struct Inspect;

impl MagicHandler for Inspect {
    fn name(&self) -> &'static str {
        "inspect"
    }
    fn description(&self) -> &'static str {
        "Render any R object as a formatted text table"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let expr = line.args.trim();
        if expr.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %inspect <R expression>".into(),
            });
        }

        #[cfg(feature = "tui")]
        {
            // Try TUI mode first. If the object is non-tabular, fall through
            // to the comfy-table text rendering below.
            match crate::magics::inspect_tui::fetch_inspect_data(expr) {
                Ok(data) => {
                    crate::magics::inspect_tui::run_tui_inspect(data)
                        .unwrap_or_else(|e| eprintln!("TUI inspect error: {e}"));
                    return Ok(Output::Silent);
                }
                Err(e) if e.message.contains("not tabular") => {
                    // Fall through to non-TUI path
                }
                Err(e) => return Err(e),
            }
        }

        // Non-TUI fallback (also used when `tui` feature is disabled)
        let code = build_inspect_code(expr);
        let result = r_runtime::eval_string_raw_global(&code).map_err(|e| magic::MagicError {
            message: e.to_string(),
        })?;

        if result.is_empty() {
            return Ok(Output::Text(format!("(empty result for {expr})\n")));
        }

        // Check if the result is a non-tabular fallback
        if let Some(rest) = result.strip_prefix("no-table\t") {
            // Non-tabular object — show str output with a simple header
            let output = format!("── {expr} ──\n{rest}\n");
            return Ok(Output::Text(output));
        }

        let rendered = render_tabular(&result, expr);
        Ok(Output::Text(rendered))
    }
}

// ---------------------------------------------------------------------------
// %methods — Show S3/S4 methods for a generic function or class
// ---------------------------------------------------------------------------

pub struct Methods;

impl MagicHandler for Methods {
    fn name(&self) -> &'static str {
        "methods"
    }
    fn description(&self) -> &'static str {
        "Show S3/S4 methods for a generic function or class"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let name = line.args.trim();
        if name.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %methods <function_or_class>".into(),
            });
        }
        r_utils::eval_r_captured(&format!("methods({name})"))
    }
}

// ---------------------------------------------------------------------------
// %psearch — Pattern-based object search (find + apropos)
// ---------------------------------------------------------------------------

pub struct Psearch;

impl MagicHandler for Psearch {
    fn name(&self) -> &'static str {
        "psearch"
    }
    fn description(&self) -> &'static str {
        "Search for objects matching a pattern using find() and apropos()"
    }
    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let pattern = line.args.trim();
        if pattern.is_empty() {
            return Err(magic::MagicError {
                message: "Usage: %psearch <pattern>".into(),
            });
        }
        r_utils::eval_r_captured(&format!(
            r#"cat("=== find('{}') ===\n", sep=""); cat(find("{}"), sep="\n"); cat("\n=== apropos('{}') ===\n", sep=""); cat(apropos("{}", ignore.case = TRUE), sep="\n")"#,
            pattern, pattern, pattern, pattern
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tsv() -> &'static str {
        "data.frame\t32\t11\nmpg\tcyl\tdisp\t hp\n21.0\t6\t160\t110\n21.0\t6\t160\t110\n22.8\t4\t108\t93"
    }

    #[test]
    fn render_tabular_includes_column_names() {
        let output = render_tabular(sample_tsv(), "mtcars");
        assert!(output.contains("mpg"), "should contain column name 'mpg'");
        assert!(output.contains("cyl"), "should contain column name 'cyl'");
        assert!(output.contains("disp"), "should contain column name 'disp'");
    }

    #[test]
    fn render_tabular_includes_data_values() {
        let output = render_tabular(sample_tsv(), "mtcars");
        assert!(output.contains("21.0"), "should contain data value '21.0'");
        assert!(output.contains("22.8"), "should contain data value '22.8'");
    }

    #[test]
    fn render_tabular_includes_footer() {
        let output = render_tabular(sample_tsv(), "mtcars");
        assert!(output.contains("32"), "footer should show 32 rows");
        assert!(output.contains("11"), "footer should show 11 columns");
        assert!(output.contains("data.frame"), "footer should show class");
    }

    #[test]
    fn render_tabular_handles_single_row() {
        let data = "data.frame\t1\t2\ncol1\tcol2\nval1\tval2";
        let output = render_tabular(data, "x");
        assert!(output.contains("col1"));
        assert!(output.contains("val1"));
    }

    #[test]
    fn render_tabular_handles_fewer_columns_in_data() {
        let data = "data.frame\t3\t2\nA\tB\n1\t2\n3";
        // This shouldn't panic, just render what we have
        let output = render_tabular(data, "x");
        assert!(output.contains("A"));
    }

    #[test]
    fn render_tabular_empty_data_returns_fallback() {
        let output = render_tabular("", "x");
        assert!(output.contains("empty result"));
    }

    #[test]
    fn render_tabular_malformed_header_does_not_panic() {
        let output = render_tabular("just one line", "x");
        assert!(!output.is_empty());
    }

    #[test]
    fn build_inspect_code_contains_expression() {
        let code = build_inspect_code("mtcars");
        assert!(code.contains("mtcars"));
    }

    #[test]
    fn build_inspect_code_contains_is_data_dot_frame() {
        let code = build_inspect_code("x");
        assert!(code.contains("is.data.frame"));
    }

    #[test]
    fn build_inspect_code_contains_no_table_fallback() {
        let code = build_inspect_code("x");
        assert!(code.contains("no-table"));
    }

    #[test]
    fn methods_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("methods").is_some());
    }

    #[test]
    fn methods_empty_args_returns_error() {
        let handler = Methods;
        let line = MagicLine {
            name: "methods".into(),
            args: "".into(),
            is_cell: false,
        };
        assert!(handler.run(&line).is_err());
    }

    #[test]
    fn psearch_registered() {
        let reg = crate::magic::magic_registry().lock().unwrap();
        assert!(reg.get("psearch").is_some());
    }

    #[test]
    fn psearch_empty_args_returns_error() {
        let handler = Psearch;
        let line = MagicLine {
            name: "psearch".into(),
            args: "".into(),
            is_cell: false,
        };
        assert!(handler.run(&line).is_err());
    }
}
