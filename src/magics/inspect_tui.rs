//! Interactive ratatui-based data inspector for `%inspect`.
//!
//! This module is compiled only when the `tui` feature is enabled.
//! It provides an interactive TUI popup for browsing data frames,
//! matrices, and other tabular R objects with scrolling, sorting,
//! and cell preview.

use crate::magic;
use crate::r_runtime;
use crossterm::event::{KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, HighlightSpacing, Paragraph, Row as TableRow, Table, TableState,
    Wrap,
};
use std::io;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Structured data parsed from the R-side TSV output of `build_inspect_code()`.
pub struct InspectData {
    pub class: String,
    pub nrow: usize,
    pub ncol: usize,
    pub col_names: Vec<String>,
    pub rows: Vec<Row>,
}

/// A single data row.
#[derive(Clone)]
pub struct Row {
    pub values: Vec<String>,
}

// ---------------------------------------------------------------------------
// Sort support
// ---------------------------------------------------------------------------

/// Sort direction for a column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Current sort state: which column and which direction.
#[derive(Clone, Debug)]
pub struct SortState {
    pub column: usize,
    pub direction: SortDirection,
}

impl InspectData {
    /// Sort rows in-place by the given column and direction.
    pub fn sort_by(&mut self, col: usize, dir: SortDirection) {
        self.rows.sort_by(|a, b| {
            let a_val = a.values.get(col).map(|s| s.as_str()).unwrap_or("");
            let b_val = b.values.get(col).map(|s| s.as_str()).unwrap_or("");
            let cmp = if let (Ok(a_f), Ok(b_f)) = (a_val.parse::<f64>(), b_val.parse::<f64>()) {
                a_f.partial_cmp(&b_f).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a_val.cmp(b_val)
            };
            match dir {
                SortDirection::Ascending => cmp,
                SortDirection::Descending => cmp.reverse(),
            }
        });
    }
}

// ---------------------------------------------------------------------------
// TSV parsing
// ---------------------------------------------------------------------------

/// Parse the TSV output from `build_inspect_code()`.
///
/// Format:
///   Line 0: `<class>\t<nrow>\t<ncol>`
///   Line 1: `<col_name_1>\t<col_name_2>\t...`
///   Line 2+: `<val_1>\t<val_2>\t...`
pub fn parse_inspect_output(data: &str) -> Result<InspectData, String> {
    let lines: Vec<&str> = data.lines().collect();
    if lines.len() < 2 {
        return Err("not enough lines in inspect output".into());
    }

    let meta: Vec<&str> = lines[0].split('\t').collect();
    if meta.len() < 3 {
        return Err("missing metadata in inspect output".into());
    }
    let class = meta[0].to_string();
    let nrow: usize = meta[1]
        .parse()
        .map_err(|_| format!("invalid nrow: {}", meta[1]))?;
    let ncol: usize = meta[2]
        .parse()
        .map_err(|_| format!("invalid ncol: {}", meta[2]))?;

    let col_names: Vec<String> = lines[1].split('\t').map(|s| s.to_string()).collect();

    let rows: Vec<Row> = lines[2..]
        .iter()
        .map(|line| Row {
            values: line.split('\t').map(|s| s.to_string()).collect(),
        })
        .collect();

    Ok(InspectData {
        class,
        nrow,
        ncol,
        col_names,
        rows,
    })
}

// ---------------------------------------------------------------------------
// R data fetching
// ---------------------------------------------------------------------------

/// Fetch data from R via `build_inspect_code` and parse into `InspectData`.
pub fn fetch_inspect_data(expr: &str) -> Result<InspectData, magic::MagicError> {
    let code = super::inspect::build_inspect_code(expr);
    let result = r_runtime::eval_string_raw_global(&code).map_err(|e| magic::MagicError {
        message: e.to_string(),
    })?;

    if result.is_empty() {
        return Err(magic::MagicError {
            message: format!("(empty result for {expr})"),
        });
    }

    // Check for non-tabular fallback from R side
    if result.starts_with("no-table\t") {
        return Err(magic::MagicError {
            message: "Object is not tabular — use `%inspect` (non-TUI) to view".into(),
        });
    }

    parse_inspect_output(&result).map_err(|e| magic::MagicError {
        message: format!("Failed to parse R output: {e}"),
    })
}

// ---------------------------------------------------------------------------
// TUI entry point
// ---------------------------------------------------------------------------

/// Maximum length of a cell value displayed inline before truncation.
const MAX_CELL_WIDTH: usize = 30;

/// Characters to show at the end of a truncated cell.
const TRUNCATION_SUFFIX: &str = "…";

/// Page step for PageUp / PageDown.
const PAGE_SCROLL: usize = 10;

/// Enter TUI mode, render the interactive inspection table, block until
/// the user exits, then restore the terminal and return.
pub fn run_tui_inspect(data: InspectData) -> Result<(), String> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    let mut stdout = io::stdout();
    enable_raw_mode().map_err(|e| format!("failed to enter raw mode: {e}"))?;
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen).map_err(|e| {
        let _ = disable_raw_mode();
        format!("failed to enter alternate screen: {e}")
    })?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen);
        format!("failed to create ratatui terminal: {e}")
    })?;

    // Restore terminal state on all exit paths.
    let result = run_event_loop(&mut terminal, data);

    let mut restore = || -> Result<(), String> {
        crossterm::terminal::disable_raw_mode()
            .map_err(|e| format!("failed to disable raw mode: {e}"))?;
        crossterm::execute!(
            terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen
        )
        .map_err(|e| format!("failed to leave alternate screen: {e}"))
    };

    if let Err(e) = restore() {
        // Best-effort: if restore partially fails, print warning but don't
        // overwrite the original error (if any).
        eprintln!("Warning: terminal restore failed: {e}");
    }

    result
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

fn run_event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    mut data: InspectData,
) -> Result<(), String> {
    let mut table_state = TableState::default();
    let mut sort_state: Option<SortState> = None;
    let mut show_preview: Option<(usize, usize)> = None; // (row, col)
    let mut h_scroll: u16 = 0;

    // Track the original row order for unsorting
    let original_rows: Vec<Row> = data.rows.clone();

    loop {
        terminal
            .draw(|f| {
                ui(
                    f,
                    &data,
                    &mut table_state,
                    &sort_state,
                    show_preview,
                    h_scroll,
                );
            })
            .map_err(|e| format!("draw failed: {e}"))?;

        let event = crossterm::event::read().map_err(|e| format!("event read failed: {e}"))?;

        if let crossterm::event::Event::Key(key) = event {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // If showing a preview popup, any key dismisses it
            if show_preview.is_some() {
                show_preview = None;
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = table_state.selected().unwrap_or(0);
                    table_state.select(Some(i.saturating_sub(1)));
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = table_state.selected().unwrap_or(0);
                    if data.rows.is_empty() {
                        table_state.select(None);
                    } else {
                        table_state.select(Some((i + 1).min(data.rows.len() - 1)));
                    }
                }
                KeyCode::PageUp => {
                    let i = table_state.selected().unwrap_or(0);
                    table_state.select(Some(i.saturating_sub(PAGE_SCROLL)));
                }
                KeyCode::PageDown => {
                    let i = table_state.selected().unwrap_or(0);
                    if data.rows.is_empty() {
                        table_state.select(None);
                    } else {
                        table_state.select(Some((i + PAGE_SCROLL).min(data.rows.len() - 1)));
                    }
                }
                KeyCode::Home => {
                    table_state.select(Some(0));
                }
                KeyCode::End => {
                    if data.rows.is_empty() {
                        table_state.select(None);
                    } else {
                        table_state.select(Some(data.rows.len() - 1));
                    }
                }
                KeyCode::Left => {
                    h_scroll = h_scroll.saturating_sub(4);
                }
                KeyCode::Right => {
                    h_scroll = h_scroll.saturating_add(4);
                }
                KeyCode::Char('s') => {
                    // Sort: cycle through columns, toggling direction
                    if data.ncol == 0 {
                        continue;
                    }
                    let current = sort_state.as_ref().map(|s| s.column).unwrap_or(0);
                    let next_col = (current + 1) % data.ncol;
                    let dir = if sort_state
                        .as_ref()
                        .map(|s| s.column == next_col)
                        .unwrap_or(false)
                    {
                        match sort_state.as_ref().unwrap().direction {
                            SortDirection::Ascending => SortDirection::Descending,
                            SortDirection::Descending => {
                                // Third press: reset to original order
                                data.rows = original_rows.clone();
                                sort_state = None;
                                table_state.select(Some(0));
                                continue;
                            }
                        }
                    } else {
                        SortDirection::Ascending
                    };
                    data.sort_by(next_col, dir);
                    sort_state = Some(SortState {
                        column: next_col,
                        direction: dir,
                    });
                    table_state.select(Some(0));
                }
                KeyCode::Char('u') => {
                    // Unsort: restore original order
                    data.rows = original_rows.clone();
                    sort_state = None;
                    table_state.select(Some(0));
                }
                KeyCode::Enter => {
                    if let Some(i) = table_state.selected()
                        && i < data.rows.len()
                    {
                        // Determine which cell to preview:
                        // use the sort column if sorting, otherwise column 0
                        let col = sort_state.as_ref().map(|s| s.column).unwrap_or(0);
                        show_preview = Some((i, col));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// UI rendering
// ---------------------------------------------------------------------------

fn ui(
    f: &mut Frame,
    data: &InspectData,
    table_state: &mut TableState,
    sort_state: &Option<SortState>,
    show_preview: Option<(usize, usize)>,
    _h_scroll: u16,
) {
    let area = f.area();

    // If there are no rows, show a simple message
    if data.col_names.is_empty() {
        let msg = Paragraph::new("(empty data set)")
            .block(Block::default().borders(Borders::ALL).title(" Inspect "))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(msg, area);
        return;
    }

    // Build column constraints: distribute space proportionally
    let col_widths: Vec<Constraint> = (0..data.ncol)
        .map(|_| Constraint::Percentage(100 / data.ncol.max(1) as u16))
        .collect();

    // Build header cells
    let header_cells: Vec<Cell> = data
        .col_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let mut cell = Cell::from(name.as_str()).style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .fg(Color::Cyan),
            );
            if let Some(s) = sort_state
                && s.column == i
            {
                let indicator = match s.direction {
                    SortDirection::Ascending => " ▲",
                    SortDirection::Descending => " ▼",
                };
                let display = format!("{name}{indicator}");
                cell = Cell::from(display).style(
                    Style::default()
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::REVERSED)
                        .fg(Color::Yellow),
                );
            }
            cell
        })
        .collect();

    let header = TableRow::new(header_cells).height(1);

    // Build data rows
    let rows: Vec<TableRow> = data
        .rows
        .iter()
        .map(|row| {
            let cells: Vec<Cell> = row
                .values
                .iter()
                .map(|v| {
                    let display = if v.len() > MAX_CELL_WIDTH {
                        let truncated: String = v.chars().take(MAX_CELL_WIDTH - 1).collect();
                        format!("{truncated}{TRUNCATION_SUFFIX}")
                    } else {
                        v.clone()
                    };
                    Cell::from(display)
                })
                .collect();
            TableRow::new(cells).height(1)
        })
        .collect();

    // Title shows class and dimensions; highlight sort info if active
    let title = if let Some(s) = sort_state {
        format!(
            " {} — {}×{} — sorted by {} {} ",
            data.class,
            data.nrow,
            data.ncol,
            data.col_names.get(s.column).unwrap_or(&"?".into()),
            match s.direction {
                SortDirection::Ascending => "▲",
                SortDirection::Descending => "▼",
            },
        )
    } else {
        format!(" {} — {}×{} ", data.class, data.nrow, data.ncol)
    };

    let table = Table::new(rows, col_widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title.as_str()))
        .row_highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(Color::Yellow),
        )
        .highlight_spacing(HighlightSpacing::Always)
        .column_spacing(2);

    // Layout: table takes most of the space, footer at bottom
    let vert = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]);
    let [table_area, footer_area] = vert.areas(area);

    f.render_stateful_widget(table, table_area, table_state);

    // Footer with keybindings help
    let selected = table_state.selected().unwrap_or(0);
    let row_count = data.rows.len();
    let footer_parts: Vec<Span> = if row_count > 0 {
        vec![
            Span::styled(
                format!(" Row {}/{}  ", selected + 1, row_count),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                format!("Cols {}  ", data.ncol),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(
                " [↑↓/j↓/k↑]Scroll  [s]Sort  [u]Unsort  [Enter]Preview  [q]Exit",
                Style::default().fg(Color::DarkGray),
            ),
        ]
    } else {
        vec![Span::styled(
            " [q]Exit",
            Style::default().fg(Color::DarkGray),
        )]
    };
    let footer = Paragraph::new(Line::from(footer_parts));
    f.render_widget(footer, footer_area);

    // Cell preview popup overlay
    if let Some((row_idx, col_idx)) = show_preview
        && let Some(row) = data.rows.get(row_idx)
        && let Some(val) = row.values.get(col_idx)
    {
        let preview_area = centered_rect(65, 50, area);
        let col_name = data
            .col_names
            .get(col_idx)
            .map(|s| s.as_str())
            .unwrap_or("?");

        // Clear the area behind the popup
        f.render_widget(Clear, preview_area);

        let preview_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" {col_name}[{col_idx}] — row {} ", row_idx + 1))
            .style(Style::default().bg(Color::Black).fg(Color::White));

        let preview_text = Paragraph::new(val.as_str())
            .block(preview_block)
            .wrap(Wrap { trim: false });
        f.render_widget(preview_text, preview_area);
    }
}

/// Create a centered `Rect` with the given percentage width and height of the
/// parent area.
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Length((r.height * (100 - percent_y)) / 200),
        Constraint::Length((r.height * percent_y) / 100),
        Constraint::Length((r.height * (100 - percent_y)) / 200),
    ])
    .split(r);

    Layout::horizontal([
        Constraint::Length((r.width * (100 - percent_x)) / 200),
        Constraint::Length((r.width * percent_x) / 100),
        Constraint::Length((r.width * (100 - percent_x)) / 200),
    ])
    .split(popup_layout[1])[1]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_inspect_output ---

    #[test]
    fn parse_tsv_extracts_header_and_data() {
        let tsv = "data.frame\t32\t11\nmpg\tcyl\tdisp\n21.0\t6\t160\n22.8\t4\t108\n";
        let result = parse_inspect_output(tsv).unwrap();
        assert_eq!(result.class, "data.frame");
        assert_eq!(result.nrow, 32);
        assert_eq!(result.ncol, 11);
        assert_eq!(result.col_names, vec!["mpg", "cyl", "disp"]);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].values, vec!["21.0", "6", "160"]);
        assert_eq!(result.rows[1].values, vec!["22.8", "4", "108"]);
    }

    #[test]
    fn parse_tsv_rejects_too_few_lines() {
        assert!(parse_inspect_output("only_one_line").is_err());
        assert!(parse_inspect_output("").is_err());
    }

    #[test]
    fn parse_tsv_rejects_bad_metadata() {
        let tsv = "data.frame\tnot_a_number\t11\ncol1\tcol2\n";
        assert!(parse_inspect_output(tsv).is_err());
    }

    #[test]
    fn parse_tsv_handles_zero_rows() {
        let tsv = "data.frame\t0\t3\ncol1\tcol2\tcol3\n";
        let result = parse_inspect_output(tsv).unwrap();
        assert_eq!(result.rows.len(), 0);
        assert_eq!(result.nrow, 0);
    }

    #[test]
    fn parse_tsv_handles_single_row() {
        let tsv = "matrix\t1\t2\nx\ty\n1.0\tabc\n";
        let result = parse_inspect_output(tsv).unwrap();
        assert_eq!(result.class, "matrix");
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].values, vec!["1.0", "abc"]);
    }

    #[test]
    fn parse_tsv_with_na_values() {
        let tsv = "data.frame\t2\t2\nname\tval\nAlice\tNA\nBob\t3.14\n";
        let result = parse_inspect_output(tsv).unwrap();
        assert_eq!(result.rows[0].values, vec!["Alice", "NA"]);
        assert_eq!(result.rows[1].values, vec!["Bob", "3.14"]);
    }

    // --- sort_by ---

    #[test]
    fn sort_str_column_ascending() {
        let mut data = InspectData {
            class: "data.frame".into(),
            nrow: 3,
            ncol: 2,
            col_names: vec!["name".into(), "val".into()],
            rows: vec![
                Row {
                    values: vec!["b".into(), "3".into()],
                },
                Row {
                    values: vec!["a".into(), "2".into()],
                },
                Row {
                    values: vec!["c".into(), "1".into()],
                },
            ],
        };
        data.sort_by(0, SortDirection::Ascending);
        assert_eq!(data.rows[0].values[0], "a");
        assert_eq!(data.rows[1].values[0], "b");
        assert_eq!(data.rows[2].values[0], "c");
    }

    #[test]
    fn sort_str_column_descending() {
        let mut data = InspectData {
            class: "data.frame".into(),
            nrow: 3,
            ncol: 2,
            col_names: vec!["name".into(), "val".into()],
            rows: vec![
                Row {
                    values: vec!["b".into(), "3".into()],
                },
                Row {
                    values: vec!["a".into(), "2".into()],
                },
                Row {
                    values: vec!["c".into(), "1".into()],
                },
            ],
        };
        data.sort_by(0, SortDirection::Descending);
        assert_eq!(data.rows[0].values[0], "c");
        assert_eq!(data.rows[2].values[0], "a");
    }

    #[test]
    fn sort_numeric_column_as_numbers() {
        let mut data = InspectData {
            class: "data.frame".into(),
            nrow: 3,
            ncol: 2,
            col_names: vec!["name".into(), "val".into()],
            rows: vec![
                Row {
                    values: vec!["x".into(), "30".into()],
                },
                Row {
                    values: vec!["y".into(), "2".into()],
                },
                Row {
                    values: vec!["z".into(), "100".into()],
                },
            ],
        };
        data.sort_by(1, SortDirection::Ascending);
        assert_eq!(data.rows[0].values[1], "2");
        assert_eq!(data.rows[1].values[1], "30");
        assert_eq!(data.rows[2].values[1], "100");
    }

    #[test]
    fn sort_numeric_mixed_with_string_falls_back_to_string() {
        let mut data = InspectData {
            class: "data.frame".into(),
            nrow: 2,
            ncol: 1,
            col_names: vec!["val".into()],
            rows: vec![
                Row {
                    values: vec!["NA".into()],
                },
                Row {
                    values: vec!["2".into()],
                },
            ],
        };
        // "NA" can't parse as f64, so string compare: "2" < "NA"
        data.sort_by(0, SortDirection::Ascending);
        assert_eq!(data.rows[0].values[0], "2");
        assert_eq!(data.rows[1].values[0], "NA");
    }
}
