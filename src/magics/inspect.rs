use crate::magic::{self, MagicHandler, MagicLine, Output};
use crate::r_runtime;

fn eval_r_captured(code: &str) -> Result<Output, magic::MagicError> {
    let wrapped = format!("capture.output({code})");
    let text = r_runtime::eval_string_raw_global(&wrapped).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    Ok(Output::Text(text))
}

fn eval_with_pkg_check(code: &str, pkg: &str) -> Result<Output, magic::MagicError> {
    let check = format!(
        "if (!requireNamespace('{pkg}', quietly=TRUE)) stop('package {pkg} is not installed')"
    );
    r_runtime::eval_string_raw_global(&check).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;
    eval_r_captured(code)
}

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
        eval_r_captured(&format!("ls({pattern})"))
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
        eval_r_captured(&format!(r#"tools::Rd2txt(utils::help("{}"))"#, line.args))
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
        eval_r_captured(&format!("args({})", line.args))
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
        eval_r_captured(&format!("cat(deparse({}), sep=\"\\n\")", line.args))
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
        eval_r_captured(&format!(
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
            return eval_r_captured("ls()");
        }
        eval_r_captured(&format!(
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
        eval_r_captured(&format!(
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
        eval_r_captured(&format!("cat(sort(ls({pattern})), sep=\"\\n\")"))
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
        eval_r_captured(&format!("rm(list=c({}))", line.args))
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
        eval_r_captured("rm(list=ls(envir=.GlobalEnv), envir=.GlobalEnv)")
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
        eval_r_captured(&format!("utils::str({}, give.attr=FALSE)", line.args))
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
        eval_r_captured(&format!("head({})", line.args))
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
        eval_with_pkg_check(&format!("skimr::skim({})", line.args), "skimr")
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
        eval_r_captured(&format!(r#"cat(deparse(dim({})), "\n")"#, line.args))
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
        eval_r_captured(&format!("names({})", line.args))
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
        eval_with_pkg_check(&format!("broom::tidy({})", line.args), "broom")
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
