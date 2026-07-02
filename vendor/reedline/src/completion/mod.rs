mod base;
mod default;
pub(crate) mod history;

pub use base::{Completer, CompletionIntent, Span, Suggestion};
pub use default::DefaultCompleter;
