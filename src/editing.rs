use crate::lexer::cursor_in_string;
use std::sync::{Mutex, OnceLock};

static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PasteAction {
    Insert(String),
    Submit(String),
}

pub fn insert_pair(text: &str, cursor: usize, typed: char) -> (String, usize) {
    let pair = match typed {
        '(' => "()",
        '[' => "[]",
        '{' => "{}",
        '"' => "\"\"",
        '\'' => "''",
        _ => return insert_char(text, cursor, typed),
    };
    if cursor_in_string(text, cursor) || !following_text_accepts_pair(text, cursor) {
        return insert_char(text, cursor, typed);
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push_str(&text[..cursor]);
    out.push_str(pair);
    out.push_str(&text[cursor..]);
    (out, cursor + 1)
}

pub fn insert_raw_string_pair(
    text: &str,
    cursor: usize,
    opener: char,
    dashes: &str,
) -> (String, usize) {
    let closer = match opener {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => return (text.to_string(), cursor),
    };
    let pair = format!("r\"{dashes}{opener}{closer}{dashes}\"");
    let mut out = text[..cursor].to_string();
    out.push_str(&pair);
    out.push_str(&text[cursor..]);
    (out, cursor + 3 + dashes.len())
}

pub fn type_closing(text: &str, cursor: usize, typed: char) -> (String, usize) {
    if text[cursor..].starts_with(typed) {
        (text.to_string(), cursor + typed.len_utf8())
    } else {
        insert_char(text, cursor, typed)
    }
}

pub fn type_closing_on_blank_indent(
    text: &str,
    cursor: usize,
    typed: char,
    tab_size: usize,
) -> (String, usize) {
    if !matches!(typed, ')' | ']' | '}') {
        return insert_char(text, cursor, typed);
    }
    let before = &text[..cursor];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    if before[line_start..].chars().all(|c| c == ' ') {
        let remove = before[line_start..].len().min(tab_size.max(1));
        let mut out = text[..cursor - remove].to_string();
        out.push(typed);
        out.push_str(&text[cursor..]);
        return (out, cursor + typed.len_utf8() - remove);
    }
    type_closing(text, cursor, typed)
}

pub fn backspace(text: &str, cursor: usize, tab_size: usize) -> (String, usize) {
    if cursor == 0 {
        return (text.to_string(), cursor);
    }
    let before = &text[..cursor];
    let after = &text[cursor..];
    if matches!(
        (before.chars().last(), after.chars().next()),
        (Some('('), Some(')'))
            | (Some('['), Some(']'))
            | (Some('{'), Some('}'))
            | (Some('"'), Some('"'))
            | (Some('\''), Some('\''))
    ) {
        let mut out = before[..before.len() - 1].to_string();
        out.push_str(&after[1..]);
        return (out, cursor - 1);
    }
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    if before[line_start..].chars().all(|c| c == ' ') {
        let remove = before[line_start..].len().min(tab_size.max(1));
        let mut out = text[..cursor - remove].to_string();
        out.push_str(after);
        return (out, cursor - remove);
    }
    let prev = before.chars().last().unwrap();
    let start = cursor - prev.len_utf8();
    let mut out = text[..start].to_string();
    out.push_str(after);
    (out, start)
}

pub fn insert_tab(text: &str, cursor: usize, tab_size: usize) -> (String, usize) {
    let before = &text[..cursor];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    if before[line_start..].chars().all(|c| c == ' ') {
        let spaces = " ".repeat(tab_size.max(1));
        let mut out = before.to_string();
        out.push_str(&spaces);
        out.push_str(&text[cursor..]);
        return (out, cursor + spaces.len());
    }
    insert_char(text, cursor, '\t')
}

pub fn bracketed_paste(
    text: &str,
    cursor: usize,
    pasted: &str,
    complete: impl Fn(&str) -> bool,
) -> PasteAction {
    let normalized = pasted.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = text[..cursor].to_string();
    out.push_str(&normalized);
    out.push_str(&text[cursor..]);
    if cursor == text.len() && normalized.ends_with('\n') {
        let submit = out.trim_end_matches('\n').to_string();
        if complete(&submit) {
            return PasteAction::Submit(submit);
        }
    }
    PasteAction::Insert(out)
}

pub fn indent_after_enter(text: &str, cursor: usize, tab_size: usize) -> (String, usize) {
    let before = &text[..cursor];
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let mut indent = before[line_start..]
        .chars()
        .take_while(|c| *c == ' ')
        .count();
    if before.trim_end().ends_with('{') {
        indent += tab_size;
    }
    let insert = format!("\n{}", " ".repeat(indent));
    let mut out = before.to_string();
    out.push_str(&insert);
    out.push_str(&text[cursor..]);
    (out, cursor + insert.len())
}

pub fn select_editor(r_option: Option<&str>) -> String {
    r_option
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .or_else(|| std::env::var("VISUAL").ok().filter(|s| !s.is_empty()))
        .or_else(|| std::env::var("EDITOR").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "vi".to_string())
}

fn following_text_accepts_pair(text: &str, cursor: usize) -> bool {
    text[cursor..]
        .chars()
        .next()
        .is_none_or(|c| c.is_whitespace() || matches!(c, ')' | ']' | '}' | ',' | ';'))
}

fn insert_char(text: &str, cursor: usize, ch: char) -> (String, usize) {
    let mut out = text[..cursor].to_string();
    out.push(ch);
    out.push_str(&text[cursor..]);
    (out, cursor + ch.len_utf8())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- auto-pair insertion ---

    #[test]
    fn inserts_and_deletes_empty_pair() {
        let (text, cursor) = insert_pair("", 0, '(');
        assert_eq!((text.as_str(), cursor), ("()", 1));
        assert_eq!(backspace(&text, cursor, 4), ("".to_string(), 0));
    }

    #[test]
    fn inserts_pairs_for_all_bracket_types() {
        for (ch, expected) in [
            ('(', "()"),
            ('[', "[]"),
            ('{', "{}"),
            ('"', "\"\""),
            ('\'', "''"),
        ] {
            let (text, cursor) = insert_pair("", 0, ch);
            assert_eq!(
                (text.as_str(), cursor),
                (expected, 1),
                "mismatch for {ch:?}"
            );
        }
    }

    #[test]
    fn does_not_insert_pair_inside_string() {
        // cursor_in_string should detect the string context
        let (text, cursor) = insert_pair("\"hello ", 7, '(');
        // Inside a string — should insert literal, not pair
        assert_eq!(text, "\"hello (");
        assert_eq!(cursor, 8);
    }

    #[test]
    fn non_pair_character_inserts_normally() {
        let (text, cursor) = insert_pair("ab", 1, 'x');
        assert_eq!(text, "axb");
        assert_eq!(cursor, 2);
    }

    // --- raw string pairs ---

    #[test]
    fn raw_string_pair_places_cursor_inside() {
        assert_eq!(
            insert_raw_string_pair("", 0, '(', "---"),
            ("r\"---()---\"".into(), 6)
        );
    }

    #[test]
    fn raw_string_pair_with_different_delimiters() {
        assert_eq!(
            insert_raw_string_pair("", 0, '[', ""),
            ("r\"[]\"".into(), 3)
        );
        assert_eq!(
            insert_raw_string_pair("", 0, '{', "-"),
            ("r\"-{}-\"".into(), 4)
        );
    }

    // --- closing delimiter behavior ---

    #[test]
    fn skips_closing_delimiter() {
        assert_eq!(type_closing("()", 1, ')'), ("()".to_string(), 2));
    }

    #[test]
    fn inserts_closing_delimiter_when_no_match() {
        assert_eq!(type_closing("(]", 1, ')'), ("()]".to_string(), 2));
    }

    // --- closing bracket on blank indented line ---

    #[test]
    fn closing_bracket_dedents_blank_line() {
        assert_eq!(
            type_closing_on_blank_indent("    ", 4, '}', 4),
            ("}".into(), 1)
        );
    }

    #[test]
    fn closing_bracket_on_non_blank_line_acts_normally() {
        // Line has content — should not dedent, should use type_closing behavior
        assert_eq!(
            type_closing_on_blank_indent("x  ", 3, ')', 4),
            ("x  )".into(), 4)
        );
    }

    #[test]
    fn non_bracket_character_passes_through() {
        assert_eq!(
            type_closing_on_blank_indent("  ", 2, 'x', 4),
            ("  x".into(), 3)
        );
    }

    // --- backspace ---

    #[test]
    fn backspace_deletes_empty_pair() {
        assert_eq!(backspace("()", 1, 4), ("".into(), 0));
        assert_eq!(backspace("[]", 1, 4), ("".into(), 0));
        assert_eq!(backspace("\"\"", 1, 4), ("".into(), 0));
    }

    #[test]
    fn backspace_in_leading_indent_deletes_tab_size_spaces() {
        assert_eq!(backspace("    x", 4, 4), ("x".into(), 0));
    }

    #[test]
    fn backspace_at_beginning_does_nothing() {
        assert_eq!(backspace("hello", 0, 4), ("hello".into(), 0));
    }

    #[test]
    fn backspace_removes_previous_character_normally() {
        assert_eq!(backspace("ab", 2, 4), ("a".into(), 1));
    }

    // --- tab ---

    #[test]
    fn tab_in_leading_indent_inserts_spaces() {
        assert_eq!(insert_tab("  x", 2, 4), ("      x".into(), 6));
    }

    #[test]
    fn tab_in_non_leading_indent_inserts_literal_tab() {
        assert_eq!(insert_tab("x", 1, 4), ("x\t".into(), 2));
    }

    // --- enter indentation ---

    #[test]
    fn enter_indents_after_open_brace() {
        assert_eq!(
            indent_after_enter("if (x) {", 8, 4),
            ("if (x) {\n    ".into(), 13)
        );
    }

    #[test]
    fn enter_without_open_brace_preserves_existing_indent() {
        assert_eq!(indent_after_enter("  x", 3, 4), ("  x\n  ".into(), 6));
    }

    // --- bracketed paste ---

    #[test]
    fn paste_submits_complete_trailing_newline() {
        assert_eq!(
            bracketed_paste("", 0, "1 + 1\n", |_| true),
            PasteAction::Submit("1 + 1".into())
        );
    }

    #[test]
    fn paste_does_not_submit_when_cursor_not_at_end() {
        assert_eq!(
            bracketed_paste("abc", 1, "1 + 1\n", |_| true),
            PasteAction::Insert("a1 + 1\nbc".into())
        );
    }

    #[test]
    fn paste_does_not_submit_incomplete_code() {
        assert_eq!(
            bracketed_paste("", 0, "1 +\n", |_| false),
            PasteAction::Insert("1 +\n".into())
        );
    }

    #[test]
    fn paste_normalizes_crlf_and_cr() {
        assert_eq!(
            bracketed_paste("", 0, "a\r\nb\rc", |_| false),
            PasteAction::Insert("a\nb\nc".into())
        );
    }

    #[test]
    fn paste_returns_insert_when_no_trailing_newline() {
        assert_eq!(
            bracketed_paste("", 0, "hello", |_| false),
            PasteAction::Insert("hello".into())
        );
    }

    // --- editor selection ---

    #[test]
    fn select_editor_prefers_r_option_over_env() {
        assert_eq!(select_editor(Some("emacs")), "emacs");
    }

    #[test]
    fn select_editor_defaults_to_vi() {
        let _guard = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        // Temporarily clear env vars so fallback is tested
        let old_editor = std::env::var("EDITOR").ok();
        let old_visual = std::env::var("VISUAL").ok();
        if old_editor.is_some() {
            unsafe { std::env::remove_var("EDITOR") };
        }
        if old_visual.is_some() {
            unsafe { std::env::remove_var("VISUAL") };
        }
        assert_eq!(select_editor(None), "vi");
        // Restore
        if let Some(v) = old_editor {
            unsafe { std::env::set_var("EDITOR", v) };
        }
        if let Some(v) = old_visual {
            unsafe { std::env::set_var("VISUAL", v) };
        }
    }
}

