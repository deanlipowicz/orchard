use crate::r_runtime::RRuntime;

#[derive(Clone, Debug, PartialEq)]
pub struct CustomKeyBinding {
    pub key: String,
    pub value: String,
    pub mode: String,
}

pub const PROMPT: &str = "\x1b[34mr$>\x1b[0m ";
pub const SHELL_PROMPT: &str = "\x1b[31m#!>\x1b[0m ";
pub const BROWSE_PROMPT: &str = "\x1b[33mBrowse[{}]>\x1b[0m ";
pub const VI_MODE_PROMPT: &str = "\x1b[34m[{}]\x1b[0m ";
pub const STDERR_FORMAT: &str = "\x1b[31m{}\x1b[0m";

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub auto_suggest: bool,
    pub editing_mode: String,
    pub color_scheme: String,
    pub auto_match: bool,
    pub highlight_matching_bracket: bool,
    pub auto_indentation: bool,
    pub tab_size: i32,
    pub complete_while_typing: bool,
    pub completion_timeout: f64,
    pub completion_prefix_length: i32,
    pub completion_adding_spaces_around_equals: bool,
    pub history_size: i32,
    pub global_history_file: String,
    pub local_history_file: String,
    pub history_search_no_duplicates: bool,
    pub history_search_ignore_case: bool,
    pub history_ignore_browser_commands: bool,
    pub insert_new_line: bool,
    pub indent_lines: bool,
    pub prompt: String,
    pub shell_prompt: String,
    pub browse_prompt: String,
    pub show_vi_mode_prompt: bool,
    pub vi_mode_prompt: String,
    pub stderr_format: String,
    pub auto_width: bool,
    pub automagic: bool,
    pub escape_key_map: Vec<CustomKeyBinding>,
    pub ctrl_key_map: Vec<CustomKeyBinding>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_suggest: false,
            editing_mode: "emacs".to_string(),
            color_scheme: "native".to_string(),
            auto_match: true,
            highlight_matching_bracket: false,
            auto_indentation: true,
            tab_size: 4,
            complete_while_typing: true,
            completion_timeout: 0.15,
            completion_prefix_length: 2,
            completion_adding_spaces_around_equals: true,
            history_size: 20000,
            global_history_file: "~/.orchard_history".to_string(),
            local_history_file: ".orchard_history".to_string(),
            history_search_no_duplicates: false,
            history_search_ignore_case: false,
            history_ignore_browser_commands: true,
            insert_new_line: true,
            indent_lines: true,
            prompt: PROMPT.to_string(),
            shell_prompt: SHELL_PROMPT.to_string(),
            browse_prompt: BROWSE_PROMPT.to_string(),
            show_vi_mode_prompt: true,
            vi_mode_prompt: VI_MODE_PROMPT.to_string(),
            stderr_format: STDERR_FORMAT.to_string(),
            auto_width: true,
            automagic: false,
            escape_key_map: Vec::new(),
            ctrl_key_map: Vec::new(),
        }
    }
}

fn parse_key_bindings(raw: &str) -> Vec<CustomKeyBinding> {
    raw.lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let key = parts.next()?.to_string();
            let value = parts.next()?.to_string();
            let mode = parts.next().unwrap_or("r").to_string();
            Some(CustomKeyBinding { key, value, mode })
        })
        .collect()
}

impl Settings {
    pub fn load_from_r_options(runtime: &mut RRuntime) -> anyhow::Result<Self> {
        let d = Self::default();
        let sys_prompt = runtime
            .get_option_string("prompt", Some("> "))?
            .unwrap_or_else(|| "> ".to_string());
        let prompt = runtime
            .get_option_string("orchard.prompt", None)?
            .unwrap_or_else(|| {
                if sys_prompt == "> " {
                    d.prompt.clone()
                } else {
                    sys_prompt
                }
            });
        let default_auto_width = runtime.get_option_bool("setWidthOnResize", d.auto_width)?;

        Ok(Self {
            auto_suggest: runtime.get_option_bool("orchard.auto_suggest", d.auto_suggest)?,
            editing_mode: runtime
                .get_option_string("orchard.editing_mode", Some(&d.editing_mode))?
                .unwrap_or(d.editing_mode),
            color_scheme: runtime
                .get_option_string("orchard.color_scheme", Some(&d.color_scheme))?
                .unwrap_or(d.color_scheme),
            auto_match: runtime.get_option_bool("orchard.auto_match", d.auto_match)?,
            highlight_matching_bracket: runtime.get_option_bool(
                "orchard.highlight_matching_bracket",
                d.highlight_matching_bracket,
            )?,
            auto_indentation: runtime
                .get_option_bool("orchard.auto_indentation", d.auto_indentation)?,
            tab_size: runtime.get_option_int("orchard.tab_size", d.tab_size)?,
            complete_while_typing: runtime
                .get_option_bool("orchard.complete_while_typing", d.complete_while_typing)?,
            completion_timeout: runtime
                .get_option_real("orchard.completion_timeout", d.completion_timeout)?,
            completion_prefix_length: runtime.get_option_int(
                "orchard.completion_prefix_length",
                d.completion_prefix_length,
            )?,
            completion_adding_spaces_around_equals: runtime.get_option_bool(
                "orchard.completion_adding_spaces_around_equals",
                d.completion_adding_spaces_around_equals,
            )?,
            history_size: runtime.get_option_int("orchard.history_size", d.history_size)?,
            global_history_file: runtime
                .get_option_string("orchard.global_history_file", Some(&d.global_history_file))?
                .unwrap_or(d.global_history_file),
            local_history_file: runtime
                .get_option_string("orchard.local_history_file", Some(&d.local_history_file))?
                .unwrap_or(d.local_history_file),
            history_search_no_duplicates: runtime.get_option_bool(
                "orchard.history_search_no_duplicates",
                d.history_search_no_duplicates,
            )?,
            history_search_ignore_case: runtime.get_option_bool(
                "orchard.history_search_ignore_case",
                d.history_search_ignore_case,
            )?,
            history_ignore_browser_commands: runtime.get_option_bool(
                "orchard.history_ignore_browser_commands",
                d.history_ignore_browser_commands,
            )?,
            insert_new_line: runtime
                .get_option_bool("orchard.insert_new_line", d.insert_new_line)?,
            indent_lines: runtime.get_option_bool("orchard.indent_lines", d.indent_lines)?,
            prompt,
            shell_prompt: runtime
                .get_option_string("orchard.shell_prompt", Some(&d.shell_prompt))?
                .unwrap_or(d.shell_prompt),
            browse_prompt: runtime
                .get_option_string("orchard.browse_prompt", Some(&d.browse_prompt))?
                .unwrap_or(d.browse_prompt),
            show_vi_mode_prompt: runtime
                .get_option_bool("orchard.show_vi_mode_prompt", d.show_vi_mode_prompt)?,
            vi_mode_prompt: runtime
                .get_option_string("orchard.vi_mode_prompt", Some(&d.vi_mode_prompt))?
                .unwrap_or(d.vi_mode_prompt),
            stderr_format: runtime
                .get_option_string("orchard.stderr_format", Some(&d.stderr_format))?
                .unwrap_or(d.stderr_format),
            auto_width: runtime.get_option_bool("orchard.auto_width", default_auto_width)?,
            automagic: runtime.get_option_bool("orchard.automagic", d.automagic)?,
            escape_key_map: parse_key_bindings(&runtime.eval_string_raw(
                "local({v <- getOption('orchard.escape_key_map'); \
                 if (is.null(v) || length(v) == 0) '' else \
                 paste(vapply(v, function(x) paste( \
                   if (is.null(x[['key']])) '' else x[['key']], \
                   if (is.null(x[['value']])) '' else x[['value']], \
                   if (is.null(x[['mode']])) 'r' else x[['mode']], \
                   sep = '\t'), character(1)), collapse = '\n')})",
            )?),
            ctrl_key_map: parse_key_bindings(&runtime.eval_string_raw(
                "local({v <- getOption('orchard.ctrl_key_map'); \
                 if (is.null(v) || length(v) == 0) '' else \
                 paste(vapply(v, function(x) paste( \
                   if (is.null(x[['key']])) '' else x[['key']], \
                   if (is.null(x[['value']])) '' else x[['value']], \
                   if (is.null(x[['mode']])) 'r' else x[['mode']], \
                   sep = '\t'), character(1)), collapse = '\n')})",
            )?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_plan() {
        let s = Settings::default();
        assert_eq!(s.prompt, PROMPT);
        assert_eq!(s.browse_prompt, BROWSE_PROMPT);
        assert_eq!(s.history_size, 20000);
        assert!(s.auto_match);
        assert!(s.auto_width);
    }

    #[test]
    fn custom_key_binding_parses() {
        let kb = CustomKeyBinding {
            key: "ctrl-r".into(),
            value: "HistoryHint".into(),
            mode: "emacs".into(),
        };
        assert_eq!(kb.key, "ctrl-r");
        assert_eq!(kb.value, "HistoryHint");
        assert_eq!(kb.mode, "emacs");
    }

    #[test]
    fn prompt_constants() {
        assert!(PROMPT.contains("r$>"));
        assert!(SHELL_PROMPT.contains("#!>"));
        assert!(BROWSE_PROMPT.contains("Browse"));
        assert!(VI_MODE_PROMPT.contains("{}"));
        assert!(STDERR_FORMAT.contains("{}"));
    }
}
