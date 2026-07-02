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
}

