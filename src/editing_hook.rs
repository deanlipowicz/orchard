use crate::{
    lexer::cursor_in_string,
    r_runtime::ConsoleSettings,
};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use reedline::{EditCommand, ReedlineEvent};
use std::sync::atomic::{AtomicBool, Ordering};

/// Tracks whether Ctrl+X was pressed without a completing second chord.
/// Used for the Ctrl+X Ctrl+E editor chord.
static CTRL_X_PRESSED: AtomicBool = AtomicBool::new(false);

/// Set to `true` by `read_shell_prompt` when the editor is in persistent
/// shell mode.  When `true`, a Backspace at cursor position 0 submits an
/// empty line, which causes the shell loop to exit back to R mode.
static SHELL_MODE: AtomicBool = AtomicBool::new(false);

/// Mark the editor as being in persistent shell mode so that the hook can
/// handle Backspace-on-empty-buffer as a shell-exit signal.
pub fn set_shell_mode(enabled: bool) {
    SHELL_MODE.store(enabled, Ordering::SeqCst);
}

/// Pre-edit hook for reedline.
///
/// Intercepts specific key events, inspects the current buffer and cursor
/// position, and returns an appropriate `ReedlineEvent` to override the
/// normal edit-mode dispatch.  Returns `None` for unhandled keys, allowing
/// the default emacs/vi handling to proceed.
///
/// Multi-chord sequences:
///
/// - **Ctrl+X Ctrl+E** → `ReedlineEvent::OpenEditor` (same as Ctrl+O)
pub fn handle(
    event: &Event,
    buffer: &str,
    cursor: usize,
    settings: &ConsoleSettings,
) -> Option<ReedlineEvent> {

    // --- Multi-chord sequences (check before clearing the prefix flag) ---
    if let Event::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        ..
    }) = &event
        && *modifiers == KeyModifiers::CONTROL {
            match code {
                KeyCode::Char('x') | KeyCode::Char('X') => {
                    CTRL_X_PRESSED.store(true, Ordering::SeqCst);
                    return None; // pass through (Ctrl+X is a no-op in emacs)
                }
                KeyCode::Char('e') | KeyCode::Char('E')
                    if CTRL_X_PRESSED.swap(false, Ordering::SeqCst) => {
                        return Some(ReedlineEvent::OpenEditor);
                    }
                _ => {}
            }
        }

    // Clear the prefix flag for any other keypress
    CTRL_X_PRESSED.store(false, Ordering::SeqCst);

    // --- Single-key dispatch ---
    match &event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            ..
        }) => match code {
            KeyCode::Char('(') | KeyCode::Char('[') | KeyCode::Char('{') => {
                bracket_or_raw_pair(buffer, cursor, *code)
            }
            KeyCode::Char('"') => auto_pair(buffer, cursor, '"', '"'),
            KeyCode::Char('\'') => auto_pair(buffer, cursor, '\'', '\''),
            KeyCode::Char(')') | KeyCode::Char(']') | KeyCode::Char('}') => {
                closing_delimiter(buffer, cursor, *code, settings.tab_size)
            }
            KeyCode::Backspace => {
                if cursor == 0 && SHELL_MODE.load(Ordering::SeqCst) {
                    // In persistent shell mode, Backspace on an empty buffer
                    // submits the line, causing the shell loop to exit to R.
                    return Some(ReedlineEvent::Enter);
                }
                smart_backspace(buffer, cursor, settings.tab_size)
            }
            KeyCode::Enter => enter_indent(buffer, cursor, settings),
            KeyCode::Tab => smart_tab(buffer, cursor, settings.tab_size),
            _ => None,
        },
        Event::Key(KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            ..
        }) => {
            // Ctrl-D at end of empty buffer → EOF (pass through to default handling)
            None
        }
        Event::Paste(body) => paste_handler(buffer, cursor, body),
        _ => None,
    }
}

/// Handle bracketed paste: normalize line endings and strip a trailing
/// newline when pasting at end of buffer (so the buffer doesn't end up
/// with a blank line).  The full Python radian behavior — stripping and
/// auto-submitting when code is parse-complete — requires a parser
/// closure not available in the hook; that's deferred.
fn paste_handler(buffer: &str, cursor: usize, body: &str) -> Option<ReedlineEvent> {
    let normalized = body.replace("\r\n", "\n").replace('\r', "\n");
    if cursor == buffer.len() && normalized.ends_with('\n') {
        let stripped = &normalized[..normalized.len() - 1];
        Some(ReedlineEvent::Edit(vec![EditCommand::InsertString(
            stripped.to_string(),
        )]))
    } else {
        // Fall through to reedline's built-in paste handling (which also
        // does CRLF/CR → LF normalization).
        None
    }
}

// ---------------------------------------------------------------------------
// Key-specific logic
// ---------------------------------------------------------------------------

/// Auto-pair insertion: if the cursor is inside a string or the following
/// character doesn't accept a pair, let the character through as-is.
/// Otherwise insert the pair and move the cursor between them.
fn auto_pair(buffer: &str, cursor: usize, open: char, close: char) -> Option<ReedlineEvent> {
    if cursor_in_string(buffer, cursor) || !following_text_accepts_pair(buffer, cursor) {
        return None;
    }
    Some(ReedlineEvent::Edit(vec![
        EditCommand::InsertChar(open),
        EditCommand::InsertChar(close),
        EditCommand::MoveLeft { select: false },
    ]))
}

/// Closing delimiter handling:
/// 1. If the next character matches, skip over it (type_closing).
/// 2. If on a blank indented line, dedent to the next tab stop then insert.
/// 3. Otherwise, fall through to normal character insertion.
fn closing_delimiter(
    buffer: &str,
    cursor: usize,
    typed: KeyCode,
    tab_size: i32,
) -> Option<ReedlineEvent> {
    let ch = match typed {
        KeyCode::Char(c) => c,
        _ => return None,
    };
    // Case 1: next char matches → skip
    if buffer[cursor..].starts_with(ch) {
        return Some(ReedlineEvent::Edit(vec![EditCommand::MoveRight {
            select: false,
        }]));
    }
    // Case 2: blank indented line → dedent then insert
    let before = &buffer[..cursor];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    if before[line_start..].chars().all(|c| c == ' ') {
        let indent = before[line_start..].len();
        let remove = indent.min((tab_size.max(1) as usize).max(indent));
        if remove > 0 {
            let mut cmds = Vec::new();
            for _ in 0..remove {
                cmds.push(EditCommand::Backspace);
            }
            cmds.push(EditCommand::InsertChar(ch));
            return Some(ReedlineEvent::Edit(cmds));
        }
    }
    None
}

/// Smart backspace:
/// 1. If the cursor is between an empty pair `()`, delete both chars.
/// 2. If the cursor is in leading whitespace, delete tab_size worth.
/// 3. Otherwise fall through to normal backspace.
fn smart_backspace(buffer: &str, cursor: usize, tab_size: i32) -> Option<ReedlineEvent> {
    if cursor == 0 {
        return None;
    }
    let before = &buffer[..cursor];
    let after = &buffer[cursor..];
    // Empty pair deletion
    if matches!(
        (before.chars().last(), after.chars().next()),
        (Some('('), Some(')'))
            | (Some('['), Some(']'))
            | (Some('{'), Some('}'))
            | (Some('"'), Some('"'))
            | (Some('\''), Some('\''))
    ) {
        return Some(ReedlineEvent::Edit(vec![
            EditCommand::Backspace,
            EditCommand::Delete,
        ]));
    }
    // Leading whitespace deletion
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    if before[line_start..].chars().all(|c| c == ' ') {
        let remove = before[line_start..]
            .len()
            .min((tab_size.max(1) as usize).max(1));
        if remove > 0 {
            let mut cmds = Vec::new();
            for _ in 0..remove {
                cmds.push(EditCommand::Backspace);
            }
            return Some(ReedlineEvent::Edit(cmds));
        }
    }
    None
}

/// Enter with auto-indentation: if `auto_indentation` is enabled, insert a
/// newline followed by spaces matching the current line's indent level (plus
/// one extra `tab_size` if the line before the cursor ends with `{`).
/// When disabled (or in browse/shell mode), fall through to normal Enter.
fn enter_indent(buffer: &str, cursor: usize, settings: &ConsoleSettings) -> Option<ReedlineEvent> {
    if !settings.auto_indentation {
        return None;
    }
    let before = &buffer[..cursor];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let current_indent = before[line_start..].chars().take_while(|c| *c == ' ').count();
    let extra = if before.trim_end().ends_with('{') {
        settings.tab_size.max(1) as usize
    } else {
        0
    };
    let indent = current_indent + extra;
    let spaces = " ".repeat(indent);
    Some(ReedlineEvent::Edit(vec![
        EditCommand::InsertNewline,
        EditCommand::InsertString(spaces),
    ]))
}

/// Tab: if the cursor is in leading whitespace, insert spaces up to the next
/// tab stop.  Otherwise fall through to a literal tab character.
fn smart_tab(buffer: &str, cursor: usize, tab_size: i32) -> Option<ReedlineEvent> {
    let before = &buffer[..cursor];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    if before[line_start..].chars().all(|c| c == ' ') {
        let spaces = " ".repeat((tab_size.max(1) as usize).max(1));
        return Some(ReedlineEvent::Edit(vec![EditCommand::InsertString(spaces)]));
    }
    None
}

/// Returns `true` if the character after the cursor will accept a pair being
/// inserted before it (whitespace, closing bracket, comma, semicolon, or EOF).
fn following_text_accepts_pair(text: &str, cursor: usize) -> bool {
    text[cursor..]
        .chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || matches!(c, ')' | ']' | '}' | ',' | ';'))
}

/// Detect if the cursor is positioned right after an R raw string prefix
/// (`r"`, `r'`, `R"`, `R'`) with optional dash delimiters.
///
/// Returns `Some((quote_char, dashes))` when the text before the cursor
/// matches `[rR]["'](-*)` — that is, the raw string prefix immediately
/// followed by zero or more `-` characters.
fn raw_string_context(buffer: &str, cursor: usize) -> Option<(char, String)> {
    let before = &buffer[..cursor];
    let bytes = before.as_bytes();
    let len = before.len();

    if len < 2 {
        return None;
    }

    // Count trailing dashes (or lack thereof)
    let mut dash_start = len;
    while dash_start > 0 && bytes[dash_start - 1] == b'-' {
        dash_start -= 1;
    }

    // We need at least r" or r' (or R", R') at positions dash_start-2, dash_start-1
    if dash_start >= 2 {
        let r = bytes[dash_start - 2];
        let quote = bytes[dash_start - 1];
        if (r == b'r' || r == b'R') && (quote == b'"' || quote == b'\'') {
            let dashes: String = before[dash_start..len].to_string();
            return Some((quote as char, dashes));
        }
    }

    None
}

/// Auto-pair a bracket in a raw string context, inserting the full closing
/// sequence: `close` + `dashes` + `quote_char`, then moving the cursor back
/// between the opening and closing brackets.
fn auto_pair_raw(
    buffer: &str,
    cursor: usize,
    open: char,
    close: char,
    quote_char: char,
    dashes: &str,
) -> Option<ReedlineEvent> {
    if !following_text_accepts_pair(buffer, cursor) {
        return None;
    }
    let mut cmds = vec![
        EditCommand::InsertChar(open),
        EditCommand::InsertChar(close),
    ];
    for ch in dashes.chars() {
        cmds.push(EditCommand::InsertChar(ch));
    }
    cmds.push(EditCommand::InsertChar(quote_char));
    // Move cursor back between open and close
    let move_by = 1 + dashes.len() + 1; // close + dashes + quote_char
    for _ in 0..move_by {
        cmds.push(EditCommand::MoveLeft { select: false });
    }
    Some(ReedlineEvent::Edit(cmds))
}

/// Helper that dispatches `(`, `[`, `{` to raw-string auto-pair if the cursor
/// is right after a raw string prefix, or to regular auto-pair otherwise.
fn bracket_or_raw_pair(buffer: &str, cursor: usize, code: KeyCode) -> Option<ReedlineEvent> {
    let (open, close) = match code {
        KeyCode::Char('(') => ('(', ')'),
        KeyCode::Char('[') => ('[', ']'),
        KeyCode::Char('{') => ('{', '}'),
        _ => return None,
    };
    if let Some((quote_char, dashes)) = raw_string_context(buffer, cursor) {
        auto_pair_raw(buffer, cursor, open, close, quote_char, &dashes)
    } else {
        auto_pair(buffer, cursor, open, close)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reedline::ReedlineEvent;

    fn settings_with_auto_match() -> ConsoleSettings {
        let mut s = ConsoleSettings::default();
        s.auto_match = true;
        s.auto_indentation = true;
        s.tab_size = 4;
        s
    }

    // --- auto-pair ---

    #[test]
    fn auto_pair_inserts_pair_when_context_allows() {
        let result = auto_pair("foo", 3, '(', ')');
        assert!(result.is_some());
    }

    #[test]
    fn auto_pair_returns_none_inside_string() {
        // cursor at position 6 is inside the string
        let result = auto_pair("\"hello ", 6, '(', ')');
        assert!(result.is_none());
    }

    // --- closing delimiter ---

    #[test]
    fn closing_delim_skips_when_next_char_matches() {
        let result = closing_delimiter("()", 1, KeyCode::Char(')'), 4);
        assert!(matches!(
            result,
            Some(ReedlineEvent::Edit(ref cmds)) if cmds.len() == 1
        ));
    }

    #[test]
    fn closing_delim_dedents_blank_line() {
        let result = closing_delimiter("    ", 4, KeyCode::Char('}'), 4);
        assert!(result.is_some());
    }

    #[test]
    fn closing_delim_returns_none_on_non_blank_line() {
        let result = closing_delimiter("x  ", 3, KeyCode::Char(')'), 4);
        assert!(result.is_none());
    }

    // --- smart backspace ---

    #[test]
    fn backspace_deletes_empty_pair() {
        let result = smart_backspace("()", 1, 4);
        assert!(result.is_some());
    }

    #[test]
    fn backspace_in_leading_indent_deletes_tab_size() {
        let result = smart_backspace("    x", 4, 4);
        assert!(result.is_some());
    }

    #[test]
    fn backspace_at_beginning_returns_none() {
        let result = smart_backspace("hello", 0, 4);
        assert!(result.is_none());
    }

    // --- enter indent ---

    #[test]
    fn enter_indents_after_open_brace() {
        let settings = settings_with_auto_match();
        let result = enter_indent("if (x) {", 8, &settings);
        assert!(result.is_some());
    }

    #[test]
    fn enter_indents_at_current_level_without_brace() {
        let settings = settings_with_auto_match();
        let result = enter_indent("  x", 3, &settings);
        assert!(result.is_some());
        // Should insert newline + 2 spaces (current indent = 2, extra = 0)
        if let Some(ReedlineEvent::Edit(cmds)) = result {
            assert_eq!(cmds.len(), 2, "expected InsertNewline + InsertString");
        }
    }

    // --- smart tab ---

    #[test]
    fn tab_inserts_spaces_in_leading_indent() {
        let result = smart_tab("  x", 2, 4);
        assert!(result.is_some());
    }

    #[test]
    fn tab_returns_none_in_non_leading_indent() {
        let result = smart_tab("x", 1, 4);
        assert!(result.is_none());
    }

    // --- following_text_accepts_pair ---

    #[test]
    fn following_text_accepts_pair_at_eof() {
        assert!(following_text_accepts_pair("foo", 3));
    }

    #[test]
    fn following_text_rejects_pair_before_alnum() {
        assert!(!following_text_accepts_pair("f", 0));
    }

    #[test]
    fn following_text_accepts_pair_before_whitespace() {
        assert!(following_text_accepts_pair("( ", 1));
    }

    // --- raw string context ---

    #[test]
    fn raw_string_context_detects_r_double_quote() {
        assert_eq!(raw_string_context("r\"", 2), Some(('"', "".to_string())));
    }

    #[test]
    fn raw_string_context_detects_r_single_quote() {
        assert_eq!(raw_string_context("r'", 2), Some(('\'', "".to_string())));
    }

    #[test]
    fn raw_string_context_detects_uppercase_r() {
        assert_eq!(raw_string_context("R\"", 2), Some(('"', "".to_string())));
    }

    #[test]
    fn raw_string_context_with_dashes() {
        assert_eq!(
            raw_string_context("r\"---", 5),
            Some(('"', "---".to_string()))
        );
    }

    #[test]
    fn raw_string_context_returns_none_for_regular_string() {
        assert_eq!(raw_string_context("\"hello ", 7), None);
    }

    #[test]
    fn raw_string_context_returns_none_for_function_call() {
        assert_eq!(raw_string_context("foo", 3), None);
    }

    #[test]
    fn raw_string_context_returns_none_without_r_prefix() {
        assert_eq!(raw_string_context("\"", 1), None);
    }

    // --- auto_pair_raw ---

    #[test]
    fn auto_pair_raw_creates_simple_raw_string() {
        // r"( followed by auto-completion of )" with cursor repositioning
        let result = auto_pair_raw("r\"", 2, '(', ')', '"', "");
        assert!(result.is_some());
        if let Some(ReedlineEvent::Edit(cmds)) = result {
            // Commands: InsertChar('('), InsertChar(')'), InsertChar('"'), MoveLeft x2 = 5
            assert_eq!(cmds.len(), 5, "expected 5 commands, got {cmds:?}");
        }
    }

    #[test]
    fn auto_pair_raw_with_dashes() {
        // r"---( followed by )---" with cursor repositioning
        let result = auto_pair_raw("r\"---", 5, '(', ')', '"', "---");
        assert!(result.is_some());
        if let Some(ReedlineEvent::Edit(cmds)) = result {
            // InsertChar('('), InsertChar(')'), InsertChar('-') x3, InsertChar('"'), MoveLeft x5 = 11
            assert_eq!(cmds.len(), 1 + 1 + 3 + 1 + 5, "expected 11 commands, got {cmds:?}");
        }
    }

    #[test]
    fn auto_pair_raw_respects_following_text_guard() {
        // Buffer "r\"x" with cursor at 2 means `x` follows the cursor — should reject
        let result = auto_pair_raw("r\"x", 2, '(', ')', '"', "");
        assert!(result.is_none());
    }

    // --- bracket_or_raw_pair ---

    #[test]
    fn bracket_or_raw_pair_uses_raw_context_when_applicable() {
        let result = bracket_or_raw_pair("r\"", 2, KeyCode::Char('('));
        assert!(
            result.is_some(),
            "expected raw string auto-pair for r\" + ("
        );
    }

    #[test]
    fn bracket_or_raw_pair_falls_back_to_normal_when_not_raw() {
        let result = bracket_or_raw_pair("foo", 3, KeyCode::Char('('));
        assert!(
            result.is_some(),
            "expected normal auto-pair for regular context"
        );
    }

    #[test]
    fn bracket_or_raw_pair_returns_none_inside_regular_string() {
        let result = bracket_or_raw_pair("\"hello ", 7, KeyCode::Char('('));
        assert!(result.is_none(), "no auto-pair inside regular string");
    }

    // --- multi-chord sequences ---

    #[test]
    fn ctrl_x_does_not_interfere_with_normal_keys() {
        CTRL_X_PRESSED.store(false, Ordering::SeqCst);
        // Ctrl+X alone should return None (pass through)
        let event = Event::Key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::CONTROL,
        ));
        let result = handle(&event, "", 0, &ConsoleSettings::default());
        assert!(result.is_none(), "Ctrl+X alone should pass through");
        // Flag should be set after Ctrl+X
        assert!(CTRL_X_PRESSED.load(Ordering::SeqCst));
        // Reset for subsequent tests
        CTRL_X_PRESSED.store(false, Ordering::SeqCst);
    }

    #[test]
    fn ctrl_x_ctrl_e_opens_editor() {
        CTRL_X_PRESSED.store(false, Ordering::SeqCst);
        // Simulate Ctrl+X press
        let ctrl_x = Event::Key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::CONTROL,
        ));
        let _ = handle(&ctrl_x, "", 0, &ConsoleSettings::default());
        assert!(CTRL_X_PRESSED.load(Ordering::SeqCst));

        // Simulate Ctrl+E press right after
        let ctrl_e = Event::Key(KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL,
        ));
        let result = handle(&ctrl_e, "", 0, &ConsoleSettings::default());
        assert!(matches!(result, Some(ReedlineEvent::OpenEditor)));
        // Flag should be consumed
        assert!(!CTRL_X_PRESSED.load(Ordering::SeqCst));
    }

    #[test]
    fn ctrl_x_then_other_key_clears_flag() {
        CTRL_X_PRESSED.store(false, Ordering::SeqCst);
        // Ctrl+X
        let ctrl_x = Event::Key(KeyEvent::new(
            KeyCode::Char('x'),
            KeyModifiers::CONTROL,
        ));
        let _ = handle(&ctrl_x, "", 0, &ConsoleSettings::default());
        assert!(CTRL_X_PRESSED.load(Ordering::SeqCst));

        // Some other keypress (Ctrl+D)
        let ctrl_d = Event::Key(KeyEvent::new(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL,
        ));
        let _ = handle(&ctrl_d, "", 0, &ConsoleSettings::default());
        // Flag should be cleared
        assert!(!CTRL_X_PRESSED.load(Ordering::SeqCst));
    }

    // --- paste handler ---

    #[test]
    fn paste_strips_trailing_newline_at_end_of_buffer() {
        let result = paste_handler("abc", 3, "def\n");
        assert!(result.is_some());
        if let Some(ReedlineEvent::Edit(cmds)) = result {
            assert_eq!(cmds.len(), 1);
            if let EditCommand::InsertString(s) = &cmds[0] {
                assert_eq!(s, "def", "trailing newline should be stripped");
            } else {
                panic!("expected InsertString");
            }
        }
    }

    #[test]
    fn paste_passes_through_when_cursor_not_at_end() {
        let result = paste_handler("ab", 1, "X\n");
        assert!(result.is_none(), "should pass through to reedline");
    }

    #[test]
    fn paste_passes_through_when_no_trailing_newline() {
        let result = paste_handler("abc", 3, "def");
        assert!(result.is_none(), "should pass through to reedline");
    }

    #[test]
    fn paste_normalizes_crlf() {
        let result = paste_handler("a", 1, "b\r\nc\n");
        assert!(result.is_some());
        if let Some(ReedlineEvent::Edit(cmds)) = result {
            if let EditCommand::InsertString(s) = &cmds[0] {
                assert_eq!(s, "b\nc", "CRLF should be normalized, trailing newline stripped");
            } else {
                panic!("expected InsertString");
            }
        }
    }

    #[test]
    fn ctrl_e_without_prefix_does_not_open_editor() {
        CTRL_X_PRESSED.store(false, Ordering::SeqCst);
        let ctrl_e = Event::Key(KeyEvent::new(
            KeyCode::Char('e'),
            KeyModifiers::CONTROL,
        ));
        let result = handle(&ctrl_e, "", 0, &ConsoleSettings::default());
        // Without Ctrl+X prefix, Ctrl+E should not open the editor
        assert!(
            !matches!(result, Some(ReedlineEvent::OpenEditor)),
            "Ctrl+E without prefix should not open editor"
        );
    }

    // --- shell mode backspace ---

    #[test]
    fn shell_mode_backspace_submits_empty_buffer() {
        SHELL_MODE.store(false, Ordering::SeqCst);
        set_shell_mode(true);

        let event = Event::Key(KeyEvent::new(
            KeyCode::Backspace,
            KeyModifiers::NONE,
        ));
        let result = handle(&event, "", 0, &ConsoleSettings::default());
        assert!(
            matches!(result, Some(ReedlineEvent::Enter)),
            "Backspace on empty buffer in shell mode should submit"
        );

        set_shell_mode(false);
    }

    #[test]
    fn shell_mode_backspace_does_not_submit_when_buffer_not_empty() {
        SHELL_MODE.store(false, Ordering::SeqCst);
        set_shell_mode(true);

        let event = Event::Key(KeyEvent::new(
            KeyCode::Backspace,
            KeyModifiers::NONE,
        ));
        // Cursor at position 1, buffer has "x" → should not submit
        let result = handle(&event, "x", 1, &ConsoleSettings::default());
        assert!(
            !matches!(result, Some(ReedlineEvent::Enter)),
            "Backspace with non-empty buffer should not submit in shell mode"
        );

        set_shell_mode(false);
    }

    #[test]
    fn normal_mode_backspace_at_start_does_not_submit() {
        SHELL_MODE.store(false, Ordering::SeqCst);
        // Ensure shell mode is NOT set
        set_shell_mode(false);

        let event = Event::Key(KeyEvent::new(
            KeyCode::Backspace,
            KeyModifiers::NONE,
        ));
        let result = handle(&event, "", 0, &ConsoleSettings::default());
        assert!(
            !matches!(result, Some(ReedlineEvent::Enter)),
            "Backspace in normal mode should not submit"
        );
    }
}

