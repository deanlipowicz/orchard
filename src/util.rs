use std::env;
use std::path::PathBuf;

pub fn expand_tilde(input: &str) -> String {
    if let Some(rest) = input.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return format!("{}/{}", home.display(), rest);
        }
    } else if input == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home.display().to_string();
    }
    input.to_string()
}

pub fn expand_vars(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '$' {
            out.push(ch);
            continue;
        }
        if chars.peek() == Some(&'{') {
            chars.next();
            let mut name = String::new();
            for ch in chars.by_ref() {
                if ch == '}' {
                    break;
                }
                name.push(ch);
            }
            out.push_str(&env::var(name).unwrap_or_default());
        } else {
            let mut name = String::new();
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                name.push(chars.next().unwrap());
            }
            if name.is_empty() {
                out.push('$');
            } else {
                out.push_str(&env::var(name).unwrap_or_default());
            }
        }
    }
    out
}

pub fn home() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn r_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

pub fn select_editor(r_option: Option<&str>) -> String {
    r_option
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .or_else(|| env::var("VISUAL").ok().filter(|s| !s.is_empty()))
        .or_else(|| env::var("EDITOR").ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "vi".to_string())
}
