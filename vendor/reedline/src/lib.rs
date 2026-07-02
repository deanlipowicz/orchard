//! - General editing functionality, that should feel familiar coming from other shells (e.g. bash, fish, zsh).
//! - Configurable keybindings (emacs-style bindings and basic vi-style).
//! - Configurable prompt
//! - Content-aware syntax highlighting.
//! - Autocompletion (With graphical selection menu or simple cycling inline).
//! - History with interactive search options (optionally persists to file, can support multiple sessions accessing the same file)
//! - Fish-style history autosuggestion hints
//! - Undo support.
//! - Clipboard integration
//! - Line completeness validation for seamless entry of multiline command sequences.
//!
//! ### Areas for future improvements
//!
//! - [ ] Support for Unicode beyond simple left-to-right scripts
//! - [ ] Easier keybinding configuration
//! - [ ] Support for more advanced vi commands
//! - [ ] Visual selection
//! - [ ] Smooth experience if completion or prompt content takes long to compute
//! - [ ] Support for a concurrent output stream from background tasks to be displayed, while the input prompt is active. ("Full duplex" mode)
//!
//! For more ideas check out the [feature discussion](https://github.com/nushell/reedline/issues/63) or hop on the `#reedline` channel of the [nushell discord](https://discordapp.com/invite/NtAbbGn).
//!
//! ### Development history
//!
//! If you want to follow along with the history how reedline got started, you can watch the [recordings](https://youtube.com/playlist?list=PLP2yfE2-FXdQw0I6O4YdIX_mzBeF5TDdv) of [JT](https://github.com/jntrnr)'s [live-coding streams](https://www.twitch.tv/jntrnr).
//!
//! [Playlist: Creating a line editor in Rust](https://youtube.com/playlist?list=PLP2yfE2-FXdQw0I6O4YdIX_mzBeF5TDdv)
//!
//! ### Alternatives
//!
//! For currently more mature Rust line editing check out:
//!
//! - [rustyline](https://crates.io/crates/rustyline)
//!
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(missing_docs)]
// #![deny(warnings)]
mod core_editor;
pub use core_editor::Editor;
pub use core_editor::LineBuffer;

mod enums;
pub use enums::{
    EditCommand, EditCommandDiscriminants, MouseButton, ReedlineEvent, ReedlineEventDiscriminants,
    ReedlineRawEvent, Signal, TextObject, TextObjectScope, TextObjectType, UndoBehavior,
};

mod painting;
pub use painting::{Painter, StyledText};

mod engine;
pub use engine::{MouseClickMode, Reedline};

mod result;
pub use result::{ReedlineError, ReedlineErrorVariants, Result};

mod history;
#[cfg(any(feature = "sqlite", feature = "sqlite-dynlib"))]
pub use history::SqliteBackedHistory;
pub use history::{
    CommandLineSearch, FileBackedHistory, History, HistoryItem, HistoryItemExtraInfo,
    HistoryItemId, HistoryNavigationQuery, HistorySessionId, IgnoreAllExtraInfo, SearchDirection,
    SearchFilter, SearchQuery, HISTORY_SIZE,
};

mod prompt;
pub use prompt::{
    DefaultPrompt, DefaultPromptSegment, Prompt, PromptEditMode, PromptEditModeDiscriminants,
    PromptHistorySearch, PromptHistorySearchStatus, PromptViMode,
};

mod edit_mode;
#[cfg(feature = "helix")]
pub use edit_mode::Helix;
pub use edit_mode::{
    default_emacs_keybindings, default_vi_insert_keybindings, default_vi_normal_keybindings,
    CursorConfig, EditMode, Emacs, Keybindings, Vi,
};

mod highlighter;
pub use highlighter::{ExampleHighlighter, Highlighter, SimpleMatchHighlighter};

mod completion;
pub use completion::{Completer, CompletionIntent, DefaultCompleter, Span, Suggestion};

mod hinter;
pub use hinter::CwdAwareHinter;
pub use hinter::{DefaultHinter, Hinter};

mod validator;
pub use validator::{DefaultValidator, ValidationResult, Validator};

mod menu;
pub use menu::{
    menu_functions, ColumnarMenu, DescriptionMenu, DescriptionMode, IdeMenu, ListMenu, Menu,
    MenuBuilder, MenuEvent, MenuSettings, MenuTextStyle, ReedlineMenu, TraversalDirection,
};

mod terminal_extensions;
pub use terminal_extensions::kitty_protocol_available;
pub use terminal_extensions::semantic_prompt::{
    Osc133ClickEventsMarkers, Osc133Markers, Osc633Markers, PromptKind, SemanticPromptMarkers,
};

mod utils;

mod external_printer;
pub use utils::{
    get_reedline_default_keybindings, get_reedline_keybinding_modifiers, get_reedline_keycodes,
};

#[expect(deprecated)]
pub use utils::{
    get_reedline_edit_commands, get_reedline_prompt_edit_modes, get_reedline_reedline_events,
};

// Reexport the key types to be independent from an explicit crossterm dependency.
pub use crossterm::{
    event::{KeyCode, KeyModifiers},
    style::Color,
};
#[cfg(feature = "external_printer")]
pub use external_printer::ExternalPrinter;
