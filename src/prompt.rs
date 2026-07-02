#[cfg(test)]
use crate::settings::Settings;
use crate::{
    completion, editing_hook,
    history::OrchardHistoryBackend,
    lexer::{self, TokenKind},
    r_runtime,
    r_runtime::{ConsoleSettings, PromptMode},
    settings::CustomKeyBinding,
    util,
};
use nu_ansi_term::{Color, Style};

use reedline::{
    ColumnarMenu, Completer, CompletionIntent, DefaultHinter, EditCommand, EditMode, Emacs,
    Highlighter, KeyCode, KeyModifiers, MenuBuilder, Prompt, Reedline, ReedlineEvent, ReedlineMenu,
    Signal, Span, StyledText, Suggestion, ValidationResult, Validator, Vi,
    default_emacs_keybindings, default_vi_insert_keybindings, default_vi_normal_keybindings,
};
use std::{
    borrow::Cow,
    process::Command,
    sync::{Arc, Mutex},
};

pub enum ReadResult {
    Line(String),
    CtrlC,
    Eof,
    Error,
}

pub struct PromptSession {
    editor: Reedline,
    context: Arc<Mutex<PromptContext>>,
}

impl PromptSession {
    pub fn new(settings: &ConsoleSettings) -> Self {
        let context = Arc::new(Mutex::new(PromptContext {
            settings: settings.clone(),
            mode: PromptMode::R,
            mode_arc: Arc::new(Mutex::new(PromptMode::R)),
        }));
        let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));
        let mut editor = Reedline::create()
            .with_completer(Box::new(OrchardCompleter::new(context.clone())))
            .with_validator(Box::new(OrchardValidator::new(context.clone())))
            .with_highlighter(Box::new(OrchardHighlighter {
                highlight_matching_bracket: settings.highlight_matching_bracket,
            }))
            .with_pre_edit_hook(Box::new({
                let settings = settings.clone();
                move |event, buffer, cursor| editing_hook::handle(event, buffer, cursor, &settings)
            }))
            .with_buffer_editor(
                Command::new(util::select_editor(None)),
                std::env::temp_dir().join("orchard-editor-tmp.R"),
            )
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu));
        if settings.auto_suggest {
            editor = editor.with_hinter(Box::new(DefaultHinter::default()));
        }
        let editor = editor.with_edit_mode(edit_mode(settings));
        Self { editor, context }
    }

    /// Create a new prompt session with a `OrchardHistoryBackend` wired
    /// into reedline for loaded-history search.  The backend is
    /// constructed externally so the caller can seed it from orchard's
    /// loaded entries.  `mode_arc` is shared with `PromptContext` for
    /// mode-aware history search.
    pub fn with_arc_history(
        settings: &ConsoleSettings,
        history_backend: OrchardHistoryBackend,
        mode_arc: Arc<Mutex<PromptMode>>,
    ) -> Self {
        let context = Arc::new(Mutex::new(PromptContext {
            settings: settings.clone(),
            mode: PromptMode::R,
            mode_arc: mode_arc.clone(),
        }));
        let completion_menu = Box::new(ColumnarMenu::default().with_name("completion_menu"));
        let mut editor = Reedline::create()
            .with_completer(Box::new(OrchardCompleter::new(context.clone())))
            .with_validator(Box::new(OrchardValidator::new(context.clone())))
            .with_highlighter(Box::new(OrchardHighlighter {
                highlight_matching_bracket: settings.highlight_matching_bracket,
            }))
            .with_pre_edit_hook(Box::new({
                let settings = settings.clone();
                move |event, buffer, cursor| editing_hook::handle(event, buffer, cursor, &settings)
            }))
            .with_buffer_editor(
                Command::new(util::select_editor(None)),
                std::env::temp_dir().join("orchard-editor-tmp.R"),
            )
            .with_menu(ReedlineMenu::EngineCompleter(completion_menu));
        if settings.auto_suggest {
            editor = editor.with_hinter(Box::new(DefaultHinter::default()));
        }
        let editor = editor
            .with_history(Box::new(history_backend))
            .with_edit_mode(edit_mode(settings));
        Self { editor, context }
    }

    pub fn update_mode(&self, mode: PromptMode) {
        let mut ctx = self.context.lock().unwrap();
        ctx.mode = mode.clone();
        *ctx.mode_arc.lock().unwrap() = mode;
    }

    pub fn read_line(
        &mut self,
        prompt: String,
        continuation_prompt: String,
        mode: PromptMode,
    ) -> ReadResult {
        self.context.lock().unwrap().mode = mode;
        match self.editor.read_line(&OrchardPrompt {
            prompt,
            continuation_prompt,
        }) {
            Ok(Signal::Success(line)) => ReadResult::Line(line),
            Ok(Signal::CtrlC) => ReadResult::CtrlC,
            Ok(Signal::CtrlD) => ReadResult::Eof,
            Ok(_) => ReadResult::Error,
            Err(_) => ReadResult::Error,
        }
    }
}

fn apply_custom_bindings(
    keybindings: &mut reedline::Keybindings,
    ctrl_key_map: &[CustomKeyBinding],
    escape_key_map: &[CustomKeyBinding],
) {
    for b in ctrl_key_map {
        // Reserved keys that cannot be remapped
        if b.key.len() == 1 && "mihdc".contains(b.key.as_str()) {
            continue;
        }
        if let Some(ch) = b.key.chars().next() {
            keybindings.add_binding(
                KeyModifiers::CONTROL,
                KeyCode::Char(ch),
                ReedlineEvent::Edit(vec![EditCommand::InsertString(b.value.clone())]),
            );
        }
    }
    for b in escape_key_map {
        if let Some(ch) = b.key.chars().next() {
            keybindings.add_binding(
                KeyModifiers::ALT,
                KeyCode::Char(ch),
                ReedlineEvent::Edit(vec![EditCommand::InsertString(b.value.clone())]),
            );
        }
    }
}

fn edit_mode(settings: &ConsoleSettings) -> Box<dyn EditMode> {
    if settings.editing_mode == "vi" {
        let mut insert_kb = default_vi_insert_keybindings();
        let mut normal_kb = default_vi_normal_keybindings();
        apply_custom_bindings(
            &mut insert_kb,
            &settings.ctrl_key_map,
            &settings.escape_key_map,
        );
        apply_custom_bindings(
            &mut normal_kb,
            &settings.ctrl_key_map,
            &settings.escape_key_map,
        );
        Box::new(Vi::new(insert_kb, normal_kb))
    } else {
        let mut keybindings = default_emacs_keybindings();
        keybindings.add_binding(
            KeyModifiers::NONE,
            KeyCode::Tab,
            ReedlineEvent::UntilFound(vec![
                ReedlineEvent::Menu("completion_menu".to_string()),
                ReedlineEvent::MenuNext,
            ]),
        );
        apply_custom_bindings(
            &mut keybindings,
            &settings.ctrl_key_map,
            &settings.escape_key_map,
        );
        Box::new(Emacs::new(keybindings))
    }
}

struct OrchardPrompt {
    prompt: String,
    continuation_prompt: String,
}

impl Prompt for OrchardPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _: reedline::PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(&self.prompt)
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.continuation_prompt)
    }

    fn render_prompt_history_search_indicator(
        &self,
        _: reedline::PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Owned(format!("{}history> ", self.prompt))
    }
}

#[derive(Clone)]
struct PromptContext {
    settings: ConsoleSettings,
    mode: PromptMode,
    /// Shared with the `OrchardHistoryBackend` for mode-aware search.
    mode_arc: Arc<Mutex<PromptMode>>,
}

struct OrchardValidator {
    context: Arc<Mutex<PromptContext>>,
}

impl OrchardValidator {
    fn new(context: Arc<Mutex<PromptContext>>) -> Self {
        Self { context }
    }
}

impl Validator for OrchardValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        let context = self.context.lock().unwrap().clone();
        if context.mode.multiline_allowed(&context.settings)
            && !r_runtime::text_looks_complete(line)
        {
            ValidationResult::Incomplete
        } else {
            ValidationResult::Complete
        }
    }
}

struct OrchardCompleter {
    context: Arc<Mutex<PromptContext>>,
}

impl OrchardCompleter {
    fn new(context: Arc<Mutex<PromptContext>>) -> Self {
        Self { context }
    }
}

impl Completer for OrchardCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        self.complete_with_intent(line, pos, CompletionIntent::Automatic)
    }

    fn complete_with_intent(
        &mut self,
        line: &str,
        pos: usize,
        intent: CompletionIntent,
    ) -> Vec<Suggestion> {
        let context = self.context.lock().unwrap().clone();
        if context.mode == PromptMode::Unknown {
            return Vec::new();
        }
        if context.mode == PromptMode::Shell || line[..pos.min(line.len())].starts_with(';') {
            return if intent == CompletionIntent::Manual {
                shell_suggestions(line, pos)
            } else {
                Vec::new()
            };
        }

        // Schema-aware completions for $ / @ / [[ — fire regardless of prefix length
        if let Some((schema_items, schema_span)) = completion::schema_completions(line, pos) {
            let span = Span::new(schema_span, pos.min(line.len()));
            return suggestions(schema_items, span);
        }

        // Pipe chain completion for %>% — fires regardless of prefix length
        if let Some((pipe_items, pipe_span)) = completion::pipe_completions(line, pos) {
            let span = Span::new(pipe_span, pos.min(line.len()));
            return suggestions(pipe_items, span);
        }

        // Magic command argument completion (%run, %cd, %rm, etc.)
        if let Some((magic_items, magic_span)) = completion::magic_completions(line, pos) {
            let span = Span::new(magic_span, pos.min(line.len()));
            return suggestions(magic_items, span);
        }

        // Variable selector for manual completion (Ctrl-Space / Tab)
        if intent == CompletionIntent::Manual {
            let (var_start, var_end) = completion::package_span(line, pos);
            let var_prefix = line[var_start..var_end].to_string();
            let vars = completion::variable_selector_completions(&var_prefix);
            if !vars.is_empty() {
                let span = Span::new(var_start, var_end);
                return suggestions(vars, span);
            }
        }

        let (start, end) = completion::package_span(line, pos);
        let prefix = &line[start..end];
        if intent == CompletionIntent::Automatic
            && prefix.len() < completion_prefix(&context.settings)
            && !prefix.starts_with('\\')
        {
            return Vec::new();
        }

        let span = Span::new(start, end);
        let latex = completion::latex_completions(prefix);
        if !latex.is_empty() {
            return suggestions(latex, span);
        }

        let packages = installed_packages().unwrap_or_default();
        let package_matches = completion::package_completions(line, pos, &packages);
        if completion::package_context(line, pos) || lexer::cursor_in_string(line, pos) {
            return suggestions(package_matches, span);
        }

        let raw = r_runtime::with_suppressed_stderr(|| {
            r_runtime::eval_string_raw_global(&completion::r_completion_code(
                line,
                pos,
                r_timeout(&context.settings, line, pos, intent),
            ))
        })
        .unwrap_or_default();
        let mut out = completion::completions_from_raw(
            &raw,
            context.settings.completion_adding_spaces_around_equals,
        );
        out.extend(package_matches);
        suggestions(out, span)
    }
}

fn r_timeout(
    settings: &ConsoleSettings,
    line: &str,
    pos: usize,
    intent: CompletionIntent,
) -> Option<f64> {
    if intent == CompletionIntent::Automatic && !completion::namespace_completion(line, pos) {
        Some(settings.completion_timeout)
    } else {
        None
    }
}

fn completion_prefix(settings: &ConsoleSettings) -> usize {
    settings
        .completion_prefix_length
        .max(0)
        .try_into()
        .unwrap_or(0)
}

fn shell_suggestions(line: &str, pos: usize) -> Vec<Suggestion> {
    let offset = usize::from(line.starts_with(';'));
    let pos = pos.min(line.len()).max(offset);
    let command = &line[offset..pos];
    let start = line[offset..pos]
        .rfind(char::is_whitespace)
        .map_or(offset, |i| offset + i + 1);
    suggestions(
        completion::shell_path_completions(command),
        Span::new(start, pos),
    )
}

fn installed_packages() -> anyhow::Result<Vec<String>> {
    let raw = r_runtime::eval_string_raw_global(
        "paste(.packages(all.available = TRUE), collapse='\\n')",
    )?;
    Ok(raw.lines().map(ToString::to_string).collect())
}

fn suggestions(items: Vec<completion::Completion>, span: Span) -> Vec<Suggestion> {
    items
        .into_iter()
        .map(|item| Suggestion {
            value: item.replacement,
            display_override: Some(item.display),
            span,
            ..Suggestion::default()
        })
        .collect()
}

struct OrchardHighlighter {
    highlight_matching_bracket: bool,
}

/// Find a matching bracket pair when cursor is just after a closing bracket.
/// Returns `(open_pos, close_pos)` if a match is found.
fn find_matching_bracket(line: &str, cursor: usize) -> Option<(usize, usize)> {
    if cursor == 0 || cursor > line.len() {
        return None;
    }
    let close = line[..cursor].chars().last().unwrap();
    let open = match close {
        ')' => '(',
        '}' => '{',
        ']' => '[',
        _ => return None,
    };
    let mut depth = 0;
    for (i, c) in line[..cursor - 1].chars().rev().enumerate() {
        if c == close {
            depth += 1;
        } else if c == open {
            if depth == 0 {
                return Some((cursor - 2 - i, cursor - 1));
            }
            depth -= 1;
        }
    }
    None
}

impl Highlighter for OrchardHighlighter {
    fn highlight(&self, line: &str, cursor: usize) -> StyledText {
        let matched_pair = self
            .highlight_matching_bracket
            .then(|| find_matching_bracket(line, cursor))
            .flatten();
        let mut styled = StyledText::new();
        let mut pos = 0;
        for token in lexer::tokenize(line) {
            let style = match token.kind {
                TokenKind::Comment => Style::new().fg(Color::DarkGray),
                TokenKind::String => Style::new().fg(Color::Green),
                TokenKind::Number => Style::new().fg(Color::Purple),
                TokenKind::Operator | TokenKind::Punctuation => Style::new().fg(Color::Blue),
                TokenKind::Error => Style::new().fg(Color::Red),
                _ => Style::new(),
            };
            let len = token.end - token.start;
            let final_style = if let Some((open_pos, close_pos)) = matched_pair {
                if (pos..pos + len).contains(&open_pos) || (pos..pos + len).contains(&close_pos) {
                    Style::new().fg(Color::Yellow)
                } else {
                    style
                }
            } else {
                style
            };
            styled.push((final_style, line[token.start..token.end].to_string()));
            pos += len;
        }
        styled
    }

    fn is_inside_string_literal(&self, line: &str, cursor: usize) -> bool {
        lexer::cursor_in_string(line, cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completer_respects_prefix_length() {
        let mut settings = ConsoleSettings::from(Settings::default());
        settings.completion_prefix_length = 3;
        let context = Arc::new(Mutex::new(PromptContext {
            settings,
            mode: PromptMode::R,
            mode_arc: Arc::new(Mutex::new(PromptMode::R)),
        }));
        let mut completer = OrchardCompleter::new(context);
        assert!(completer.complete("me", 2).is_empty());
    }

    #[test]
    fn validator_marks_obvious_incomplete_r() {
        let context = Arc::new(Mutex::new(PromptContext {
            settings: Settings::default().into(),
            mode: PromptMode::R,
            mode_arc: Arc::new(Mutex::new(PromptMode::R)),
        }));
        let validator = OrchardValidator::new(context);
        assert!(matches!(
            validator.validate("1 +"),
            ValidationResult::Incomplete
        ));
        assert!(matches!(
            validator.validate("1 + 1"),
            ValidationResult::Complete
        ));
    }

    #[test]
    fn edit_mode_does_not_panic() {
        let mode = edit_mode(&Settings::default().into());
        let kind = mode.edit_mode();
        assert!(
            matches!(
                kind,
                reedline::PromptEditMode::Emacs | reedline::PromptEditMode::Vi(_)
            ),
            "expected Emacs or Vi edit mode"
        );
    }

    #[test]
    fn highlight_matches_brackets() {
        let highlighter = OrchardHighlighter {
            highlight_matching_bracket: true,
        };
        let result = highlighter.highlight("foo(bar)", 8);
        // Cursor after ')' — positions: 3 is '(', 7 is ')'
        // The output is Vec<(Style, String)>, tokens are: "foo", "(", "bar", ")"
        assert_eq!(result.buffer[1].0, Style::new().fg(Color::Yellow));
        assert_eq!(result.buffer[3].0, Style::new().fg(Color::Yellow));
    }

    #[test]
    fn highlight_ignores_unmatched_brackets() {
        let highlighter = OrchardHighlighter {
            highlight_matching_bracket: true,
        };
        let result = highlighter.highlight("foo(bar", 7);
        // No matching close bracket — all styles should be non-yellow
        for (style, _) in &result.buffer {
            assert_ne!(*style, Style::new().fg(Color::Yellow));
        }
    }

    #[test]
    fn highlight_respects_disabled_setting() {
        let highlighter = OrchardHighlighter {
            highlight_matching_bracket: false,
        };
        let result = highlighter.highlight("foo(bar)", 8);
        for (style, _) in &result.buffer {
            assert_ne!(*style, Style::new().fg(Color::Yellow));
        }
    }
}
