//! `%connections` — DBI connection browser.
//!
//! Lists active DBI connections in the global environment and inspects their
//! tables and fields.

use crate::magic::{self, MagicHandler, MagicLine, Output};
use crate::r_runtime::eval_string_raw_global;

pub struct Connections;

impl MagicHandler for Connections {
    fn name(&self) -> &'static str {
        "connections"
    }

    fn description(&self) -> &'static str {
        "List DBI connections, tables, and fields"
    }

    fn run(&self, line: &MagicLine) -> Result<Output, magic::MagicError> {
        let args: Vec<&str> = line.args.split_whitespace().collect();

        match args.as_slice() {
            [] => {
                // List all DBI connections in the global environment.
                let r_code = r#"
local({
  objs <- ls(envir = .GlobalEnv)
  conns <- list()
  for (o in objs) {
    obj <- get(o, envir = .GlobalEnv)
    if (inherits(obj, "DBIConnection")) {
      info <- tryCatch({
        list(name = o, class = paste(class(obj), collapse = ", "),
             connected = if (DBI::dbIsValid(obj)) "yes" else "no")
      }, error = function(e) list(name = o, class = "?", connected = "?"))
      conns[[length(conns) + 1]] <- info
    }
  }
  if (length(conns) == 0) {
    cat("No DBI connections found in the global environment.\n")
  } else {
    cat(sprintf("%-20s %-30s  %s\n", "Name", "Class", "Connected"))
    cat(paste(rep("-", 65), collapse = ""), "\n", sep = "")
    for (c in conns) {
      cat(sprintf("%-20s %-30s  %s\n", c$name, c$class, c$connected))
    }
  }
})
"#;
                let output = eval_string_raw_global(r_code).map_err(|e| magic::MagicError {
                    message: e.to_string(),
                })?;
                Ok(Output::Text(output))
            }
            [conn_name] => {
                // List tables for the given connection.
                let r_code = format!(
                    r#"local({{
  conn <- get("{conn_name}", envir = .GlobalEnv)
  if (!inherits(conn, "DBIConnection")) {{
    cat("Error: {conn_name} is not a DBI connection object.\n")
  }} else if (!DBI::dbIsValid(conn)) {{
    cat("Error: connection {conn_name} is closed.\n")
  }} else {{
    tables <- DBI::dbListTables(conn)
    if (length(tables) == 0) {{
      cat("No tables found in connection {conn_name}.\n")
    }} else {{
      cat(sprintf("Tables in %s (%d):\n", "{conn_name}", length(tables)))
      for (t in tables) cat("  ", t, "\n", sep = "")
    }}
  }}
}})
"#
                );
                let output = eval_string_raw_global(&r_code).map_err(|e| magic::MagicError {
                    message: e.to_string(),
                })?;
                Ok(Output::Text(output))
            }
            [conn_name, table_name] => {
                // List fields for the given table in the given connection.
                let r_code = format!(
                    r#"local({{
  conn <- get("{conn_name}", envir = .GlobalEnv)
  if (!inherits(conn, "DBIConnection")) {{
    cat("Error: {conn_name} is not a DBI connection object.\n")
  }} else if (!DBI::dbIsValid(conn)) {{
    cat("Error: connection {conn_name} is closed.\n")
  }} else {{
    fields <- DBI::dbListFields(conn, "{table_name}")
    if (length(fields) == 0) {{
      cat("No fields found for table {table_name}.\n")
    }} else {{
      cat(sprintf("Fields in {conn_name}.{table_name} (%d):\n", length(fields)))
      for (f in fields) cat("  ", f, "\n", sep = "")
    }}
  }}
}})
"#
                );
                let output = eval_string_raw_global(&r_code).map_err(|e| magic::MagicError {
                    message: e.to_string(),
                })?;
                Ok(Output::Text(output))
            }
            _ => Err(magic::MagicError {
                message: "Usage: %connections [<conn_name> [<table_name>]]".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connections_no_args() {
        let handler = Connections;
        let line = MagicLine {
            name: "connections".into(),
            args: "".into(),
            is_cell: false,
        };
        // This will try to evaluate in R, which may not be available in tests.
        // We accept either success or an R-not-available error.
        match handler.run(&line) {
            Ok(Output::Text(_)) => {} // success (R available, connections may or may not exist)
            Err(e) => {
                assert!(
                    e.message.contains("R is not initialized"),
                    "unexpected error: {}",
                    e.message
                );
            }
            Ok(_) => panic!("unexpected Output variant"),
        }
    }

    #[test]
    fn test_connections_invalid_name_errors() {
        let handler = Connections;
        let line = MagicLine {
            name: "connections".into(),
            args: "nonexistent_conn".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        // Should get either an R-not-available error or a DBI evaluation result.
        match result {
            Err(e) => {
                assert!(
                    e.message.contains("R is not initialized"),
                    "unexpected error: {}",
                    e.message
                );
            }
            Ok(_) => {} // R ran, may have failed at R level — still valid
        }
    }

    #[test]
    fn test_connections_too_many_args() {
        let handler = Connections;
        let line = MagicLine {
            name: "connections".into(),
            args: "conn_a table_a extra_arg".into(),
            is_cell: false,
        };
        let result = handler.run(&line);
        assert!(result.is_err(), "expected error for too many args");
        assert!(
            result.unwrap_err().message.contains("Usage:"),
            "expected Usage message"
        );
    }
}
