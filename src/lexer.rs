#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Comment,
    Whitespace,
    Name,
    Number,
    Operator,
    Punctuation,
    String,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub end: usize,
}

pub fn cursor_in_string(text: &str, cursor: usize) -> bool {
    let end = cursor.min(text.len());
    let before = text[..end].trim_end();
    tokenize(before)
        .into_iter()
        .rev()
        .find(|t| t.kind != TokenKind::Whitespace && t.kind != TokenKind::Comment)
        .is_some_and(|t| matches!(t.kind, TokenKind::String | TokenKind::Error))
}

pub fn tokenize(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            out.push(tok(TokenKind::Whitespace, start, i));
        } else if b == b'#' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            out.push(tok(TokenKind::Comment, start, i));
        } else if b == b'r' && i + 1 < bytes.len() && bytes[i + 1] == b'"' {
            if let Some(end) = raw_string(bytes, i) {
                i = end;
                out.push(tok(TokenKind::String, start, i));
            } else {
                i = bytes.len();
                out.push(tok(TokenKind::Error, start, i));
            }
        } else if b == b'\'' || b == b'"' {
            i = quoted(bytes, i, b);
            out.push(tok(
                if i <= bytes.len() && bytes[i - 1] == b {
                    TokenKind::String
                } else {
                    TokenKind::Error
                },
                start,
                i,
            ));
        } else if b == b'`' {
            i += 1;
            while i < bytes.len() && bytes[i] != b'`' {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
                out.push(tok(TokenKind::Name, start, i));
            } else {
                out.push(tok(TokenKind::Error, start, i));
            }
        } else if b.is_ascii_digit()
            || (b == b'.' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit())
        {
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_')
            {
                i += 1;
            }
            out.push(tok(TokenKind::Number, start, i));
        } else if is_name_start(b) {
            i += 1;
            while i < bytes.len() && is_name_continue(bytes[i]) {
                i += 1;
            }
            out.push(tok(TokenKind::Name, start, i));
        } else if b"()[]{};,".contains(&b) {
            i += 1;
            out.push(tok(TokenKind::Punctuation, start, i));
        } else {
            i += 1;
            while i < bytes.len() && b"+-*/^=<>!|&:$~@?".contains(&bytes[i]) {
                i += 1;
            }
            out.push(tok(TokenKind::Operator, start, i));
        }
    }
    out
}

fn tok(kind: TokenKind, start: usize, end: usize) -> Token {
    Token { kind, start, end }
}

fn quoted(bytes: &[u8], mut i: usize, quote: u8) -> usize {
    i += 1;
    let mut escaped = false;
    while i < bytes.len() {
        if escaped {
            escaped = false;
        } else if bytes[i] == b'\\' {
            escaped = true;
        } else if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    i
}

fn raw_string(bytes: &[u8], i: usize) -> Option<usize> {
    let mut j = i + 2;
    let dash_start = j;
    while j < bytes.len() && bytes[j] == b'-' {
        j += 1;
    }
    let open = *bytes.get(j)?;
    let close = match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        _ => return None,
    };
    let dashes = &bytes[dash_start..j];
    j += 1;
    while j < bytes.len() {
        if bytes[j] == close
            && bytes.get(j + 1..j + 1 + dashes.len()) == Some(dashes)
            && bytes.get(j + 1 + dashes.len()) == Some(&b'"')
        {
            return Some(j + 2 + dashes.len());
        }
        j += 1;
    }
    None
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'.' || b == b'_'
}

fn is_name_continue(b: u8) -> bool {
    is_name_start(b) || b.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_normal_and_escaped_strings() {
        assert!(cursor_in_string("x <- \"abc", 9));
        assert!(cursor_in_string("x <- \"a\\\"b\"", 11));
        assert!(!cursor_in_string("x <- \"a\\\"b\" +", 13));
    }

    #[test]
    fn comments_are_not_strings() {
        assert!(!cursor_in_string("x # \"abc", 8));
    }

    #[test]
    fn detects_raw_strings() {
        assert!(cursor_in_string("x <- r\"---(a)---\"", 17));
        assert!(!cursor_in_string("x <- r\"---(a)---\" +", 19));
        assert!(cursor_in_string("x <- r\"---(a", 12));
    }

    // --- Direct tokenize() tests ---

    fn kinds(text: &str) -> Vec<TokenKind> {
        tokenize(text).into_iter().map(|t| t.kind).collect()
    }

    fn kinds_ranges(text: &str) -> Vec<(TokenKind, usize, usize)> {
        tokenize(text)
            .into_iter()
            .map(|t| (t.kind, t.start, t.end))
            .collect()
    }

    // Whitespace

    #[test]
    fn tokenize_single_space() {
        assert_eq!(kinds_ranges(" "), vec![(TokenKind::Whitespace, 0, 1)]);
    }

    #[test]
    fn tokenize_whitespace_run() {
        assert_eq!(
            kinds_ranges("   \t\t  "),
            vec![(TokenKind::Whitespace, 0, 7)]
        );
    }

    #[test]
    fn tokenize_newline_is_whitespace() {
        assert_eq!(
            kinds_ranges("a\nb"),
            vec![
                (TokenKind::Name, 0, 1),
                (TokenKind::Whitespace, 1, 2),
                (TokenKind::Name, 2, 3),
            ]
        );
    }

    // Comments

    #[test]
    fn tokenize_comment_to_end_of_input() {
        assert_eq!(kinds_ranges("# hello"), vec![(TokenKind::Comment, 0, 7)]);
    }

    #[test]
    fn tokenize_comment_stops_at_newline() {
        let toks = kinds_ranges("# c\nx");
        assert_eq!(toks.len(), 3);
        assert_eq!(toks[0], (TokenKind::Comment, 0, 3));
        assert_eq!(toks[1], (TokenKind::Whitespace, 3, 4));
        assert_eq!(toks[2], (TokenKind::Name, 4, 5));
    }

    #[test]
    fn tokenize_empty_comment() {
        assert_eq!(kinds_ranges("#"), vec![(TokenKind::Comment, 0, 1)]);
    }

    // Raw strings

    #[test]
    fn tokenize_raw_string_parens() {
        assert_eq!(
            kinds_ranges("r\"(hello)\""),
            vec![(TokenKind::String, 0, 10)]
        );
    }

    #[test]
    fn tokenize_raw_string_brackets() {
        assert_eq!(
            kinds_ranges("r\"[hello]\""),
            vec![(TokenKind::String, 0, 10)]
        );
    }

    #[test]
    fn tokenize_raw_string_braces() {
        assert_eq!(
            kinds_ranges("r\"{hello}\""),
            vec![(TokenKind::String, 0, 10)]
        );
    }

    #[test]
    fn tokenize_raw_string_with_dashes() {
        // r"--(content)--"
        assert_eq!(
            kinds_ranges("r\"--(content)--\""),
            vec![(TokenKind::String, 0, 16)]
        );
    }

    #[test]
    fn tokenize_raw_string_unterminated_is_error() {
        assert_eq!(
            kinds_ranges("r\"(no close"),
            vec![(TokenKind::Error, 0, 11)]
        );
    }

    #[test]
    fn tokenize_raw_string_unterminated_with_dashes_is_error() {
        assert_eq!(
            kinds_ranges("r\"--(no close"),
            vec![(TokenKind::Error, 0, 13)]
        );
    }

    #[test]
    fn tokenize_raw_string_with_mismatched_close_is_error() {
        // r"(content]  -- close bracket doesn't match open paren
        assert_eq!(
            kinds_ranges("r\"(content]\""),
            vec![(TokenKind::Error, 0, 12)]
        );
    }

    // Quoted strings

    #[test]
    fn tokenize_double_quoted_string() {
        assert_eq!(kinds_ranges("\"hello\""), vec![(TokenKind::String, 0, 7)]);
    }

    #[test]
    fn tokenize_single_quoted_string() {
        assert_eq!(kinds_ranges("'hello'"), vec![(TokenKind::String, 0, 7)]);
    }

    #[test]
    fn tokenize_string_with_escaped_quote() {
        assert_eq!(kinds_ranges("\"a\\\"b\""), vec![(TokenKind::String, 0, 6)]);
    }

    #[test]
    fn tokenize_string_with_escaped_backslash() {
        assert_eq!(kinds_ranges("'a\\\\b'"), vec![(TokenKind::String, 0, 6)]);
    }

    #[test]
    fn tokenize_unterminated_double_quote_is_error() {
        assert_eq!(kinds_ranges("\"no close"), vec![(TokenKind::Error, 0, 9)]);
    }

    #[test]
    fn tokenize_unterminated_single_quote_is_error() {
        assert_eq!(kinds_ranges("'no close"), vec![(TokenKind::Error, 0, 9)]);
    }

    #[test]
    fn tokenize_empty_string() {
        assert_eq!(kinds_ranges("\"\""), vec![(TokenKind::String, 0, 2)]);
    }

    // Backtick names

    #[test]
    fn tokenize_backtick_name() {
        assert_eq!(kinds_ranges("`my name`"), vec![(TokenKind::Name, 0, 9)]);
    }

    #[test]
    fn tokenize_backtick_name_unterminated_is_error() {
        assert_eq!(kinds_ranges("`no close"), vec![(TokenKind::Error, 0, 9)]);
    }

    #[test]
    fn tokenize_empty_backtick_name() {
        assert_eq!(kinds_ranges("``"), vec![(TokenKind::Name, 0, 2)]);
    }

    // Numbers

    #[test]
    fn tokenize_integer() {
        assert_eq!(kinds_ranges("42"), vec![(TokenKind::Number, 0, 2)]);
    }

    #[test]
    fn tokenize_decimal_number() {
        assert_eq!(kinds_ranges("3.14"), vec![(TokenKind::Number, 0, 4)]);
    }

    #[test]
    fn tokenize_leading_dot_number() {
        assert_eq!(kinds_ranges(".5"), vec![(TokenKind::Number, 0, 2)]);
    }

    #[test]
    fn tokenize_number_with_underscore() {
        assert_eq!(kinds_ranges("1_000"), vec![(TokenKind::Number, 0, 5)]);
    }

    #[test]
    fn tokenize_scientific_notation_number() {
        assert_eq!(kinds_ranges("1e10"), vec![(TokenKind::Number, 0, 4)]);
    }

    #[test]
    fn tokenize_leading_dot_alone_is_name_not_number() {
        // "." followed by non-digit is a name
        assert_eq!(kinds_ranges(".foo"), vec![(TokenKind::Name, 0, 4)]);
    }

    // Names

    #[test]
    fn tokenize_simple_name() {
        assert_eq!(kinds_ranges("foo"), vec![(TokenKind::Name, 0, 3)]);
    }

    #[test]
    fn tokenize_name_with_dot() {
        assert_eq!(kinds_ranges("obj.method"), vec![(TokenKind::Name, 0, 10)]);
    }

    #[test]
    fn tokenize_name_with_underscore() {
        assert_eq!(kinds_ranges("my_var"), vec![(TokenKind::Name, 0, 6)]);
    }

    #[test]
    fn tokenize_name_starting_with_dot() {
        assert_eq!(kinds_ranges(".hidden"), vec![(TokenKind::Name, 0, 7)]);
    }

    #[test]
    fn tokenize_name_starting_with_underscore() {
        assert_eq!(kinds_ranges("_private"), vec![(TokenKind::Name, 0, 8)]);
    }

    // Punctuation

    #[test]
    fn tokenize_all_punctuation() {
        let toks = kinds_ranges("()[]{};,");
        assert_eq!(toks.len(), 8);
        for (kind, _, _) in &toks {
            assert_eq!(*kind, TokenKind::Punctuation);
        }
    }

    #[test]
    fn tokenize_punctuation_ranges() {
        assert_eq!(
            kinds_ranges("()"),
            vec![
                (TokenKind::Punctuation, 0, 1),
                (TokenKind::Punctuation, 1, 2),
            ]
        );
    }

    // Operators

    #[test]
    fn tokenize_single_char_operators() {
        for op in &[
            "+", "-", "*", "/", "^", "=", "<", ">", "!", "|", "&", ":", "$", "~", "@", "?",
        ] {
            let toks = kinds_ranges(op);
            assert_eq!(toks.len(), 1, "expected 1 token for operator {:?}", op);
            assert_eq!(
                toks[0].0,
                TokenKind::Operator,
                "expected Operator for {:?}",
                op
            );
        }
    }

    #[test]
    fn tokenize_multi_char_operators() {
        assert_eq!(kinds_ranges("<-"), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges("->"), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges("=="), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges("!="), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges(">="), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges("<="), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges("&&"), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges("||"), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges("|>"), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges("::"), vec![(TokenKind::Operator, 0, 2)]);
        assert_eq!(kinds_ranges(":::"), vec![(TokenKind::Operator, 0, 3)]);
        assert_eq!(kinds_ranges("<<-"), vec![(TokenKind::Operator, 0, 3)]);
        assert_eq!(kinds_ranges("->>"), vec![(TokenKind::Operator, 0, 3)]);
    }

    // Mixed / realistic expressions

    #[test]
    fn tokenize_assignment_expression() {
        let toks = kinds("x <- 42");
        assert_eq!(
            toks,
            vec![
                TokenKind::Name,
                TokenKind::Whitespace,
                TokenKind::Operator,
                TokenKind::Whitespace,
                TokenKind::Number,
            ]
        );
    }

    #[test]
    fn tokenize_function_call() {
        let toks = kinds("print(x)");
        assert_eq!(
            toks,
            vec![
                TokenKind::Name,
                TokenKind::Punctuation,
                TokenKind::Name,
                TokenKind::Punctuation,
            ]
        );
    }

    #[test]
    fn tokenize_full_line_with_comment() {
        let toks = kinds_ranges("x <- 1 # set x");
        assert_eq!(toks.len(), 7);
        assert_eq!(toks[0], (TokenKind::Name, 0, 1));
        assert_eq!(toks[1], (TokenKind::Whitespace, 1, 2));
        assert_eq!(toks[2], (TokenKind::Operator, 2, 4));
        assert_eq!(toks[3], (TokenKind::Whitespace, 4, 5));
        assert_eq!(toks[4], (TokenKind::Number, 5, 6));
        assert_eq!(toks[5], (TokenKind::Whitespace, 6, 7));
        assert_eq!(toks[6], (TokenKind::Comment, 7, 14));
    }

    #[test]
    fn tokenize_empty_input() {
        assert!(tokenize("").is_empty());
    }

    #[test]
    fn tokenize_only_whitespace() {
        assert_eq!(kinds("   "), vec![TokenKind::Whitespace]);
    }

    #[test]
    fn tokenize_pipe_operator_chain() {
        let toks = kinds("x |> f() |> g()");
        assert_eq!(
            toks,
            vec![
                TokenKind::Name, // x
                TokenKind::Whitespace,
                TokenKind::Operator, // |>
                TokenKind::Whitespace,
                TokenKind::Name,        // f
                TokenKind::Punctuation, // (
                TokenKind::Punctuation, // )
                TokenKind::Whitespace,
                TokenKind::Operator, // |>
                TokenKind::Whitespace,
                TokenKind::Name,        // g
                TokenKind::Punctuation, // (
                TokenKind::Punctuation, // )
            ]
        );
    }

    #[test]
    fn tokenize_string_with_escape_sequences() {
        assert_eq!(
            kinds_ranges("\"a\\nb\\tc\""),
            vec![(TokenKind::String, 0, 9)]
        );
    }

    #[test]
    fn tokenize_raw_string_containing_quotes() {
        // r"(he said "hi")"
        assert_eq!(
            kinds_ranges("r\"(he said \"hi\")\""),
            vec![(TokenKind::String, 0, 17)]
        );
    }

    #[test]
    fn tokenize_raw_string_with_dashes_containing_dashes() {
        // r"--(a--b)--"
        assert_eq!(
            kinds_ranges("r\"--(a--b)--\""),
            vec![(TokenKind::String, 0, 13)]
        );
    }
}
