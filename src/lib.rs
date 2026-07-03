#![deny(unsafe_op_in_unsafe_fn)]
pub mod cli;

pub mod completion;

pub mod dyld;
pub mod editing_hook;
pub mod editor_bridge;
pub mod env_setup;
pub mod frequency;
pub mod history;
pub mod lexer;
pub mod magic;
pub mod magics;
pub mod profile;
pub mod prompt;
pub mod r_discovery;
pub mod r_runtime;
pub mod settings;
pub mod shell;
pub mod terminal_graphics;
pub mod util;
